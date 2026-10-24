//! DNS 客户端共用层：固定上游、加密传输、DNSSEC、正负 TTL 缓存及资源预算。
//! Hickory 负责标准报文/密码学验证；不读取系统 DNS 或 hosts，不隐式降级。
mod config;
pub mod fake_ip;
pub(crate) mod logging;
mod main;
pub mod observation;
mod runtime;
pub mod service;
mod wire;
use crate::lifecycle::session::{Authority, Operation, Phase};
use crate::{
    budget::{ByteBudget, ByteLease},
    error::{Error, Result},
};
pub use config::{Config, Server};
use hickory_resolver::{
    Resolver as UpstreamResolver,
    net::{DnsError, NetError},
    proto::{
        dnssec::Proof,
        rr::{RData, RecordType},
    },
};
use runtime::ManagedRuntime;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    str::FromStr,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::sync::{RwLock, Semaphore};
static METADATA_BUDGET: ByteBudget = ByteBudget::new(16 * 1024 * 1024);
const MAX_CACHE: usize = 128;
const MAX_RESULT_BYTES: usize = 16 * 1024;
static CACHE_BUDGET: ByteBudget = ByteBudget::new(16 * 1024 * 1024);
static CLIENTS: Semaphore = Semaphore::const_new(16);
static QUERIES: Semaphore = Semaphore::const_new(16);

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Query {
    pub name: String,
    pub query_type: String,
}
#[derive(Debug, Clone, Serialize)]
pub struct Record {
    pub name: String,
    pub record_type: String,
    pub ttl: u32,
    pub data: String,
}
#[derive(Debug, Clone, Serialize)]
pub struct Answer {
    pub name: String,
    pub query_type: String,
    pub records: Vec<Record>,
    pub addresses: Vec<String>,
    pub ttl: u32,
    pub rcode: u16,
    pub validation: String,
    pub from_cache: bool,
    pub duration_ms: u64,
    #[serde(skip)]
    wire: Vec<u8>,
}
struct Cached {
    answer: Answer,
    expires: Instant,
    _lease: ByteLease,
}
/// 所有上游显式配置；实例最多 16 个，每次查询有总期限，缓存不提高 TTL。
struct Resources {
    inner: Vec<UpstreamResolver<ManagedRuntime>>,
    runtime: ManagedRuntime,
    permit: tokio::sync::SemaphorePermit<'static>,
}
/// 一次已准入的内核事务令牌；只在内核内部按作用域持有，不交给前端。
/// 令牌允许同一建连事务在退休后完成后续 DNS 与提交，但不能越过显式关闭。
pub(crate) struct ResolverOperation {
    identity: crate::lifecycle::session::Identity,
    _active: Operation,
    _admission: Operation,
}
pub struct Resolver {
    resources: RwLock<Option<Resources>>,
    services: Mutex<Vec<Arc<service::Control>>>,
    binding: Mutex<Option<Arc<Authority>>>,
    authority: Arc<Authority>,
    // 查询准入与 DNS 驱动权限分离：退休只关闭准入，已登记查询仍使用有效驱动完成。
    // 不能用 Arc 计数代替在途查询计数，也不能在配置替换时直接撤销驱动权限。
    admission: Arc<Authority>,
    tls: Option<rustls::ClientConfig>,
    anchors: Option<Arc<hickory_resolver::proto::dnssec::TrustAnchors>>,
    _metadata: ByteLease,
    config: Config,
    main: Option<Arc<main::Plan>>,
    fake_ip: Option<fake_ip::Table>,
    cache: Mutex<BTreeMap<(String, String), Cached>>,
    observation: std::sync::Arc<Mutex<Option<observation::Observation>>>,
}
impl Resolver {
    pub fn new(config: Config) -> Result<Self> {
        Self::build(config, None, None)
    }
    pub fn with_tls_config(config: Config, tls: rustls::ClientConfig) -> Result<Self> {
        Self::build(config, Some(tls), None)
    }
    /// 显式私有 DNSSEC 根信任锚，仅影响此解析器；普通客户端使用内置根锚。
    pub fn with_trust_anchors(
        config: Config,
        anchors: hickory_resolver::proto::dnssec::TrustAnchors,
    ) -> Result<Self> {
        Self::build(config, None, Some(std::sync::Arc::new(anchors)))
    }
    fn build(
        config: Config,
        tls: Option<rustls::ClientConfig>,
        anchors: Option<std::sync::Arc<hickory_resolver::proto::dnssec::TrustAnchors>>,
    ) -> Result<Self> {
        config.validate()?;
        crate::lifecycle::runtime()?;
        let metadata = METADATA_BUDGET.reserve(config.allocated_bytes()?)?;
        let authority = Authority::new()?;
        let runtime = ManagedRuntime::new(authority.clone());
        let permit = CLIENTS.try_acquire().map_err(|_| Error::ResourceLimit)?;
        let mut inner = Vec::with_capacity(config.servers.len());
        for server in &config.servers {
            let single = Config {
                servers: vec![server.clone()],
                timeout_ms: config.timeout_ms,
                dnssec: config.dnssec,
                fake_ip: None,
                service: None,
            };
            let (servers, mut options) = single.build()?;
            options.timeout = Duration::from_millis(config.timeout_ms);
            let mut builder = UpstreamResolver::builder_with_config(servers, runtime.clone())
                .with_options(options);
            if let Some(mut tls) = tls.clone() {
                tls.alpn_protocols.clear();
                builder = builder.with_tls_config(tls);
            }
            if let Some(anchors) = &anchors {
                builder = builder.with_trust_anchor(anchors.clone());
            }
            inner.push(builder.build().map_err(|_| Error::InvalidInput)?);
        }
        let fake_ip = config
            .fake_ip
            .clone()
            .map(fake_ip::Table::new)
            .transpose()?;
        Ok(Self {
            resources: RwLock::new(Some(Resources {
                inner,
                runtime,
                permit,
            })),
            services: Mutex::new(Vec::new()),
            binding: Mutex::new(None),
            authority,
            admission: Authority::new()?,
            tls,
            anchors,
            _metadata: metadata,
            observation: observation::shared(&config)?,
            fake_ip,
            config,
            main: None,
            cache: Mutex::new(BTreeMap::new()),
        })
    }
    pub(crate) fn identity(&self) -> crate::lifecycle::session::Identity {
        self.authority.identity()
    }
    /// 旧引用不能发起新操作；已准入事务通过专用令牌完成，不走此新准入检查。
    pub(crate) fn ensure_active(&self) -> Result<()> {
        self.admission.check(self.admission.identity())?;
        self.ensure_running()
    }
    /// 只供已经登记的查询完成检查使用；退休不撤销它们的执行权，显式关闭仍会撤销。
    fn ensure_running(&self) -> Result<()> {
        self.authority.check(self.authority.identity())?;
        if let Some(owner) = self.binding_authority()? {
            owner.check(owner.identity())?;
        }
        if let Some(table) = &self.fake_ip {
            table.ensure_active()?;
        }
        Ok(())
    }
    pub(crate) fn binding_authority(&self) -> Result<Option<Arc<Authority>>> {
        Ok(self.binding.lock().map_err(|_| Error::Internal)?.clone())
    }
    pub(crate) fn operation(&self) -> Result<ResolverOperation> {
        // 登记与退休互斥；一旦获准，查询可在退休后继续，关闭会话则仍立即取消。
        let admission = self.admission.enter(self.admission.identity())?;
        self.ensure_running()?;
        Ok(ResolverOperation {
            identity: self.identity(),
            _active: self.authority.enter(self.authority.identity())?,
            _admission: admission,
        })
    }
    pub(crate) fn with_active<T>(&self, commit: impl FnOnce() -> Result<T>) -> Result<T> {
        self.admission.with_active(self.admission.identity(), || {
            self.authority
                .with_active(self.authority.identity(), commit)
        })
    }
    /// 网络配置提交时同步关闭旧实例准入；清理不依赖外部旧快照释放。
    /// 新旧解析器相同的提交不得调用此方法。已登记查询按原有总期限完成后，
    /// 内核主动 close 并等待监听/驱动释放；已建立的业务连接不归解析器所有。
    pub(crate) fn retire(self: &Arc<Self>) -> Result<()> {
        let executor = crate::lifecycle::runtime()?;
        // 无外部提交的退休保持幂等；已经关闭的旧解析器不能阻止新实例替换。
        // 带缓存提交的 retire_after 仍严格验证旧准入，失败不执行文件提交。
        if self.admission.revoke()? {
            self.schedule_retirement(executor);
        }
        Ok(())
    }
    /// 缓存原子提交失败不退休旧 DNS；成功后关闭准入并由内核等待在途查询，清理与旧 Arc 无关。
    pub(crate) fn retire_after(
        self: &Arc<Self>,
        commit: impl FnOnce() -> Result<()>,
    ) -> Result<()> {
        let executor = crate::lifecycle::runtime()?;
        if self.admission.revoke_after(commit)? {
            self.schedule_retirement(executor);
        }
        Ok(())
    }
    fn schedule_retirement(self: &Arc<Self>, executor: &tokio::runtime::Runtime) {
        let own = self.clone();
        executor.spawn(async move {
            let result = async {
                own.admission.wait_idle().await?;
                own.close().await
            }
            .await;
            let _ = crate::error::diagnostics::observe(
                result,
                crate::error::diagnostics::DiagnosticSite::Cleanup,
            );
        });
    }
    pub(crate) fn check_operation(&self, operation: &ResolverOperation) -> Result<()> {
        if !self.owns_operation(operation) {
            return Err(Error::InvalidState);
        }
        self.ensure_running()
    }
    pub(crate) fn owns_operation(&self, operation: &ResolverOperation) -> bool {
        operation.identity == self.identity()
    }
    /// 仅接受此解析器先前登记的事务令牌；不能凭旧 Arc 在退休后新建事务。
    pub(crate) fn commit_operation<T>(
        &self,
        operation: &ResolverOperation,
        commit: impl FnOnce() -> Result<T>,
    ) -> Result<T> {
        if operation.identity != self.identity() {
            return Err(Error::InvalidState);
        }
        self.authority.with_active(operation.identity, commit)
    }
    pub(crate) async fn resolve_operation(
        &self,
        query: Query,
        synthetic: bool,
        operation: &ResolverOperation,
    ) -> Result<Answer> {
        self.resolve_logged(query, synthetic, Some(operation), None)
            .await
    }
    /// 日志作用域属于本次调用，不改变解析器归属，不保存历史或裁决查询结果。
    pub(crate) async fn resolve_logged(
        &self,
        query: Query,
        synthetic: bool,
        operation: Option<&ResolverOperation>,
        scope: Option<logging::Scope>,
    ) -> Result<Answer> {
        let mut trace = logging::Trace::begin(scope, &query);
        let result = async {
            let acquired;
            let operation = if let Some(operation) = operation {
                operation
            } else {
                acquired = self.operation()?;
                &acquired
            };
            self.check_operation(operation)?;
            self.resolve_admitted(query, synthetic, trace.as_ref())
                .await
        }
        .await;
        if let Some(trace) = trace.as_mut() {
            trace.finish(&result);
        }
        result
    }
    pub(crate) async fn canceled(&self) {
        self.authority.canceled().await;
    }
    pub(crate) fn is_closed(&self) -> Result<bool> {
        Ok(self.authority.phase()? == Phase::Closed)
    }
    /// 新会话由内核从已验证元数据创建新代次，前端不能恢复旧对象权限。
    pub(crate) fn replacement(&self) -> Result<Self> {
        let mut replacement =
            Self::build(self.config.clone(), self.tls.clone(), self.anchors.clone())?;
        replacement.main = self
            .main
            .as_ref()
            .map(|p| p.replacement(replacement.authority.clone()).map(Arc::new))
            .transpose()?;
        Ok(replacement)
    }
    /// 撤销立即发生，清理任务由内核托管；调用方取消等待不会取消实际清理。
    pub async fn close(self: &Arc<Self>) -> Result<()> {
        // 显式关闭与退休不同：同时撤销准入和驱动，正在查询者按原契约取消。
        self.admission.revoke()?;
        if self.authority.revoke()? {
            let own = self.clone();
            crate::lifecycle::runtime()?.spawn(async move {
                let result = own.cleanup().await;
                // 两套权限都在真实清理完成后结束；不能让准入永远停留在 Closing。
                let result = own.admission.complete(result).and(result);
                let _ = crate::error::diagnostics::observe(
                    own.authority.complete(result),
                    crate::error::diagnostics::DiagnosticSite::Cleanup,
                );
            });
        }
        self.authority.wait_closed().await
    }
    pub(crate) fn register_service(&self, service: Arc<service::Control>) -> Result<()> {
        self.with_active(|| {
            let mut services = self.services.lock().map_err(|_| Error::Internal)?;
            services.retain(|service| service.running());
            if services.len() >= 4 {
                return Err(Error::ResourceLimit);
            }
            services.push(service);
            Ok(())
        })
    }
    async fn cleanup(&self) -> Result<()> {
        // 先取消监听与连接子任务，再等待在途查询，避免监听持有旧解析器延长资源占用。
        let services = std::mem::take(&mut *self.services.lock().map_err(|_| Error::Internal)?);
        for service in services {
            service.stop().await?;
        }
        // DNS 专用节点连接本身持有 authority 操作令牌，必须先停止连接，
        // 再等待令牌归零；反过来会等待尚未收到关闭命令的空闲连接。
        if let Some(plan) = &self.main {
            plan.close().await?;
        }
        self.authority.wait_idle().await?;
        // 事务令牌先释放驱动计数再释放准入计数，两个计数都归零后才能报告完成。
        self.admission.wait_idle().await?;
        let resources = self.resources.write().await.take();
        if let Some(Resources {
            inner,
            runtime,
            permit,
        }) = resources
        {
            drop(inner);
            runtime.stop().await?;
            drop(permit);
        }
        Ok(())
    }
    /// 只读最近一次默认服务器解析；读取不会查询 DNS。缓存命中为 0 ms，合成映射不更新。
    pub fn last_query(&self) -> Result<Option<observation::Observation>> {
        self.ensure_active()?;
        Ok(self
            .observation
            .lock()
            .map_err(|_| Error::Internal)?
            .clone())
    }
    fn observe(&self, duration_ms: u64, from_cache: bool, successful: bool) -> Result<()> {
        *self.observation.lock().map_err(|_| Error::Internal)? = Some(observation::Observation {
            duration_ms,
            from_cache,
            successful,
        });
        Ok(())
    }
    pub async fn serve(self: std::sync::Arc<Self>) -> Result<service::Service> {
        self.serve_logged(None).await
    }
    pub(crate) async fn serve_logged(
        self: Arc<Self>,
        scope: Option<logging::Scope>,
    ) -> Result<service::Service> {
        self.ensure_active()?;
        let config = self.config.service.clone().ok_or(Error::InvalidState)?;
        service::Service::start_logged(config, self, scope).await
    }
    pub fn reverse_fake_ip(&self, address: &str) -> Result<Option<String>> {
        self.ensure_active()?;
        self.fake_ip
            .as_ref()
            .ok_or(Error::Unsupported)?
            .reverse(address)
    }
    pub fn reset_fake_ip(&self) -> Result<()> {
        self.ensure_active()?;
        self.fake_ip.as_ref().ok_or(Error::Unsupported)?.reset()
    }
    pub fn clear(&self) -> Result<()> {
        self.ensure_active()?;
        self.cache.lock().map_err(|_| Error::Internal)?.clear();
        Ok(())
    }
    /// TUN 原始包无宿主代次；显式绑定后合成池同进程不复用旧地址。
    pub(crate) fn bind_tun_fake_ip(&self, owner: &Arc<Authority>) -> Result<()> {
        self.with_active(|| {
            let mut binding = self.binding.lock().map_err(|_| Error::Internal)?;
            if binding.is_some() {
                return Err(Error::InvalidState);
            }
            if let Some(table) = &self.fake_ip {
                table.bind_tun(owner)?;
            }
            *binding = Some(owner.clone());
            Ok(())
        })
    }
    pub fn fake_ip_generation(&self) -> Option<u64> {
        self.fake_ip.as_ref().map(fake_ip::Table::generation)
    }
    pub fn is_fake_ip(&self, address: std::net::IpAddr) -> bool {
        self.fake_ip
            .as_ref()
            .is_some_and(|table| table.contains(address))
    }
    pub fn restore_fake_ip(&self, address: &str, generation: u64) -> Result<String> {
        self.ensure_active()?;
        let table = self.fake_ip.as_ref().ok_or(Error::Unsupported)?;
        if generation != table.generation() {
            return Err(Error::InvalidState);
        }
        table.reverse(address)?.ok_or(Error::InvalidState)
    }
    /// 逐个实测主配置上游，绕过 hosts/Fake-IP/前台查询缓存；不以另一上游成功掩盖失败。
    pub(crate) async fn preflight_upstreams(
        &self,
        name: &str,
        strategy: crate::configuration::models::IpStrategy,
        scope: Option<logging::Scope>,
    ) -> Result<usize> {
        use crate::configuration::models::IpStrategy;
        self.ensure_running()?;
        let binding = self.binding_authority()?;
        let _binding = binding
            .as_ref()
            .map(|a| a.enter(a.identity()))
            .transpose()?;
        let _operation = self.authority.enter(self.authority.identity())?;
        let _permit = QUERIES.try_acquire().map_err(|_| Error::ResourceLimit)?;
        let (name, _) = validated_query(Query {
            name: name.to_owned(),
            query_type: "A".into(),
        })?;
        if name == "localhost" || name.ends_with(".localhost") {
            return Err(Error::InvalidInput);
        }
        let fqdn = format!("{name}.");
        let families: &[RecordType] = match strategy {
            IpStrategy::OnlyIpv4 => &[RecordType::A],
            IpStrategy::OnlyIpv6 => &[RecordType::AAAA],
            IpStrategy::PreferIpv4 => &[RecordType::A, RecordType::AAAA],
            IpStrategy::PreferIpv6 => &[RecordType::AAAA, RecordType::A],
        };
        let guard = self.resources.read().await;
        let resources = guard.as_ref().ok_or(Error::InvalidState)?;
        let plan = self.main.as_ref().ok_or(Error::InvalidState)?;
        let count = plan.len();
        resources
            .runtime
            .guard(async {
                for index in 0..count {
                    let mut result = Err(Error::DnsResponse);
                    for &kind in families {
                        let mut trace = logging::Trace::begin(
                            scope.clone(),
                            &Query {
                                name: name.clone(),
                                query_type: kind.to_string(),
                            },
                        );
                        if let Some(trace) = &trace {
                            trace.upstream(
                                index,
                                plan.setting(index).ok_or(Error::Internal)?,
                                None,
                            );
                        }
                        let lookup = tokio::time::timeout(
                            Duration::from_millis(self.config.timeout_ms),
                            plan.lookup(index, &fqdn, kind, &resources.runtime, scope.clone()),
                        )
                        .await
                        .map_err(|_| Error::DnsTimeout)
                        .and_then(|value| value);
                        let lookup = match lookup {
                            Ok(value) => value,
                            Err(error) => {
                                if let Some(trace) = &mut trace {
                                    trace.upstream(
                                        index,
                                        plan.setting(index).ok_or(Error::Internal)?,
                                        Some(error),
                                    );
                                    trace.finish_probe(&Err(error));
                                }
                                return Err(error);
                            }
                        };
                        result = match lookup {
                            Ok(answer)
                                if answer
                                    .answers()
                                    .iter()
                                    .any(|r| matches!(&r.data, RData::A(_) | RData::AAAA(_))) =>
                            {
                                Ok(())
                            }
                            Err(NetError::Timeout) => Err(Error::DnsTimeout),
                            Err(_) if self.config.dnssec => {
                                if let Some(trace) = &mut trace {
                                    trace.upstream(
                                        index,
                                        plan.setting(index).ok_or(Error::Internal)?,
                                        Some(Error::DnsValidation),
                                    );
                                    trace.finish_probe(&Err(Error::DnsValidation));
                                }
                                return Err(Error::DnsValidation);
                            }
                            _ => Err(Error::DnsResponse),
                        };
                        if let Some(trace) = &mut trace {
                            if let Err(error) = result {
                                trace.upstream(
                                    index,
                                    plan.setting(index).ok_or(Error::Internal)?,
                                    Some(error),
                                );
                            }
                            trace.finish_probe(&result);
                        }
                        if result.is_ok() {
                            break;
                        }
                    }
                    result?;
                }
                self.ensure_running()?;
                Ok(count)
            })
            .await
    }
    pub async fn resolve(&self, query: Query) -> Result<Answer> {
        self.resolve_inner(query, true).await
    }
    /// 真实解析复用上游、缓存和 DNSSEC；只跳过合成映射，不跳过验证。
    pub async fn resolve_real(&self, query: Query) -> Result<Answer> {
        self.resolve_inner(query, false).await
    }
    /// 只生成本地映射，排除项直接拒绝，不向上游查询。
    pub fn allocate_fake_ip(&self, query: Query) -> Result<Answer> {
        self.ensure_active()?;
        let (name, kind) = validated_query(query)?;
        if !matches!(kind, RecordType::A | RecordType::AAAA) {
            return Err(Error::Unsupported);
        }
        let _permit = QUERIES.try_acquire().map_err(|_| Error::ResourceLimit)?;
        self.synthetic_answer(&name, kind)?
            .ok_or(Error::FakeIpUnavailable)
    }
    fn synthetic_answer(&self, name: &str, kind: RecordType) -> Result<Option<Answer>> {
        if let Some(table) = &self.fake_ip
            && matches!(kind, RecordType::A | RecordType::AAAA)
            && let Some((address, from_cache)) = table.allocate(name, kind == RecordType::AAAA)?
        {
            return Ok(Some(Answer {
                wire: wire::synthetic(name, &address, kind)?,
                name: name.to_string(),
                query_type: kind.to_string(),
                records: vec![Record {
                    name: format!("{name}."),
                    record_type: kind.to_string(),
                    ttl: 60,
                    data: address.clone(),
                }],
                addresses: vec![address],
                ttl: 60,
                rcode: 0,
                validation: "synthetic".into(),
                from_cache,
                duration_ms: 0,
            }));
        }
        Ok(None)
    }
    async fn resolve_inner(&self, query: Query, synthetic: bool) -> Result<Answer> {
        let operation = self.operation()?;
        self.resolve_operation(query, synthetic, &operation).await
    }
    async fn resolve_admitted(
        &self,
        query: Query,
        synthetic: bool,
        trace: Option<&logging::Trace>,
    ) -> Result<Answer> {
        let binding = self.binding_authority()?;
        let _binding_operation = binding
            .as_ref()
            .map(|owner| owner.enter(owner.identity()))
            .transpose()?;
        tokio::select! {
            biased;
            _ = self.authority.canceled() => Err(Error::InvalidState),
            result = self.resolve_allowed(query, synthetic, trace) => {
                self.ensure_running()?;
                result
            }
        }
    }
    async fn resolve_allowed(
        &self,
        query: Query,
        synthetic: bool,
        trace: Option<&logging::Trace>,
    ) -> Result<Answer> {
        let guard = self.resources.read().await;
        let resources = guard.as_ref().ok_or(Error::InvalidState)?;
        let inner = &resources.inner;
        let (name, kind) = validated_query(query)?;
        let _permit = QUERIES.try_acquire().map_err(|_| Error::ResourceLimit)?;
        if let Some(plan) = &self.main
            && let Some(lookup) = plan.hosts(&name, kind)?
        {
            let answer = answer_from_hosts(name, kind, lookup)?;
            if let Some(trace) = trace {
                trace.hit(crate::logging::Event::DnsHostsHit, "hosts", &answer);
            }
            return Ok(answer);
        }
        if synthetic && let Some(answer) = self.synthetic_answer(&name, kind)? {
            if let Some(trace) = trace {
                trace.hit(crate::logging::Event::DnsSynthetic, "fake-ip", &answer);
            }
            return Ok(answer);
        }
        let key = (name.clone(), kind.to_string());
        let started = Instant::now();
        {
            let mut cache = self.cache.lock().map_err(|_| Error::Internal)?;
            cache.retain(|_, v| v.expires > started);
            if let Some(entry) = cache.get(&key) {
                let mut answer = entry.answer.clone();
                answer.from_cache = true;
                answer.duration_ms = 0;
                answer.ttl = entry.expires.saturating_duration_since(started).as_secs() as u32;
                for record in &mut answer.records {
                    record.ttl = record.ttl.min(answer.ttl);
                }
                self.observe(0, true, answer.rcode == 0 || answer.rcode == 3)?;
                if let Some(trace) = trace {
                    trace.hit(crate::logging::Event::DnsCacheHit, "cache", &answer);
                }
                return Ok(answer);
            }
        }
        let fqdn = if name == "." {
            name.clone()
        } else {
            format!("{name}.")
        };
        let count = self.main.as_ref().map_or(inner.len(), |p| p.len());
        let attempt = Duration::from_millis(self.config.timeout_ms);
        let lookup = resources
            .runtime
            .guard(async {
                tokio::time::timeout(attempt * count as u32 + Duration::from_millis(50), async {
                    let mut last = Err(NetError::Timeout);
                    for index in 0..count {
                        let label = self
                            .main
                            .as_ref()
                            .and_then(|p| p.setting(index))
                            .unwrap_or_else(|| match &self.config.servers[index] {
                                Server::Udp { address }
                                | Server::Tcp { address }
                                | Server::Dot { address, .. } => address,
                                Server::Doh { url, .. } => url,
                            });
                        if let Some(trace) = trace {
                            trace.upstream(index, label, None);
                        }
                        let attempt_started = Instant::now();
                        let attempted = tokio::time::timeout(attempt, async {
                            if let Some(plan) = &self.main {
                                plan.lookup(
                                    index,
                                    &fqdn,
                                    kind,
                                    &resources.runtime,
                                    trace.map(logging::Trace::scope),
                                )
                                .await
                            } else {
                                Ok(inner
                                    .get(index)
                                    .ok_or(Error::Internal)?
                                    .lookup(fqdn.clone(), kind)
                                    .await)
                            }
                        })
                        .await
                        .unwrap_or(Ok(Err(NetError::Timeout)));
                        last = match attempted {
                            Ok(value) => value,
                            Err(error) => {
                                if let Some(trace) = trace {
                                    trace.upstream(index, label, Some(error));
                                }
                                return Err(error);
                            }
                        };
                        if let Some(trace) = trace {
                            let error = match &last {
                                Err(NetError::Dns(DnsError::NoRecordsFound(value)))
                                    if !matches!(u16::from(value.response_code), 0 | 3) =>
                                {
                                    Some(Error::DnsResponse)
                                }
                                Ok(_) | Err(NetError::Dns(DnsError::NoRecordsFound(_))) => None,
                                Err(NetError::Timeout) => Some(Error::DnsTimeout),
                                Err(_) if self.config.dnssec => Some(Error::DnsValidation),
                                Err(_) => Some(Error::DnsResponse),
                            };
                            if let Some(error) = error {
                                trace.upstream(index, label, Some(error));
                            }
                        }
                        if index == 0 {
                            // 同步写入一条小统计，不创建后台任务；失败保留失败状态，不伪造零延迟成功。
                            self.observe(
                                attempt_started.elapsed().as_millis() as u64,
                                false,
                                last.is_ok()
                                    || matches!(
                                        &last,
                                        Err(NetError::Dns(DnsError::NoRecordsFound(_)))
                                    ),
                            )?;
                        }
                        if last.is_ok()
                            || matches!(&last, Err(NetError::Dns(DnsError::NoRecordsFound(_))))
                        {
                            break;
                        }
                        // DNSSEC 错误不以其他上游结果掩盖，防止验证失败被当成普通重试。
                        if self.config.dnssec && !matches!(&last, Err(NetError::Timeout)) {
                            break;
                        }
                    }
                    // 保留统计锁等内核失败，与上游查询结果分层；不能误标为 DNS 超时。
                    Ok::<_, Error>(last)
                })
                .await
                .map_err(|_| {
                    let _ = crate::error::diagnostics::observe(
                        self.observe(started.elapsed().as_millis() as u64, false, false),
                        crate::error::diagnostics::DiagnosticSite::DnsObservation,
                    );
                    Error::DnsTimeout
                })?
            })
            .await?;
        let mut answer = Answer {
            name,
            query_type: kind.to_string(),
            records: vec![],
            addresses: vec![],
            ttl: 0,
            rcode: 0,
            validation: if self.config.dnssec {
                "insecure"
            } else {
                "disabled"
            }
            .into(),
            from_cache: false,
            duration_ms: started.elapsed().as_millis() as u64,
            wire: vec![],
        };
        let mut cache_deadline = None;
        match lookup {
            Ok(lookup) => {
                answer.wire = wire::encode(lookup.message())?;
                if lookup.answers().len() > 128 {
                    return Err(Error::ResourceLimit);
                }
                let mut size = 0;
                let mut secure = true;
                let now = Instant::now();
                let deadline = lookup.valid_until().min(now + Duration::from_secs(3600));
                answer.ttl = deadline
                    .saturating_duration_since(now)
                    .as_millis()
                    .div_ceil(1000) as u32;
                cache_deadline = Some(deadline);
                for record in lookup.answers() {
                    let data = &record.data;
                    let text = data.to_string();
                    let owner = record.name.to_ascii();
                    size += text.len() + owner.len();
                    if size > MAX_RESULT_BYTES {
                        return Err(Error::ResourceLimit);
                    }
                    if matches!(data, RData::A(_) | RData::AAAA(_)) {
                        answer.addresses.push(text.clone());
                    }
                    secure &= record.proof == Proof::Secure;
                    answer.records.push(Record {
                        name: owner,
                        record_type: record.record_type().to_string(),
                        ttl: record.ttl,
                        data: text,
                    });
                }
                if self.config.dnssec && secure && !answer.records.is_empty() {
                    answer.validation = "secure".into();
                }
            }
            Err(NetError::Dns(DnsError::NoRecordsFound(no_records))) => {
                let mut message = hickory_resolver::proto::op::Message::query();
                message.metadata.response_code = no_records.response_code;
                if let Some(authorities) = &no_records.authorities {
                    if authorities.len() > 256 {
                        return Err(Error::ResourceLimit);
                    }
                    message.authorities = authorities.to_vec();
                }
                if !message
                    .authorities
                    .iter()
                    .any(|r| r.record_type() == RecordType::SOA)
                    && let Some(soa) = &no_records.soa
                {
                    message.add_authority(hickory_resolver::proto::rr::Record::from_rdata(
                        soa.name.clone(),
                        soa.ttl,
                        RData::SOA(soa.data.clone()),
                    ));
                }
                answer.wire = wire::encode(&message)?;
                answer.rcode = u16::from(no_records.response_code);
                if answer.rcode != 0 && answer.rcode != 3 {
                    return Err(Error::DnsResponse);
                }
                answer.ttl = no_records.negative_ttl.unwrap_or(0).min(300);
                if self.config.dnssec {
                    let secure = no_records.authorities.as_ref().is_some_and(|records| {
                        !records.is_empty() && records.iter().all(|r| r.proof == Proof::Secure)
                    });
                    answer.validation = if secure { "secure" } else { "insecure" }.into();
                    // 仅按已验证且裁剪过 TTL 的 SOA/NSEC 缓存否定结果。
                    if secure && let Some(soa) = &no_records.soa {
                        let proof_ttl = no_records
                            .authorities
                            .as_ref()
                            .and_then(|records| records.iter().map(|r| r.ttl).min())
                            .unwrap_or(0);
                        answer.ttl = soa.ttl.min(soa.data.minimum).min(proof_ttl).min(300);
                    }
                }
            }
            Err(NetError::Timeout) => return Err(Error::DnsTimeout),
            Err(_) if self.config.dnssec => return Err(Error::DnsValidation),
            Err(_) => return Err(Error::DnsResponse),
        }
        if answer.ttl > 0 {
            let mut cache = self.cache.lock().map_err(|_| Error::Internal)?;
            let now = Instant::now();
            cache.retain(|_, v| v.expires > now);
            if cache.len() >= MAX_CACHE
                && let Some(old) = cache
                    .iter()
                    .min_by_key(|(_, v)| v.expires)
                    .map(|(k, _)| k.clone())
            {
                cache.remove(&old);
            }
            // 返回文本上限 16 KiB；固定 128 KiB 覆盖最多 64 KiB 报文、展示记录和容器开销。
            if let Ok(lease) = crate::error::diagnostics::observe(
                CACHE_BUDGET.reserve(128 * 1024),
                crate::error::diagnostics::DiagnosticSite::DnsCache,
            ) {
                cache.insert(
                    key,
                    Cached {
                        expires: cache_deadline
                            .unwrap_or(now + Duration::from_secs(u64::from(answer.ttl))),
                        answer: answer.clone(),
                        _lease: lease,
                    },
                );
            }
        }
        Ok(answer)
    }
}
pub(super) fn normalize(name: &str) -> Result<String> {
    if name == "." {
        return Ok(".".into());
    }
    let name = name.strip_suffix('.').unwrap_or(name);
    if name.is_empty()
        || name.len() > 253
        || !name.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && label
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        })
    {
        return Err(Error::InvalidInput);
    }
    Ok(name.to_ascii_lowercase())
}

fn validated_query(query: Query) -> Result<(String, RecordType)> {
    let name = normalize(&query.name)?;
    if query.query_type.len() > 16 {
        return Err(Error::InvalidInput);
    }
    let kind = RecordType::from_str(&query.query_type.to_ascii_uppercase())
        .map_err(|_| Error::Unsupported)?;
    if matches!(
        kind,
        RecordType::ANY
            | RecordType::AXFR
            | RecordType::IXFR
            | RecordType::OPT
            | RecordType::Unknown(_)
    ) {
        return Err(Error::Unsupported);
    }
    Ok((name, kind))
}

impl Drop for Resolver {
    fn drop(&mut self) {
        if let Err(error) = self.admission.revoke() {
            crate::error::diagnostics::record(
                error,
                crate::error::diagnostics::DiagnosticSite::Cleanup,
            );
        }
        if let Err(error) = self.authority.revoke() {
            crate::error::diagnostics::record(
                error,
                crate::error::diagnostics::DiagnosticSite::Cleanup,
            );
        }
        // Drop 仅是未发布/未显式关闭实例的兜底，不承担配置退休的业务触发。
        // 服务任务可持有 Resolver；因此正常路径必须由 retire/close 主动打破所有权环。
        let services = match self.services.get_mut() {
            Ok(services) => std::mem::take(services),
            Err(poisoned) => std::mem::take(poisoned.into_inner()),
        };
        if let Some(Resources {
            inner,
            runtime,
            permit,
        }) = self.resources.get_mut().take()
        {
            let main = self.main.take();
            let owner = self.authority.clone();
            let admission = self.admission.clone();
            match crate::lifecycle::runtime() {
                Ok(executor) => {
                    executor.spawn(async move {
                        let mut result = Ok(());
                        for service in services {
                            if let Err(error) = service.stop().await {
                                result = Err(error);
                            }
                        }
                        if let Some(main) = main
                            && let Err(error) = main.close().await
                        {
                            result = Err(error);
                        }
                        drop(inner);
                        if let Err(error) = runtime.stop().await {
                            result = Err(error);
                        }
                        // OS 引导的阻塞调用不能强行终止；其操作令牌在真实退出前仍有效。
                        // 先排空再归还活动名额，不能在 Drop 兜底中提前宣告完成。
                        if let Err(error) = owner.wait_idle().await {
                            result = Err(error);
                        }
                        if let Err(error) = admission.wait_idle().await {
                            result = Err(error);
                        }
                        drop(permit);
                        let result = admission.complete(result).and(result);
                        let _ = crate::error::diagnostics::observe(
                            owner.complete(result),
                            crate::error::diagnostics::DiagnosticSite::Cleanup,
                        );
                    });
                }
                Err(error) => crate::error::diagnostics::record(
                    error,
                    crate::error::diagnostics::DiagnosticSite::Cleanup,
                ),
            }
        }
    }
}

#[cfg(test)]
#[path = "../../tests/test_dns_mod.rs"]
mod test_dns_mod;

fn answer_from_hosts(
    name: String,
    kind: RecordType,
    lookup: hickory_resolver::lookup::Lookup,
) -> Result<Answer> {
    let wire = wire::encode(lookup.message())?;
    let records = lookup
        .answers()
        .iter()
        .map(|r| Record {
            name: r.name.to_ascii(),
            record_type: r.record_type().to_string(),
            ttl: 60,
            data: r.data.to_string(),
        })
        .collect::<Vec<_>>();
    Ok(Answer {
        name,
        query_type: kind.to_string(),
        addresses: records.iter().map(|r| r.data.clone()).collect(),
        records,
        ttl: 60,
        rcode: 0,
        validation: "hosts".into(),
        from_cache: false,
        duration_ms: 0,
        wire,
    })
}
