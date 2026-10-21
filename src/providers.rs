//! 宿主文件的只读提供者加载；不下载、调度、创建目录或持久化缓存。
mod document;
use crate::{
    budget::{ByteBudget, ByteLease},
    configuration::Document,
    error::{Error, ErrorInfo, Result},
    https::HttpsClient,
    lifecycle::session::Authority,
    outbound::Manager,
    routing::runtime::{Layer, Source},
    storage::Resources,
};
use serde::Serialize;
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex, Weak},
};
const BODY_LIMIT: usize = 8 * 1024 * 1024;
static CONTROLLERS: ByteBudget = ByteBudget::new(4);
static READ_MEMORY: ByteBudget = ByteBudget::new(64 * 1024 * 1024);
#[derive(Serialize)]
pub struct ProviderStatus {
    pub name: String,
    pub enabled: bool,
    pub rules: usize,
    pub ignored: usize,
    pub last_success: Option<u64>,
    pub last_slot: Option<i32>,
}
#[derive(Serialize)]
pub struct Status {
    pub active: bool,
    pub revision: Option<u64>,
    pub providers: Vec<ProviderStatus>,
    pub error_info: Option<ErrorInfo>,
    pub read_only: bool,
}
struct State {
    configuration: Option<Arc<Document>>,
    resources: Resources,
    revision: Option<u64>,
    counts: BTreeMap<String, (usize, usize)>,
    error: Option<ErrorInfo>,
}
pub(crate) struct Runtime {
    owner: Weak<Manager>,
    tls: HttpsClient,
    candidate: bool,
    authority: Arc<Authority>,
    traffic: Arc<Authority>,
    control: tokio::sync::Mutex<()>,
    state: Mutex<State>,
    lease: Mutex<Option<ByteLease>>,
}
impl Runtime {
    pub(crate) fn new(owner: &Arc<Manager>, tls: HttpsClient) -> Result<Arc<Self>> {
        Self::create(owner, tls, false)
    }
    pub(crate) fn candidate(owner: &Arc<Manager>, tls: HttpsClient) -> Result<Arc<Self>> {
        Self::create(owner, tls, true)
    }
    fn create(owner: &Arc<Manager>, tls: HttpsClient, candidate: bool) -> Result<Arc<Self>> {
        crate::lifecycle::runtime()?;
        Ok(Arc::new(Self {
            owner: Arc::downgrade(owner),
            tls,
            candidate,
            authority: Authority::new()?,
            traffic: Authority::new()?,
            control: tokio::sync::Mutex::new(()),
            state: Mutex::new(State {
                configuration: None,
                resources: Resources::default(),
                revision: None,
                counts: BTreeMap::new(),
                error: None,
            }),
            lease: Mutex::new(Some(CONTROLLERS.reserve(1)?)),
        }))
    }
    pub(crate) async fn apply_files(
        self: &Arc<Self>,
        configuration: Arc<Document>,
        resources: Resources,
    ) -> Result<u64> {
        let _operation = self.authority.enter(self.authority.identity())?;
        let _serial = tokio::select! { biased; _=self.authority.canceled()=>return Err(Error::Cancelled), serial=self.control.lock()=>serial };
        self.authority.check(self.authority.identity())?;
        let result = self.apply_files_inner(configuration, resources).await;
        if let Err(error) = result
            && let Ok(mut state) = self.state.lock()
        {
            state.error = Some(error.public_info());
        }
        result
    }
    async fn apply_files_inner(
        self: &Arc<Self>,
        configuration: Arc<Document>,
        resources: Resources,
    ) -> Result<u64> {
        let owner = self.owner.upgrade().ok_or(Error::InvalidState)?;
        let revision = owner.network_snapshot()?.revision;
        let purpose = if self.candidate {
            crate::configuration::PreparationPurpose::VpnCandidate
        } else {
            crate::configuration::PreparationPurpose::Configuration
        };
        configuration.preparation_preflight(purpose)?;
        resources.validate()?;
        let doc = configuration.clone();
        let files = resources.clone();
        let operation = self.authority.enter(self.authority.identity())?;
        let loaded = tokio::task::spawn_blocking(move || {
            let _operation = operation;
            load_files(&doc, &files)
        })
        .await
        .map_err(|_| Error::Internal)??;
        self.authority.check(self.authority.identity())?;
        let transport = configuration.clone().prepare_transport(
            loaded.geoip.as_ref().map(|s| s.database()).transpose()?,
            purpose,
        )?;
        let resolver = tokio::select! { biased;
            _=self.authority.canceled()=>return Err(Error::Cancelled),
            resolver=crate::dns::Resolver::from_main(configuration.dns_settings(),transport.outbounds.clone(),self.tls.tls_config())=>resolver?,
        };
        drop(transport);
        let doc = configuration.clone();
        let dns = resolver.clone();
        let operation = self.authority.enter(self.authority.identity())?;
        let prepared = tokio::task::spawn_blocking(move || {
            let _operation = operation;
            let mut prepared = doc.prepare_ready(loaded.sources, dns, loaded.geoip, purpose)?;
            prepared.plugins = loaded.assets.bundle;
            prepared.scripts = loaded.assets.scripts;
            Ok::<_, Error>((prepared, loaded.counts))
        })
        .await
        .map_err(|_| Error::Internal)
        .and_then(|result| result);
        let result = match prepared {
            Ok((prepared, counts)) => self.authority.with_active(self.authority.identity(), || {
                let mut state = self.state.lock().map_err(|_| Error::Internal)?;
                let next = owner.commit_provider_configuration(
                    revision,
                    prepared,
                    self.traffic.clone(),
                    || Ok(()),
                )?;
                state.configuration = Some(configuration);
                state.resources = resources;
                state.revision = Some(next);
                state.counts = counts;
                state.error = None;
                Ok(next)
            }),
            Err(error) => Err(error),
        };
        if let Err(error) = result {
            if let Some(resolver) = resolver {
                let _ = crate::error::diagnostics::observe(
                    resolver.close().await,
                    crate::error::diagnostics::DiagnosticSite::Cleanup,
                );
            }
            if let Ok(mut state) = self.state.lock() {
                state.error = Some(error.public_info());
            }
        }
        result
    }
    pub(crate) fn status(&self) -> Result<Status> {
        let active = self.authority.check(self.authority.identity()).is_ok();
        let state = self.state.lock().map_err(|_| Error::Internal)?;
        let providers = state
            .configuration
            .as_ref()
            .map(|doc| {
                doc.provider_settings()
                    .iter()
                    .map(|p| ProviderStatus {
                        name: p.name.clone(),
                        enabled: p.enable,
                        rules: state.counts.get(&p.name).map_or(0, |v| v.0),
                        ignored: state.counts.get(&p.name).map_or(0, |v| v.1),
                        last_success: None,
                        last_slot: None,
                    })
                    .collect()
            })
            .unwrap_or_default();
        Ok(Status {
            active,
            revision: state.revision,
            providers,
            error_info: state.error,
            read_only: true,
        })
    }
    /// 资源下载与更新属于宿主；必须由宿主显式重新提交文件清单，不使用历史路径悄悄刷新。
    pub(crate) async fn refresh(self: &Arc<Self>, _name: Option<String>) -> Result<u64> {
        Err(Error::Unsupported)
    }
    pub(crate) async fn refresh_geoip(self: &Arc<Self>) -> Result<u64> {
        Err(Error::Unsupported)
    }
    pub(crate) async fn close(self: &Arc<Self>) -> Result<()> {
        self.request_close()?;
        self.authority.wait_closed().await
    }
    pub(crate) fn request_close(self: &Arc<Self>) -> Result<()> {
        self.traffic.revoke()?;
        if self.authority.revoke()? {
            let me = self.clone();
            crate::lifecycle::runtime()?.spawn(async move {
                let idle = me.authority.wait_idle().await;
                let released = (|| {
                    let mut state = me.state.lock().map_err(|_| Error::Internal)?;
                    state.configuration = None;
                    state.resources = Resources::default();
                    state.counts.clear();
                    me.lease.lock().map_err(|_| Error::Internal)?.take();
                    Ok(())
                })();
                let traffic = me
                    .traffic
                    .wait_idle()
                    .await
                    .and_then(|_| me.traffic.complete(Ok(())));
                let result = idle.and(released).and(traffic);
                let _ = crate::error::diagnostics::observe(
                    me.authority.complete(result),
                    crate::error::diagnostics::DiagnosticSite::Cleanup,
                );
            });
        }
        Ok(())
    }
}
struct Loaded {
    sources: Vec<Source>,
    counts: BTreeMap<String, (usize, usize)>,
    geoip: Option<Arc<crate::geoip::Service>>,
    assets: crate::configuration::plugins::Assets,
    _leases: Vec<ByteLease>,
}
fn load_files(document: &Arc<Document>, resources: &Resources) -> Result<Loaded> {
    let geoip = match (document.geoip_url(), resources.geoip.as_ref()) {
        (Some(url), Some(path)) => Some(Arc::new(crate::geoip::Service::from_file(
            crate::geoip::Config {
                geoip_url: Some(url.into()),
            },
            path,
        )?)),
        (None, None) => None,
        _ => return Err(Error::InvalidInput),
    };
    let enabled: Vec<_> = document
        .provider_settings()
        .iter()
        .filter(|p| p.enable)
        .collect();
    if enabled.len() != resources.rules.len() {
        return Err(Error::InvalidInput);
    }
    let mut sources = Vec::new();
    let mut counts = BTreeMap::new();
    let mut leases = Vec::new();
    let nodes = document.provider_nodes();
    for provider in enabled {
        let file = resources
            .rules
            .iter()
            .find(|file| file.name == provider.name)
            .ok_or(Error::InvalidInput)?;
        let opened = crate::storage::ReadOnlyFile::open(&file.path)?;
        let size = opened.size()?;
        if size > BODY_LIMIT {
            return Err(Error::ResourceLimit);
        }
        let lease = READ_MEMORY.reserve(
            size.checked_mul(2)
                .and_then(|v| v.checked_add(4096))
                .ok_or(Error::ResourceLimit)?,
        )?;
        let text = opened.read(size)?;
        let (policy, outbound) = match provider.policy.as_str() {
            "direct" => (crate::routing::Policy::Direct, None),
            "reject" => (crate::routing::Policy::Reject, None),
            name => (crate::routing::Policy::Proxy, Some(name.into())),
        };
        let parsed = document::parse(&text, policy, outbound, geoip.is_some(), &nodes)?;
        counts.insert(provider.name.clone(), (parsed.rules.len(), parsed.ignored));
        let id = ring::digest::digest(&ring::digest::SHA256, provider.name.as_bytes())
            .as_ref()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        sources.push(Source {
            id,
            layer: Layer::Remote,
            enabled: true,
            rules: parsed.rules,
        });
        leases.push(lease);
        leases.push(parsed._memory);
    }
    let assets = crate::configuration::plugins::load_local(document, resources)?;
    Ok(Loaded {
        sources,
        counts,
        geoip,
        assets,
        _leases: leases,
    })
}
/// 正文预验证只返回数量；不能创建来源、发布规则或获得调试权限。
#[derive(Serialize)]
pub struct Validation {
    pub accepted: usize,
    pub ignored: usize,
}
pub fn validate_rules(
    yaml: &str,
    policy: crate::routing::Policy,
    outbound: Option<String>,
) -> Result<Validation> {
    if policy == crate::routing::Policy::Proxy {
        crate::routing::outbound_name(outbound.as_deref().ok_or(Error::InvalidInput)?)?;
    } else if outbound.is_some() {
        return Err(Error::InvalidInput);
    }
    let names = outbound.iter().map(String::as_str).collect::<Vec<_>>();
    let parsed = document::parse(yaml, policy, outbound.clone(), false, &names)?;
    Ok(Validation {
        accepted: parsed.rules.len(),
        ignored: parsed.ignored,
    })
}

#[cfg(test)]
#[path = "../tests/test_providers.rs"]
mod test_providers;
