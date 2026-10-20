//! rule-providers 完整生命周期：内核持有下载、设备日历、缓存批次与发布权限。
//! 公开只读状态不包含 URL、规则正文、认证或活动资源句柄。
mod cache;
mod document;
use crate::{
    budget::{ByteBudget, ByteLease},
    configuration::Document,
    error::{Error, ErrorInfo, Result},
    https::HttpsClient,
    lifecycle::session::Authority,
    outbound::Manager,
    routing::runtime::{Layer, Source},
};
use cache::{Data, Metadata, Text};
use serde::Serialize;
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{Arc, Mutex, Weak},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::sync::{Mutex as AsyncMutex, Notify};
const MAX_CONTROLLERS: usize = 4;
const RETRY_SECONDS: u64 = 60;
const MAX_TIME: u64 = 253402300799;
static CONTROLLERS: ByteBudget = ByteBudget::new(MAX_CONTROLLERS);
static DOWNLOADS: ByteBudget = ByteBudget::new(MAX_CONTROLLERS * cache::BODY_LIMIT);
#[derive(Serialize)]
/// 已提交提供者的只读摘要；规则条数为导入后、跨来源裁剪前的数量。
pub struct ProviderStatus {
    pub name: String,
    pub enabled: bool,
    pub rules: usize,
    pub ignored: usize,
    pub last_success: Option<u64>,
    pub last_slot: Option<i32>,
}
#[derive(Serialize)]
/// active 表示准入未撤销；关闭完成只能通过 Manager::stop().await 确认。
pub struct Status {
    pub active: bool,
    pub revision: Option<u64>,
    pub providers: Vec<ProviderStatus>,
    pub error_info: Option<ErrorInfo>,
    pub directory_synced: bool,
}
struct State {
    cache: Option<cache::Cache>,
    data: Data,
    configuration: Option<Arc<Document>>,
    revision: Option<u64>,
    counts: BTreeMap<String, (usize, usize)>,
    error: Option<ErrorInfo>,
    directory_synced: bool,
    last_cycle: Option<u64>,
    geoip_slot: Option<i32>,
    plugin_slots: BTreeMap<String, Option<i32>>,
}
pub(crate) struct Runtime {
    owner: Weak<Manager>,
    home: Option<PathBuf>,
    tls: HttpsClient,
    authority: Arc<Authority>,
    // 流量权限与控制提交锁分离，避免连接提交 State→Authority 与更新 Authority→State 倒序。
    traffic: Arc<Authority>,
    control: AsyncMutex<()>,
    state: Mutex<State>,
    worker: Mutex<Option<tokio::task::JoinHandle<()>>>,
    changed: Notify,
    _lease: Mutex<Option<ByteLease>>,
}
pub(crate) fn now() -> Result<u64> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| Error::Internal)?
        .as_secs();
    if now > MAX_TIME {
        return Err(Error::InvalidInput);
    }
    Ok(now)
}
pub(crate) fn slot(time: Option<&str>, now: u64) -> Result<Option<i32>> {
    let Some(time) = time else { return Ok(None) };
    let b = time.as_bytes();
    if b.len() != 5 || b[2] != b':' || ![0, 1, 3, 4].iter().all(|i| b[*i].is_ascii_digit()) {
        return Err(Error::InvalidInput);
    }
    let hour = (b[0] - b'0') as u32 * 10 + (b[1] - b'0') as u32;
    let minute = (b[3] - b'0') as u32 * 10 + (b[4] - b'0') as u32;
    if hour > 23 || minute > 59 {
        return Err(Error::InvalidInput);
    }
    let day = crate::platform::providers::local_day(now)?;
    Ok(Some(if day.seconds >= hour * 3600 + minute * 60 {
        day.date
    } else {
        day.previous
    })
    .filter(|d| *d != 0))
}
impl Runtime {
    pub(crate) fn new(owner: &Arc<Manager>, tls: HttpsClient, home: PathBuf) -> Result<Arc<Self>> {
        Self::with_storage(owner, tls, Some(home))
    }
    /// 候选复用同一资产编排，但没有磁盘目录和自动刷新任务。
    pub(crate) fn candidate(owner: &Arc<Manager>, tls: HttpsClient) -> Result<Arc<Self>> {
        Self::with_storage(owner, tls, None)
    }
    fn with_storage(
        owner: &Arc<Manager>,
        tls: HttpsClient,
        home: Option<PathBuf>,
    ) -> Result<Arc<Self>> {
        crate::lifecycle::runtime()?;
        let lease = CONTROLLERS.reserve(1)?;
        let tls = tls.with_network(Arc::new(Manager::default()));
        Ok(Arc::new(Self {
            owner: Arc::downgrade(owner),
            home,
            tls,
            authority: Authority::new()?,
            traffic: Authority::new()?,
            control: AsyncMutex::new(()),
            state: Mutex::new(State {
                cache: None,
                data: Data::default(),
                configuration: None,
                revision: None,
                counts: BTreeMap::new(),
                error: None,
                directory_synced: true,
                last_cycle: None,
                geoip_slot: None,
                plugin_slots: BTreeMap::new(),
            }),
            worker: Mutex::new(None),
            changed: Notify::new(),
            _lease: Mutex::new(Some(lease)),
        }))
    }
    fn purpose(&self) -> crate::configuration::PreparationPurpose {
        if self.home.is_some() {
            crate::configuration::PreparationPurpose::Configuration
        } else {
            crate::configuration::PreparationPurpose::VpnCandidate
        }
    }
    pub(crate) fn launch(self: &Arc<Self>) -> Result<()> {
        if self.home.is_none() {
            return Err(Error::InvalidState);
        }
        let mut task = self.worker.lock().map_err(|_| Error::Internal)?;
        if task.is_some() {
            return Ok(());
        }
        let weak = Arc::downgrade(self);
        let canceled = self.authority.clone();
        // 常驻计划属于内核运行时，不能随某次外部 Rust 请求的 Tokio 运行时销毁而消失。
        *task = Some(crate::lifecycle::runtime()?.spawn(async move {
            loop {
                let Some(runtime) = weak.upgrade() else { break };
                let result = runtime.automatic().await;
                if let Err(error) = result
                    && (error == Error::Cancelled
                        || runtime
                            .authority
                            .check(runtime.authority.identity())
                            .is_err())
                {
                    break;
                }
                // automatic 已记录本轮受控错误，避免后台重复审计。
                tokio::select! { _=canceled.canceled()=>break,_=runtime.changed.notified()=>(),_=tokio::time::sleep(Duration::from_secs(1))=>() }
            }
        }));
        Ok(())
    }
    fn record(&self, error: Error) {
        crate::error::diagnostics::record(
            error,
            crate::error::diagnostics::DiagnosticSite::BackgroundJob,
        );
        if let Ok(mut state) = self.state.lock() {
            state.error = Some(error.public_info())
        }
    }
    async fn initialize(self: &Arc<Self>) -> Result<()> {
        if self.home.is_none() {
            return Ok(());
        }
        if self
            .state
            .lock()
            .map_err(|_| Error::Internal)?
            .cache
            .is_some()
        {
            return Ok(());
        }
        let me = self.clone();
        let operation = self.authority.enter(self.authority.identity())?;
        tokio::task::spawn_blocking(move || {
            let _operation = operation;
            let (cache, data) = cache::Cache::open(me.home.as_deref().ok_or(Error::InvalidState)?)?;
            me.authority.with_active(me.authority.identity(), || {
                let mut state = me.state.lock().map_err(|_| Error::Internal)?;
                state.cache = Some(cache);
                state.data = data;
                Ok(())
            })
        })
        .await
        .map_err(|_| Error::Internal)?
    }
    /// 所有意图先登记，关闭能取消等待门锁/网络的请求，并等真正的阻塞编译释放完毕。
    pub(crate) async fn apply(
        self: &Arc<Self>,
        configuration: Arc<Document>,
        manual: Option<Option<String>>,
        force_geoip: bool,
    ) -> Result<u64> {
        let _operation = self.authority.enter(self.authority.identity())?;
        let result = tokio::select! {
            _=self.authority.canceled()=>Err(Error::Cancelled),
            result=async {let _serial=self.control.lock().await;if !configuration.provider_settings().is_empty()||configuration.geoip_url().is_some(){self.initialize().await?;}
                if manual.is_none() && !force_geoip {self.verify_cache().await?;}self.update(configuration,manual,force_geoip).await}=>result,
        };
        if let Err(error) = result {
            self.record(error)
        }
        self.changed.notify_one();
        result
    }
    fn due(&self, configuration: &Document, at: u64) -> Result<bool> {
        let state = self.state.lock().map_err(|_| Error::Internal)?;
        if state
            .last_cycle
            .is_some_and(|last| at < last.saturating_add(RETRY_SECONDS))
        {
            return Ok(false);
        }
        if configuration.geoip_url().is_some()
            && let Some(slot) = slot(configuration.geoip_time(), at)?
            && state.geoip_slot.is_none_or(|old| old < slot)
        {
            return Ok(true);
        }
        for p in configuration
            .provider_settings()
            .iter()
            .filter(|p| p.enable)
        {
            let key = cache::hash(p.url.as_bytes());
            if state
                .data
                .attempts
                .get(&key)
                .is_some_and(|last| at < last.saturating_add(RETRY_SECONDS))
            {
                continue;
            }
            if let Some(slot) = slot(p.update_time.as_deref(), at)?
                && state.data.entries.get(&p.name).is_none_or(|(m, _)| {
                    m.url_hash != key || m.last_slot.is_none_or(|last| last < slot)
                })
            {
                return Ok(true);
            }
        }
        configuration.assets_due(&state.plugin_slots, at)
    }
    async fn automatic(self: &Arc<Self>) -> Result<()> {
        let _operation = self.authority.enter(self.authority.identity())?;
        let result = tokio::select! {
            _=self.authority.canceled()=>Err(Error::Cancelled),
            result=async {
                let _serial=self.control.lock().await;
                // 到期条件在控制门内复核，排队的旧 tick 不能重复下载或清除上一轮失败。
                let configuration=self.state.lock().map_err(|_|Error::Internal)?.configuration.clone();
                let Some(configuration)=configuration else{return Ok(())};let at=now()?;
                if self.due(&configuration,at)? {
                    self.state.lock().map_err(|_|Error::Internal)?.last_cycle=Some(at);
                    self.verify_cache().await?;self.update(configuration,None,false).await?;
                }Ok(())
            }=>result,
        };
        if let Err(error) = result
            && error != Error::Cancelled
        {
            self.record(error)
        }
        result
    }
    async fn stage(self: &Arc<Self>, data: Data) -> Result<cache::Batch> {
        let me = self.clone();
        let operation = self.authority.enter(self.authority.identity())?;
        tokio::task::spawn_blocking(move || {
            let _operation = operation;
            me.authority.check(me.authority.identity())?;
            me.state
                .lock()
                .map_err(|_| Error::Internal)?
                .cache
                .as_ref()
                .ok_or(Error::InvalidState)?
                .stage(data)
        })
        .await
        .map_err(|_| Error::Internal)?
    }
    // 尝试记录也先原子落盘；失败不冒充成功，重启不会无间隔重复冲击下载源。
    async fn attempted(self: &Arc<Self>, key: String, at: u64) -> Result<()> {
        let me = self.clone();
        let operation = self.authority.enter(self.authority.identity())?;
        tokio::task::spawn_blocking(move || {
            let _operation = operation;
            me.authority.with_active(me.authority.identity(), || {
                let mut state = me.state.lock().map_err(|_| Error::Internal)?;
                let mut attempts = state.data.attempts.clone();
                if attempts.len() >= 128
                    && !attempts.contains_key(&key)
                    && let Some(old) = attempts
                        .iter()
                        .min_by_key(|(_, time)| *time)
                        .map(|(k, _)| k.clone())
                {
                    attempts.remove(&old);
                }
                attempts.insert(key, at);
                if me.home.is_some() {
                    state
                        .cache
                        .as_ref()
                        .ok_or(Error::InvalidState)?
                        .attempts(&attempts)?;
                }
                state.data.attempts = attempts;
                Ok(())
            })
        })
        .await
        .map_err(|_| Error::Internal)?
    }
    async fn update(
        self: &Arc<Self>,
        configuration: Arc<Document>,
        manual: Option<Option<String>>,
        force_geoip: bool,
    ) -> Result<u64> {
        let owner = self.owner.upgrade().ok_or(Error::InvalidState)?;
        let baseline = owner.network_snapshot()?.revision;
        if let Some(Some(name)) = &manual
            && !configuration
                .provider_settings()
                .iter()
                .any(|p| &p.name == name)
        {
            return Err(Error::InvalidInput);
        }
        let previous_database = if let Some(url) = configuration.geoip_url() {
            owner
                .network_snapshot()?
                .geoip
                .as_ref()
                .filter(|s| s.source_url() == Some(url))
                .map(|s| s.database_for_url(url))
                .transpose()?
        } else {
            None
        };
        let purpose = self.purpose();
        configuration.preparation_preflight(purpose)?;
        let doc = configuration.clone();
        let operation = self.authority.enter(self.authority.identity())?;
        let mut transport = tokio::task::spawn_blocking(move || {
            let _operation = operation;
            doc.prepare_download(previous_database, purpose)
        })
        .await
        .map_err(|_| Error::Internal)??;
        let resolver = crate::dns::Resolver::from_main(
            configuration.dns_settings(),
            transport.outbounds.clone(),
            self.tls.tls_config(),
        )
        .await?;
        transport.resolver = resolver.clone();
        // TLS 信任可来自系统/显式企业根；网络、源策略和恢复由内核构建，不复用客户端自报身份。
        let network = Arc::new(Manager::default());
        network.configure_provider_download(&transport, self.authority.clone())?;
        let client = self.tls.clone().with_network(network.clone());
        let geoip_slot = if configuration.geoip_url().is_some() {
            slot(configuration.geoip_time(), now()?)?
        } else {
            None
        };
        let result = async {
            let assets = if configuration.has_assets() {
                Some(
                    crate::configuration::plugins::load(
                        &configuration,
                        &client,
                        self.home.as_deref(),
                        manual.as_ref().is_some_and(Option::is_none),
                    )
                    .await?,
                )
            } else {
                None
            };
            let geoip = if let Some(url) = configuration.geoip_url() {
                let (previous, previous_slot) = {
                    let state = self.state.lock().map_err(|_| Error::Internal)?;
                    (owner.network_snapshot()?.geoip.clone(), state.geoip_slot)
                };
                if let Some(service) =
                    previous
                        .filter(|s| s.source_url() == Some(url))
                        .filter(|_| {
                            !force_geoip && (geoip_slot.is_none() || geoip_slot == previous_slot)
                        })
                {
                    Some(service)
                } else {
                    let service = Arc::new(crate::geoip::Service::new(crate::geoip::Config {
                        geoip_url: Some(url.into()),
                    })?);
                    service.refresh(&client).await?;
                    Some(service)
                }
            } else {
                crate::geoip::Warning::disabled();
                None
            };
            if let Some(service) = &geoip {
                // 库一旦 Ready，后续提供者下载必须使用已绑定国家规则，不能继续沿用冷启动自举索引。
                let mut ready = configuration
                    .clone()
                    .prepare_download(Some(service.database()?), purpose)?;
                ready.resolver = resolver.clone();
                network.configure_provider_download(&ready, self.authority.clone())?;
            }
            let mut loaded = self.load(&configuration, &client, manual).await?;
            let old = owner.network_snapshot()?.geoip.clone();
            let geoip_downloaded = geoip
                .as_ref()
                .is_some_and(|new| old.as_ref().is_none_or(|old| !Arc::ptr_eq(old, new)));
            if let Some(service) = &geoip
                && self.home.is_some()
            {
                let database = service.database()?;
                let path = self
                    .state
                    .lock()
                    .map_err(|_| Error::Internal)?
                    .cache
                    .as_ref()
                    .ok_or(Error::InvalidState)?
                    .directory
                    .join(cache::GEOIP_FILE);
                let metadata = if !geoip_downloaded {
                    loaded
                        .0
                        .geoip
                        .as_ref()
                        .map(|g| g.metadata.clone())
                        .ok_or(Error::GeoipIntegrityFailed)?
                } else {
                    Metadata {
                        url_hash: cache::hash(
                            configuration
                                .geoip_url()
                                .ok_or(Error::GeoipInitializationFailed)?
                                .as_bytes(),
                        ),
                        digest: cache::hash(database.bytes()),
                        fetched_at: now()?,
                        last_slot: geoip_slot,
                    }
                };
                loaded.0.geoip = Some(cache::GeoipFile {
                    metadata,
                    database: Some(database),
                    path,
                });
            }
            Ok((loaded, geoip, geoip_downloaded, assets))
        }
        .await;
        let stopped = network.stop_provider_download().await;
        let ((mut data, sources, counts, leases, downloaded), geoip, geoip_downloaded, mut assets) =
            match (result, stopped) {
                (Err(e), _) => {
                    if let Some(resolver) = &resolver {
                        let _ = crate::error::diagnostics::observe(
                            resolver.close().await,
                            crate::error::diagnostics::DiagnosticSite::Cleanup,
                        );
                    }
                    return Err(e);
                }
                (Ok(_), Err(e)) => return Err(e),
                (Ok(v), Ok(())) => v,
            };
        // 尝试日志单独提交过；候选基于最新日志，不能在主批次回滚计时状态。
        data.attempts = self
            .state
            .lock()
            .map_err(|_| Error::Internal)?
            .data
            .attempts
            .clone();
        let me = self.clone();
        let document = configuration.clone();
        let operation = self.authority.enter(self.authority.identity())?;
        let mut prepared = tokio::task::spawn_blocking(move || {
            let _operation = operation;
            let _leases = leases;
            me.authority.check(me.authority.identity())?;
            document.prepare_ready(sources, resolver, geoip, purpose)
        })
        .await
        .map_err(|_| Error::Internal)??;
        if let Some(assets) = &assets {
            prepared.plugins = assets.bundle.clone();
            prepared.scripts = assets.scripts.clone();
        }
        // 本地规则/策略编辑只重新编译；正文与元数据没有变化时不重写远程文件。
        let changed = {
            let state = self.state.lock().map_err(|_| Error::Internal)?;
            downloaded
                || geoip_downloaded
                || data.entries.len() != state.data.entries.len()
                || data.entries.iter().any(|(name, (meta, _))| {
                    state
                        .data
                        .entries
                        .get(name)
                        .is_none_or(|(old, _)| old != meta)
                })
        };
        let mut batch = if changed && self.home.is_some() {
            Some(self.stage(data.clone()).await?)
        } else {
            None
        };
        let plugin_slots = prepared.plugins.slots.clone();
        let revision = self.authority.with_active(self.authority.identity(), || {
            let mut state = self.state.lock().map_err(|_| Error::Internal)?;
            let revision = owner.commit_provider_configuration(
                baseline,
                prepared,
                self.traffic.clone(),
                || {
                    if let Some(assets) = &mut assets {
                        assets.commit()?;
                    }
                    if let Err(error) = batch.as_mut().map_or(Ok(()), |b| b.commit()) {
                        if let Some(assets) = &mut assets {
                            assets.rollback()?;
                        }
                        return Err(error);
                    }
                    Ok(())
                },
            )?;
            if let Some(assets) = &mut assets {
                assets.accept();
            }
            // 数据库仅由已发布服务/规则快照持有；磁盘元数据不延长内存或资源占用。
            if let Some(asset) = &mut data.geoip {
                asset.database = None;
            }
            state.data = data;
            state.configuration = Some(configuration);
            state.revision = Some(revision);
            state.counts = counts;
            state.geoip_slot = geoip_slot;
            state.plugin_slots = plugin_slots;
            // 仅真实下载成功可清除下载失败；本地编辑不能伪装后台源已恢复。
            if downloaded || geoip_downloaded {
                state.error = None;
            }
            Ok(revision)
        })?;
        if let Some(assets) = assets
            && let Err(error) = assets.sync()
        {
            self.record(error);
            self.state
                .lock()
                .map_err(|_| Error::Internal)?
                .directory_synced = false;
        }
        if let Some(batch) = batch {
            // 提交后不再 await：不能在已发布版本之后被外层取消误报为未提交。
            // 目录 fsync 失败用已提交状态明确报告，不回滚已对新请求可见的快照。
            let synced = batch.sync_parent().is_ok();
            self.state
                .lock()
                .map_err(|_| Error::Internal)?
                .directory_synced = synced;
            if !synced {
                self.record(Error::Network);
            }
        }
        Ok(revision)
    }
    async fn load(
        self: &Arc<Self>,
        configuration: &Arc<Document>,
        client: &HttpsClient,
        manual: Option<Option<String>>,
    ) -> Result<Loaded> {
        let mut data = self.state.lock().map_err(|_| Error::Internal)?.data.clone();
        data.entries.retain(|name, _| {
            configuration
                .provider_settings()
                .iter()
                .any(|p| p.name == *name)
        });
        let mut sources = Vec::new();
        let mut counts = BTreeMap::new();
        let mut leases = Vec::new();
        let mut total = 0usize;
        let mut downloaded = false;
        let at = now()?;
        let nodes = configuration.provider_nodes();
        for provider in configuration.provider_settings() {
            let hash = cache::hash(provider.url.as_bytes());
            let previous = data
                .entries
                .get(&provider.name)
                .filter(|(m, _)| m.url_hash == hash);
            let local_slot = slot(provider.update_time.as_deref(), at)?;
            let explicit = manual
                .as_ref()
                .is_some_and(|name| name.as_ref().is_none_or(|name| *name == provider.name));
            let retry = data
                .attempts
                .get(&hash)
                .is_none_or(|last| at >= last.saturating_add(RETRY_SECONDS));
            let due = provider.enable
                && local_slot.is_some_and(|slot| {
                    previous.is_none_or(|(m, _)| m.last_slot.is_none_or(|last| last < slot))
                })
                && retry;
            let import = provider.enable && previous.is_none();
            let download = explicit || due || import;
            let mut fetched = None;
            if download {
                self.attempted(hash.clone(), at).await?;
                let _download = DOWNLOADS.reserve(cache::BODY_LIMIT)?;
                let response = crate::subscription::fetch(client, &provider.url).await?;
                let text = String::from_utf8(response.body).map_err(|_| Error::InvalidInput)?;
                fetched = Some(Text::new(text)?);
                downloaded = true;
            }
            let text = fetched.as_ref().or_else(|| previous.map(|(_, text)| text));
            if let Some(text) = text {
                let (policy, outbound) = match provider.policy.as_str() {
                    "direct" => (crate::routing::Policy::Direct, None),
                    "reject" => (crate::routing::Policy::Reject, None),
                    node => (crate::routing::Policy::Proxy, Some(node.to_owned())),
                };
                let parsed = document::parse(
                    &text.text,
                    policy,
                    outbound,
                    configuration.geoip_url().is_some(),
                    &nodes,
                )?;
                let ignored = parsed.ignored;
                if provider.enable {
                    total = total
                        .checked_add(parsed.rules.len())
                        .ok_or(Error::ResourceLimit)?;
                    if total > crate::routing::runtime::MAX_RULES {
                        return Err(Error::ResourceLimit);
                    }
                    counts.insert(provider.name.clone(), (parsed.rules.len(), ignored));
                    sources.push(Source {
                        id: cache::hash(provider.name.as_bytes()),
                        layer: Layer::Remote,
                        enabled: true,
                        rules: parsed.rules,
                    });
                }
                if !provider.enable {
                    counts.insert(provider.name.clone(), (0, ignored));
                }
                leases.push(parsed._memory);
                if let Some(text) = fetched {
                    let meta = Metadata {
                        url_hash: hash,
                        digest: cache::hash(text.text.as_bytes()),
                        fetched_at: at,
                        last_slot: local_slot,
                    };
                    data.entries.insert(provider.name.clone(), (meta, text));
                }
            }
        }
        Ok((data, sources, counts, leases, downloaded))
    }
    pub(crate) fn status(&self) -> Result<Status> {
        // 发布路径按 Authority -> State 加锁；状态查询不得倒序持锁，避免与原子提交互相等待。
        // active 是本次权限观察值，不替代 stop().await 的关闭完成契约。
        let active = self.authority.check(self.authority.identity()).is_ok();
        let state = self.state.lock().map_err(|_| Error::Internal)?;
        let providers = state
            .configuration
            .as_ref()
            .map(|c| {
                c.provider_settings()
                    .iter()
                    .map(|p| {
                        let meta = state
                            .data
                            .entries
                            .get(&p.name)
                            .map(|(m, _)| m)
                            .filter(|m| m.url_hash == cache::hash(p.url.as_bytes()));
                        ProviderStatus {
                            name: p.name.clone(),
                            enabled: p.enable,
                            rules: state.counts.get(&p.name).map_or(0, |v| v.0),
                            ignored: state.counts.get(&p.name).map_or(0, |v| v.1),
                            last_success: meta.map(|m| m.fetched_at),
                            last_slot: meta.and_then(|m| m.last_slot),
                        }
                    })
                    .collect()
            })
            .unwrap_or_default();
        Ok(Status {
            active,
            revision: state.revision,
            providers,
            error_info: state.error,
            directory_synced: state.directory_synced,
        })
    }
    pub(crate) async fn refresh(self: &Arc<Self>, name: Option<String>) -> Result<u64> {
        let configuration = self
            .state
            .lock()
            .map_err(|_| Error::Internal)?
            .configuration
            .clone()
            .ok_or(Error::InvalidState)?;
        self.apply(configuration, Some(name), false).await
    }
    pub(crate) async fn refresh_geoip(self: &Arc<Self>) -> Result<u64> {
        let configuration = self
            .state
            .lock()
            .map_err(|_| Error::Internal)?
            .configuration
            .clone()
            .ok_or(Error::InvalidState)?;
        if configuration.geoip_url().is_none() {
            return Err(Error::InvalidState);
        }
        self.apply(configuration, None, true).await
    }
    async fn verify_cache(self: &Arc<Self>) -> Result<()> {
        let me = self.clone();
        let operation = self.authority.enter(self.authority.identity())?;
        tokio::task::spawn_blocking(move || {
            let _operation = operation;
            let state = me.state.lock().map_err(|_| Error::Internal)?;
            state
                .cache
                .as_ref()
                .map_or(Ok(()), |cache| cache.verify(&state.data))
        })
        .await
        .map_err(|_| Error::Internal)?
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
                let task = me
                    .worker
                    .lock()
                    .map_err(|_| Error::Internal)
                    .map(|mut t| t.take());
                let joined = match task {
                    Ok(Some(task)) => task.await.map_err(|_| Error::Internal),
                    Ok(None) => Ok(()),
                    Err(e) => Err(e),
                };
                // 即使任务退出失败，也要释放真实目录租约，不能让旧 Arc 再次阻止新内核接管。
                let me2 = me.clone();
                let resources = tokio::task::spawn_blocking(move || {
                    let mut state = me2.state.lock().map_err(|_| Error::Internal)?;
                    let result = state.cache.take().map_or(Ok(()), |mut cache| cache.close());
                    state.data = Data::default();
                    state.configuration = None;
                    result
                })
                .await
                .map_err(|_| Error::Internal)
                .and_then(|v| v);
                let released = me
                    ._lease
                    .lock()
                    .map_err(|_| Error::Internal)
                    .map(|mut lease| {
                        lease.take();
                    });
                let traffic = me.traffic.wait_idle().await;
                let traffic = traffic.and_then(|_| me.traffic.complete(Ok(())));
                let result = idle.and(joined).and(resources).and(released).and(traffic);
                let _ = crate::error::diagnostics::observe(
                    me.authority.complete(result),
                    crate::error::diagnostics::DiagnosticSite::Cleanup,
                );
            });
        }
        Ok(())
    }
}
type Loaded = (
    Data,
    Vec<Source>,
    BTreeMap<String, (usize, usize)>,
    Vec<ByteLease>,
    bool,
);
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

#[cfg(test)]
impl Runtime {
    pub(crate) fn is_candidate_storage(&self) -> Result<bool> {
        Ok(self.home.is_none()
            && self
                .state
                .lock()
                .map_err(|_| Error::Internal)?
                .cache
                .is_none()
            && self.worker.lock().map_err(|_| Error::Internal)?.is_none())
    }
}
