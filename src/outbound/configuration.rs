//! 不可变网络配置快照及资源参数，失败不替换当前配置。
use super::Manager;
use crate::{
    dns::Resolver,
    error::{Error, Result},
    routing::Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Limits {
    #[serde(rename = "maximum-connections")]
    pub maximum_connections: usize,
    #[serde(rename = "idle-timeout-ms")]
    pub idle_timeout_ms: u64,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            maximum_connections: 64,
            idle_timeout_ms: 300000,
        }
    }
}
impl Limits {
    pub fn validate(self) -> Result<()> {
        if !(1..=64).contains(&self.maximum_connections)
            || !(100..=3600000).contains(&self.idle_timeout_ms)
        {
            return Err(Error::InvalidInput);
        }
        Ok(())
    }
}
/// 建连事务捕获同一配置；完成后连接仅保留既定决策与必要资源，不持有整份旧规则快照。
/// 提交后新请求读取新快照，已开始事务/连接不中断、不改换对端。
#[derive(Clone, Default)]
pub struct NetworkSnapshot {
    pub revision: u64,
    // 仅 DNS 关闭的内核下载上下文显式允许系统解析，不能作为加密 DNS 失败回退。
    pub(crate) system_dns: bool,
    // 同一快照保留配置预算；分模块变更撤销 complete 标记，但共享节点仍需保留租约。
    pub(crate) configuration: Option<Arc<crate::configuration::Document>>,
    pub(crate) configuration_complete: bool,
    pub(crate) session: Option<std::sync::Arc<crate::lifecycle::session::Authority>>,
    pub resolver: Option<Arc<Resolver>>,
    pub router: Option<Arc<Router>>,
    pub limits: Limits,
    pub outbounds: Arc<std::collections::HashMap<String, Arc<crate::protocol::config::Node>>>,
}
impl Manager {
    /// 提交主配置意图；准备在锁外完成，锁内核对基线并一次发布整份快照。
    /// 成功仅代表配置提交，不创建监听或接管 VPN。失败保留旧配置和已建立连接。
    /// 示例：manager.apply_configuration("rules: ['final,reject']")?。
    pub fn apply_configuration(&self, yaml: &str) -> Result<u64> {
        let revision = self.network_snapshot()?.revision;
        let prepared = crate::configuration::Document::from_yaml(yaml)?.prepare()?;
        self.commit_configuration(revision, prepared)
    }
    fn commit_configuration(
        &self,
        revision: u64,
        prepared: crate::configuration::Prepared,
    ) -> Result<u64> {
        let limits = prepared.document.raw_limits();
        self.replace_network_at(Some(revision), |_| NetworkSnapshot {
            configuration: Some(prepared.document),
            configuration_complete: true,
            router: Some(prepared.router),
            outbounds: prepared.outbounds,
            limits,
            ..NetworkSnapshot::default()
        })
    }
    /// 在同一次快照读取中返回版本及无凭据摘要；调用方无法修改已提交对象。
    pub fn configuration_status(&self) -> Result<crate::configuration::ConfigurationStatus> {
        let snapshot = self.network_snapshot()?;
        Ok(crate::configuration::ConfigurationStatus {
            revision: snapshot.revision,
            configuration: snapshot
                .configuration
                .as_ref()
                .filter(|_| snapshot.configuration_complete)
                .map(|document| document.summary()),
        })
    }
    /// 仅专用 IPC 授权层可绑定 Debug；普通配置提交不得创造授权会话。
    pub(crate) fn replace_debug(
        &self,
        session: Arc<crate::routing::debug::Session>,
        rules: Vec<crate::routing::debug::DebugRule>,
        outbound: Option<String>,
    ) -> Result<()> {
        let mut state = self.state.lock().map_err(|_| Error::Internal)?;
        if state.stopped {
            return Err(Error::InvalidState);
        }
        if state
            .debug
            .as_ref()
            .is_some_and(|active| active.identity() != session.identity())
        {
            return Err(Error::DebugUnauthorized);
        }
        let router = state.network.router.as_ref().ok_or(Error::InvalidState)?;
        // 状态和路由存在性先检查，规则完整验证后发布，失败不留下部分授权。
        session.replace(rules, outbound)?;
        let router = Arc::new((**router).clone().with_debug(&session)?);
        state.network = Arc::new(NetworkSnapshot {
            router: Some(router),
            ..(*state.network).clone()
        });
        state.debug = Some(session);
        Ok(())
    }
    pub(crate) async fn close_revoked_debug(&self) -> Result<()> {
        let connections = {
            let state = self.state.lock().map_err(|_| Error::Internal)?;
            state
                .registry
                .iter()
                .filter(|(_, connection)| connection.protection.revoked_debug())
                .map(|(id, connection)| (id, connection.clone()))
                .collect::<Vec<_>>()
        };
        for (_, connection) in &connections {
            connection.request_close()?;
        }
        for (id, connection) in connections {
            connection.wait_closed().await?;
            self.state
                .lock()
                .map_err(|_| Error::Internal)?
                .registry
                .take(id);
        }
        Ok(())
    }
    pub(crate) fn unbind_debug(&self, identity: u64) -> Result<()> {
        // 撤销由 Session 完成，存活快照检查 revoked，不依赖外部引用释放。
        let mut state = self.state.lock().map_err(|_| Error::Internal)?;
        if state
            .debug
            .as_ref()
            .is_some_and(|active| active.identity() == identity)
        {
            state.debug.take();
        }
        Ok(())
    }

    pub fn network_snapshot(&self) -> Result<Arc<NetworkSnapshot>> {
        Ok(Arc::clone(
            &self.state.lock().map_err(|_| Error::Internal)?.network,
        ))
    }
    /// 已关闭的解析器只留下配置元数据；新会话由内核创建新权限和映射代次。
    pub(crate) fn prepare_tun_snapshot(&self) -> Result<Arc<NetworkSnapshot>> {
        let mut state = self.state.lock().map_err(|_| Error::Internal)?;
        if state.stopped {
            return Err(Error::InvalidState);
        }
        if let Some(resolver) = &state.network.resolver
            && resolver.is_closed()?
        {
            let resolver = Arc::new(resolver.replacement()?);
            state.network = Arc::new(NetworkSnapshot {
                revision: crate::lifecycle::identifier().ok_or(Error::ResourceLimit)?,
                resolver: Some(resolver),
                session: None,
                ..(*state.network).clone()
            });
        }
        state.network.ensure_active()?;
        Ok(state.network.clone())
    }
    /// 调用者先构造、验证 DNS 与路由；持锁一次提交，不暴露中间状态。
    pub fn configure_network(
        &self,
        resolver: Option<Arc<Resolver>>,
        router: Option<Arc<Router>>,
        limits: Limits,
    ) -> Result<u64> {
        self.configure_network_with_outbounds(resolver, router, limits, Vec::new())
    }
    /// 所有节点先校验；重复标识、过量配置或不支持的协议失败不改变快照。
    pub fn configure_network_with_outbounds(
        &self,
        resolver: Option<Arc<Resolver>>,
        router: Option<Arc<Router>>,
        limits: Limits,
        profiles: Vec<crate::protocol::config::Profile>,
    ) -> Result<u64> {
        limits.validate()?;
        if profiles.len() > 128 {
            return Err(Error::ResourceLimit);
        }
        let mut outbounds = std::collections::HashMap::with_capacity(profiles.len());
        for profile in profiles {
            let node = Arc::new(profile.validate()?);
            if outbounds.insert(profile.id.clone(), node).is_some() {
                return Err(Error::InvalidInput);
            }
        }
        self.replace_network(|_| NetworkSnapshot {
            revision: 0,
            configuration: None,
            configuration_complete: false,
            session: None,
            resolver,
            router,
            limits,
            outbounds: Arc::new(outbounds),
            system_dns: false,
        })
    }
    pub fn configure_limits(&self, limits: Limits) -> Result<u64> {
        limits.validate()?;
        self.replace_network(|old| NetworkSnapshot {
            configuration_complete: false,
            limits,
            ..old.clone()
        })
    }
    pub fn configure_dns(&self, resolver: Option<Arc<Resolver>>) -> Result<u64> {
        self.replace_network(|old| NetworkSnapshot {
            configuration_complete: false,
            resolver,
            ..old.clone()
        })
    }
    pub fn configure_routing(&self, router: Option<Arc<Router>>) -> Result<u64> {
        self.replace_network(|old| NetworkSnapshot {
            configuration_complete: false,
            router,
            ..old.clone()
        })
    }
    /// 所有可失败检查和分配发生在文件提交之前。
    pub(crate) fn commit_routing_file(
        &self,
        router: Arc<Router>,
        commit: impl FnOnce() -> Result<()>,
    ) -> Result<u64> {
        let mut state = self.state.lock().map_err(|_| Error::Internal)?;
        if state.stopped {
            return Err(Error::InvalidState);
        }
        let revision = crate::lifecycle::identifier().ok_or(Error::ResourceLimit)?;
        let router = if let Some(session) = &state.debug {
            Arc::new((*router).clone().with_debug(session)?)
        } else {
            router
        };
        let replacement = Arc::new(NetworkSnapshot {
            revision,
            router: Some(router),
            configuration_complete: false,
            ..(*state.network).clone()
        });
        commit()?;
        state.network = replacement;
        Ok(revision)
    }
    fn replace_network(
        &self,
        update: impl FnOnce(&NetworkSnapshot) -> NetworkSnapshot,
    ) -> Result<u64> {
        self.replace_network_at(None, update)
    }
    fn replace_network_at(
        &self,
        expected: Option<u64>,
        update: impl FnOnce(&NetworkSnapshot) -> NetworkSnapshot,
    ) -> Result<u64> {
        let mut state = self.state.lock().map_err(|_| Error::Internal)?;
        if state.providers.is_some() {
            return Err(Error::InvalidState);
        }
        if state.stopped || expected.is_some_and(|revision| revision != state.network.revision) {
            return Err(Error::InvalidState);
        }
        let mut replacement = update(&state.network);
        if let Some(session) = &state.debug {
            let router = replacement.router.as_ref().ok_or(Error::InvalidState)?;
            replacement.router = Some(Arc::new((**router).clone().with_debug(session)?));
        }
        replacement.revision = crate::lifecycle::identifier().ok_or(Error::ResourceLimit)?;
        if let Some(resolver) = &replacement.resolver {
            resolver.ensure_active()?;
        }
        let revision = replacement.revision;
        let replacement = Arc::new(replacement);
        // 所有校验先完成，再在同一配置锁内退休旧解析器并发布新快照。
        // Arc::ptr_eq 只判断“配置是否仍指向同一实例”，绝不用引用数量判断资源占用。
        // 退休停止新查询、等待已登记查询后由内核清理，不关闭已经建立的业务连接。
        // 限额/路由更新复用同一解析器，不得误退休；失败提交不得改变旧解析器权限。
        if let Some(old) = &state.network.resolver
            && !replacement
                .resolver
                .as_ref()
                .is_some_and(|new| Arc::ptr_eq(old, new))
        {
            old.retire()?;
        }
        state.network = replacement;
        Ok(revision)
    }
}

impl NetworkSnapshot {
    pub(crate) fn for_session(&self, owner: Arc<crate::lifecycle::session::Authority>) -> Self {
        Self {
            session: Some(owner),
            ..self.clone()
        }
    }
    pub(crate) fn ensure_active(&self) -> Result<()> {
        if let Some(owner) = &self.session {
            owner.check(owner.identity())?;
        }
        if let Some(resolver) = &self.resolver {
            resolver.ensure_active()?;
        }
        Ok(())
    }
    /// 正式出站也可能使用 TUN 绑定的解析器，连接归属必须包含该有效租约。
    pub(crate) fn owner(&self) -> Result<Option<Arc<crate::lifecycle::session::Authority>>> {
        let binding = self
            .resolver
            .as_ref()
            .map(|resolver| resolver.binding_authority())
            .transpose()?
            .flatten();
        if let (Some(session), Some(binding)) = (&self.session, &binding)
            && session.identity() != binding.identity()
        {
            return Err(Error::InvalidState);
        }
        Ok(self.session.clone().or(binding))
    }
    /// 与会话撤销串行登记真实连接；Socket 建立后权限失效必须回滚，不能发布句柄。
    pub(crate) fn commit<T>(
        &self,
        operation: Option<&crate::dns::ResolverOperation>,
        publish: impl FnOnce() -> Result<T>,
    ) -> Result<T> {
        let binding = self
            .resolver
            .as_ref()
            .map(|resolver| resolver.binding_authority())
            .transpose()?
            .flatten();
        if let (Some(session), Some(binding)) = (&self.session, &binding)
            && session.identity() != binding.identity()
        {
            return Err(Error::InvalidState);
        }
        let owner = self.session.as_ref().or(binding.as_ref());
        let publish = || {
            if let Some(resolver) = &self.resolver {
                // establish_guarded 在任何 await 前登记令牌，并持有至本次提交结束。
                // 退休允许它完成；显式关闭仍通过驱动 Authority 拒绝提交。
                resolver.commit_operation(operation.ok_or(Error::InvalidState)?, publish)
            } else {
                publish()
            }
        };
        if let Some(owner) = owner {
            owner.with_active(owner.identity(), publish)
        } else {
            publish()
        }
    }
    /// 共用出站在解析和拨号前执行协议保护，不从直连 API 绕过有效拦截。
    pub(crate) fn check_transport(&self, transport: crate::routing::Transport) -> Result<()> {
        self.ensure_active()?;
        if let Some(router) = &self.router {
            router.check_transport(transport)?;
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "../../tests/test_outbound_configuration.rs"]
mod test_outbound_configuration;

impl Manager {
    /// 有提供者时的主配置意图：下载/计划/缓存均由内核拥有，首次导入失败保留旧配置。
    /// 无提供者时可继续使用同步 apply_configuration；活动提供者存在后应统一使用本异步入口。
    pub async fn apply_configuration_async(self: &Arc<Self>, yaml: &str) -> Result<u64> {
        let document = Arc::new(crate::configuration::Document::from_yaml(yaml)?);
        if document.provider_settings().is_empty()
            && self
                .state
                .lock()
                .map_err(|_| Error::Internal)?
                .providers
                .is_none()
        {
            let revision = self.network_snapshot()?.revision;
            return self.commit_configuration(revision, document.prepare_shared(Vec::new())?);
        }
        let client = crate::https::HttpsClient::system()?;
        self.apply_provider_document(document, client, None).await
    }
    /// TLS 配置沿用 HTTPS 客户端的系统/企业根，忽略其网络对象；业务路由由内核候选配置生成。
    pub async fn apply_configuration_with_client(
        self: &Arc<Self>,
        yaml: &str,
        client: crate::https::HttpsClient,
    ) -> Result<u64> {
        let document = Arc::new(crate::configuration::Document::from_yaml(yaml)?);
        self.apply_provider_document(document, client, None).await
    }
    pub(crate) async fn apply_provider_document(
        self: &Arc<Self>,
        document: Arc<crate::configuration::Document>,
        client: crate::https::HttpsClient,
        home: Option<std::path::PathBuf>,
    ) -> Result<u64> {
        let _serial = self.provider_control.lock().await;
        // 在任何文件/网络操作前拒绝尚未实现的活动能力，不能部分接管。
        document.provider_preflight()?;
        let existing = {
            let state = self.state.lock().map_err(|_| Error::Internal)?;
            if state.stopped {
                return Err(Error::InvalidState);
            }
            state.providers.clone()
        };
        if existing.is_none() && document.provider_settings().is_empty() {
            let revision = self.network_snapshot()?.revision;
            return self.commit_configuration(revision, document.prepare_shared(Vec::new())?);
        }
        let runtime = if let Some(runtime) = existing {
            runtime
        } else {
            let home = home
                .or_else(|| std::env::var_os("HOME").map(std::path::PathBuf::from))
                .ok_or(Error::InvalidState)?;
            let runtime = crate::providers::Runtime::new(self, client, home)?;
            {
                let mut state = self.state.lock().map_err(|_| Error::Internal)?;
                if state.stopped {
                    return Err(Error::InvalidState);
                }
                state.providers = Some(runtime.clone());
            }
            runtime.launch()?;
            runtime
        };
        // 控制面 future 在此擦除嵌套类型，避免公开 async 调用方承受整个下载/解析/提交栈的布局深度。
        let operation: std::pin::Pin<
            Box<dyn std::future::Future<Output = Result<u64>> + Send + '_>,
        > = Box::pin(runtime.apply(document, None));
        operation.await
    }
    pub(crate) fn provider_runtime(&self) -> Result<Option<Arc<crate::providers::Runtime>>> {
        Ok(self
            .state
            .lock()
            .map_err(|_| Error::Internal)?
            .providers
            .clone())
    }
    pub fn provider_status(&self) -> Result<crate::providers::Status> {
        let runtime = self.provider_runtime()?;
        match runtime {
            Some(runtime) => runtime.status(),
            None => Ok(crate::providers::Status {
                active: false,
                revision: None,
                providers: Vec::new(),
                error_info: None,
                directory_synced: true,
            }),
        }
    }
    /// 手动刷新单项或全部已声明提供者，不会自动启用停用项。
    pub async fn refresh_providers(&self, name: Option<String>) -> Result<u64> {
        // 与配置意图串行，不能用排队前读取的旧文档覆盖刚提交的新配置。
        let _serial = self.provider_control.lock().await;
        let runtime = self.provider_runtime()?.ok_or(Error::InvalidState)?;
        let operation: std::pin::Pin<
            Box<dyn std::future::Future<Output = Result<u64>> + Send + '_>,
        > = Box::pin(runtime.refresh(name));
        operation.await
    }
    pub(crate) fn configure_provider_download(
        &self,
        prepared: &crate::configuration::Prepared,
        authority: Arc<crate::lifecycle::session::Authority>,
    ) -> Result<u64> {
        self.replace_network(|_| NetworkSnapshot {
            configuration: Some(prepared.document.clone()),
            router: Some(prepared.router.clone()),
            outbounds: prepared.outbounds.clone(),
            limits: prepared.document.raw_limits(),
            system_dns: true,
            session: Some(authority),
            ..NetworkSnapshot::default()
        })
    }
    pub(crate) fn commit_provider_configuration(
        &self,
        revision: u64,
        prepared: crate::configuration::Prepared,
        commit: impl FnOnce() -> Result<()>,
    ) -> Result<u64> {
        let mut state = self.state.lock().map_err(|_| Error::Internal)?;
        if state.stopped || state.network.revision != revision || state.providers.is_none() {
            return Err(Error::InvalidState);
        }
        let router = match &state.debug {
            Some(session) => Arc::new((*prepared.router).clone().with_debug(session)?),
            None => prepared.router,
        };
        let next = crate::lifecycle::identifier().ok_or(Error::ResourceLimit)?;
        let limits = prepared.document.raw_limits();
        let replacement = Arc::new(NetworkSnapshot {
            revision: next,
            configuration: Some(prepared.document),
            configuration_complete: true,
            router: Some(router),
            outbounds: prepared.outbounds,
            limits,
            ..NetworkSnapshot::default()
        });
        // 旧活动解析器迁移尚未纳入本阶段，禁止在文件提交后执行可失败退休。
        if state.network.resolver.is_some() {
            return Err(Error::InvalidState);
        }
        commit()?;
        state.network = replacement;
        Ok(next)
    }
}
