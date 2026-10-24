//! VPN 前置检查第一阶段：隔离准备候选资产，不发布到现有管理器，不授予接管权限。
pub mod platform_check;
pub mod preflight;
pub mod startup_budget;
/// 四阶段驱动尚未注册到生产 FFI，不能替代系统接管验收。
pub mod transaction;
use crate::{
    budget::{ByteBudget, ByteLease},
    configuration::{Document, Summary},
    error::{Error, ErrorInfo, Result},
    https::HttpsClient,
    lifecycle::session::{Authority, Phase as OwnerPhase},
    outbound::Manager,
};
use serde::Serialize;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const MAX_PREPARATIONS: usize = 2;
const PREPARATION_TIMEOUT: Duration = Duration::from_secs(120);
static SLOTS: ByteBudget = ByteBudget::new(MAX_PREPARATIONS);
static INPUTS: ByteBudget = ByteBudget::new(2 * crate::configuration::MAX_CONFIG_BYTES);

/// CandidateReady 只证明资产编译完成，绝不等于 Preflight 通过或 VPN Running。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Phase {
    Preparing,
    CandidateReady,
    Failed,
    Closing,
    Closed,
}
#[derive(Serialize)]
pub struct Status {
    pub phase: Phase,
    pub configuration: Option<Summary>,
    pub geoip_status: crate::geoip::Status,
    pub error_info: Option<ErrorInfo>,
    pub cleanup_error_info: Option<ErrorInfo>,
    /// 本阶段没有接管接口；宿主不得把成功准备当作权限或确认令牌。
    pub activation_permitted: bool,
}
struct Outcome {
    phase: Phase,
    error: Option<Error>,
    cleanup_error: Option<Error>,
}
struct Inner {
    authority: Arc<Authority>,
    manager: Arc<Manager>,
    outcome: Mutex<Outcome>,
    slot: Mutex<Option<ByteLease>>,
}
/// 候选仅由内核持有；公开接口只提交意图、查询摘要或请求取消，不泄漏活动管理器。
pub struct Preparation {
    inner: Arc<Inner>,
}
impl Preparation {
    /// 在内核有界运行时异步解析、准备 DNS 并编译候选规则；资源须由宿主提供。
    /// 不写资产缓存、不启动自动刷新、不触发 VPN；例如 begin("rules: ['final,direct']")。
    pub fn begin(yaml: &str) -> Result<Self> {
        Self::begin_with_client(yaml, None)
    }
    fn begin_with_client(yaml: &str, tls: Option<HttpsClient>) -> Result<Self> {
        Self::begin_resources(
            yaml,
            tls,
            PREPARATION_TIMEOUT,
            crate::storage::Resources::default(),
        )
    }
    /// 只读加载宿主授权配置及资源，候选准备不写入文件或授予系统接管权限。
    pub fn begin_file(path: &str, resources: crate::storage::Resources) -> Result<Self> {
        resources.validate()?;
        let text = crate::storage::read_configuration(path)?;
        Self::begin_resources(text.as_str(), None, PREPARATION_TIMEOUT, resources)
    }
    #[cfg(test)]
    fn begin_with_deadline(
        yaml: &str,
        tls: Option<HttpsClient>,
        deadline: Duration,
    ) -> Result<Self> {
        Self::begin_resources(yaml, tls, deadline, crate::storage::Resources::default())
    }
    fn begin_resources(
        yaml: &str,
        tls: Option<HttpsClient>,
        deadline: Duration,
        resources: crate::storage::Resources,
    ) -> Result<Self> {
        resources.validate()?;
        let runtime = crate::lifecycle::runtime()?;
        if deadline.is_zero() || deadline > PREPARATION_TIMEOUT {
            return Err(Error::InvalidInput);
        }
        if yaml.len() > crate::configuration::MAX_CONFIG_BYTES {
            return Err(Error::ResourceLimit);
        }
        let slot = SLOTS.reserve(1)?;
        let input_lease = INPUTS.reserve(yaml.len())?;
        let mut input = String::new();
        input
            .try_reserve_exact(yaml.len())
            .map_err(|_| Error::ResourceLimit)?;
        input.push_str(yaml);
        let input = zeroize::Zeroizing::new(input);
        let inner = Arc::new(Inner {
            authority: Authority::new()?,
            manager: Arc::new(Manager::default()),
            outcome: Mutex::new(Outcome {
                phase: Phase::Preparing,
                error: None,
                cleanup_error: None,
            }),
            slot: Mutex::new(Some(slot)),
        });
        let operation = inner.authority.enter(inner.authority.identity())?;
        let work = inner.clone();
        runtime.spawn(Box::pin(async move {
            let _operation = operation;
            let prepare = async {
                let parsing = work.authority.enter(work.authority.identity())?;
                let document = tokio::task::spawn_blocking(move || {
                    let _parsing = parsing;
                    let _input_lease = input_lease;
                    Document::from_yaml(&input).map(Arc::new)
                })
                .await
                .map_err(|_| Error::Internal)??;
                document.preparation_preflight(
                    crate::configuration::PreparationPurpose::VpnCandidate,
                )?;
                let client = match tls {
                    Some(tls) => tls,
                    None => HttpsClient::system()?,
                };
                work.manager
                    .prepare_vpn_candidate(document, client, resources, None)
                    .await?;
                Ok(())
            };
            let result = tokio::select! {
                _ = work.authority.canceled() => Err(Error::Cancelled),
                result = tokio::time::timeout(deadline, Box::pin(prepare)) =>
                    result.unwrap_or(Err(Error::VpnPreparationTimeout)),
            };
            let cleanup_error = if result.is_err() {
                work.manager.stop().await.err()
            } else {
                None
            };
            let recorded = work.authority.with_active(work.authority.identity(), || {
                let mut state = work.outcome.lock().map_err(|_| Error::Internal)?;
                state.phase = if result.is_ok() {
                    Phase::CandidateReady
                } else {
                    Phase::Failed
                };
                state.error = result.err();
                state.cleanup_error = cleanup_error;
                Ok(())
            });
            if let Err(error) = recorded
                && error != Error::InvalidState
            {
                crate::error::diagnostics::record(
                    error,
                    crate::error::diagnostics::DiagnosticSite::BackgroundJob,
                );
            }
        }));
        Ok(Self { inner })
    }
    /// 摘要不含凭据、URL、正文或授权；未通过的后续检查不伪造为成功。
    pub fn status(&self) -> Result<Status> {
        let owner = self.inner.authority.phase()?;
        let state = self.inner.outcome.lock().map_err(|_| Error::Internal)?;
        let phase = match owner {
            OwnerPhase::Active => state.phase,
            OwnerPhase::Closing => Phase::Closing,
            OwnerPhase::Closed => Phase::Closed,
            OwnerPhase::Failed => Phase::Failed,
        };
        let configuration = self.inner.manager.configuration_status()?;
        Ok(Status {
            phase,
            configuration: if phase == Phase::CandidateReady {
                configuration.configuration
            } else {
                None
            },
            geoip_status: configuration.geoip_status,
            error_info: state.error.map(Error::public_info),
            cleanup_error_info: state.cleanup_error.map(Error::public_info),
            activation_permitted: false,
        })
    }
    /// 撤销立即阻止候选发布；等待后台准备和真实资源关闭，旧 Rust 引用不保留活动权限。
    pub async fn close(&self) -> Result<()> {
        self.inner.request_close()?;
        self.inner.authority.wait_closed().await
    }
}
impl Inner {
    fn request_close(self: &Arc<Self>) -> Result<()> {
        if self.authority.revoke()? {
            let inner = self.clone();
            crate::lifecycle::runtime()?.spawn(async move {
                let idle = inner.authority.wait_idle().await;
                let stopped = inner.manager.stop().await;
                if let Err(error) = stopped
                    && let Ok(mut state) = inner.outcome.lock()
                {
                    state.cleanup_error = Some(error);
                }
                let released = inner
                    .slot
                    .lock()
                    .map_err(|_| Error::Internal)
                    .map(|mut lease| {
                        lease.take();
                    });
                let result = idle.and(stopped).and(released);
                let _ = crate::error::diagnostics::observe(
                    inner.authority.complete(result),
                    crate::error::diagnostics::DiagnosticSite::Cleanup,
                );
            });
        }
        Ok(())
    }
}
impl Drop for Preparation {
    fn drop(&mut self) {
        let _ = crate::error::diagnostics::observe(
            self.inner.request_close(),
            crate::error::diagnostics::DiagnosticSite::Cleanup,
        );
    }
}

#[cfg(test)]
#[path = "../tests/test_vpn.rs"]
mod test_vpn;
