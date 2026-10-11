//! 默认 DNS 最近一次真实查询统计；同配置的存活解析器共享，退出后自动释放。
use crate::error::{Error, Result};
use serde::Serialize;
use std::sync::{Arc, Mutex, OnceLock, Weak};

#[derive(Clone, Debug, Serialize)]
pub struct Observation {
    pub duration_ms: u64,
    pub from_cache: bool,
    pub successful: bool,
}
type State = Mutex<Option<Observation>>;
struct Entry {
    key: String,
    state: Weak<State>,
}
static REGISTRY: OnceLock<Mutex<Vec<Entry>>> = OnceLock::new();

pub(super) fn shared(config: &super::Config) -> Result<Arc<State>> {
    // 仅以默认服务器、超时和验证策略隔离；不保存域名、响应、凭据或请求正文。
    let key = serde_json::to_string(&(
        config.servers.first().ok_or(Error::InvalidInput)?,
        config.timeout_ms,
        config.dnssec,
    ))
    .map_err(|_| Error::InvalidInput)?;
    if key.len() > 8192 {
        return Err(Error::ResourceLimit);
    }
    let mut entries = REGISTRY
        .get_or_init(|| Mutex::new(Vec::new()))
        .lock()
        .map_err(|_| Error::Internal)?;
    entries.retain(|entry| entry.state.strong_count() > 0);
    if let Some(state) = entries
        .iter()
        .find(|entry| entry.key == key)
        .and_then(|entry| entry.state.upgrade())
    {
        return Ok(state);
    }
    // 共用解析器实例已有 16 个配额；注册表不扩大生命周期或长期累积配置。
    if entries.len() >= 16 {
        return Err(Error::ResourceLimit);
    }
    let state = Arc::new(Mutex::new(None));
    entries.push(Entry {
        key,
        state: Arc::downgrade(&state),
    });
    Ok(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn config(port: u16) -> super::super::Config {
        super::super::Config {
            servers: vec![super::super::Server::Udp {
                address: format!("127.0.0.1:{port}"),
            }],
            timeout_ms: 1000,
            dnssec: false,
            fake_ip: None,
            service: None,
        }
    }
    #[test]
    fn shares_only_live_matching_configuration() {
        let a = shared(&config(18001)).unwrap();
        let b = shared(&config(18001)).unwrap();
        assert!(Arc::ptr_eq(&a, &b));
        *a.lock().unwrap() = Some(Observation {
            duration_ms: 12,
            from_cache: false,
            successful: true,
        });
        assert_eq!(b.lock().unwrap().as_ref().unwrap().duration_ms, 12);
        let other = shared(&config(18002)).unwrap();
        assert!(other.lock().unwrap().is_none());
        drop(a);
        drop(b);
        assert!(shared(&config(18001)).unwrap().lock().unwrap().is_none());
    }
}
