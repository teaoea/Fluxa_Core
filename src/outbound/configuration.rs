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
/// 所有者必须保留此快照直到本次连接完成；替换后存量连接不改换对端。
#[derive(Clone, Default)]
pub struct NetworkSnapshot {
    pub revision: u64,
    pub(crate) session: Option<std::sync::Arc<crate::lifecycle::session::Authority>>,
    pub resolver: Option<Arc<Resolver>>,
    pub router: Option<Arc<Router>>,
    pub limits: Limits,
    pub outbounds: Arc<std::collections::HashMap<String, Arc<crate::protocol::config::Node>>>,
}
impl Manager {
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
            session: None,
            resolver,
            router,
            limits,
            outbounds: Arc::new(outbounds),
        })
    }
    pub fn configure_limits(&self, limits: Limits) -> Result<u64> {
        limits.validate()?;
        self.replace_network(|old| NetworkSnapshot {
            limits,
            ..old.clone()
        })
    }
    pub fn configure_dns(&self, resolver: Option<Arc<Resolver>>) -> Result<u64> {
        self.replace_network(|old| NetworkSnapshot {
            resolver,
            ..old.clone()
        })
    }
    pub fn configure_routing(&self, router: Option<Arc<Router>>) -> Result<u64> {
        self.replace_network(|old| NetworkSnapshot {
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
        let replacement = Arc::new(NetworkSnapshot {
            revision,
            router: Some(router),
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
        let mut state = self.state.lock().map_err(|_| Error::Internal)?;
        if state.stopped {
            return Err(Error::InvalidState);
        }
        let mut replacement = update(&state.network);
        replacement.revision = crate::lifecycle::identifier().ok_or(Error::ResourceLimit)?;
        let revision = replacement.revision;
        state.network = Arc::new(replacement);
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
    pub(crate) fn commit<T>(&self, publish: impl FnOnce() -> Result<T>) -> Result<T> {
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
                resolver.with_active(publish)
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
