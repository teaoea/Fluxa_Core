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
    pub resolver: Option<Arc<Resolver>>,
    pub router: Option<Arc<Router>>,
    pub limits: Limits,
}
impl Manager {
    pub fn network_snapshot(&self) -> Result<Arc<NetworkSnapshot>> {
        Ok(Arc::clone(
            &self.state.lock().map_err(|_| Error::Internal)?.network,
        ))
    }
    /// 调用者先构造、验证 DNS 与路由；持锁一次提交，不暴露中间状态。
    pub fn configure_network(
        &self,
        resolver: Option<Arc<Resolver>>,
        router: Option<Arc<Router>>,
        limits: Limits,
    ) -> Result<u64> {
        limits.validate()?;
        self.replace_network(|_| NetworkSnapshot {
            revision: 0,
            resolver,
            router,
            limits,
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
