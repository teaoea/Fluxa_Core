//! 用户 DNS 配置映射；bootstrap 使用数字 IP，避免加密 DNS 自举循环及系统解析泄漏。
use crate::error::{Error, Result};
use hickory_resolver::config::{
    ConnectionConfig, NameServerConfig, ResolveHosts, ResolverConfig, ResolverOpts,
    ServerOrderingStrategy,
};
use serde::{Deserialize, Serialize};
use std::{net::SocketAddr, sync::Arc, time::Duration};
use url::Url;
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "transport", rename_all = "lowercase", deny_unknown_fields)]
pub enum Server {
    Udp {
        address: String,
    },
    Tcp {
        address: String,
    },
    Dot {
        address: String,
        server_name: String,
    },
    Doh {
        url: String,
        bootstrap: String,
    },
}
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub servers: Vec<Server>,
    pub timeout_ms: u64,
    #[serde(default)]
    pub dnssec: bool,
    #[serde(default)]
    pub fake_ip: Option<super::fake_ip::Config>,
    #[serde(default)]
    pub service: Option<super::service::Config>,
}
impl Config {
    /// 直接计算被保存的配置分配容量，不为预算计算再分配；旧快照保留此租约。
    pub(super) fn allocated_bytes(&self) -> Result<usize> {
        let mut bytes = 4096usize;
        let mut add = |value: usize| -> Result<()> {
            bytes = bytes.checked_add(value).ok_or(Error::ResourceLimit)?;
            Ok(())
        };
        add(self
            .servers
            .capacity()
            .checked_mul(std::mem::size_of::<Server>())
            .ok_or(Error::ResourceLimit)?)?;
        for server in &self.servers {
            match server {
                Server::Udp { address } | Server::Tcp { address } => add(address.capacity())?,
                Server::Dot {
                    address,
                    server_name,
                } => {
                    add(address.capacity())?;
                    add(server_name.capacity())?;
                }
                Server::Doh { url, bootstrap } => {
                    add(url.capacity())?;
                    add(bootstrap.capacity())?;
                }
            }
        }
        if let Some(fake) = &self.fake_ip {
            // Table 保存独立配置副本，计入两份字符串/容器容量。
            for value in [&fake.ipv4_range, &fake.ipv6_range] {
                add(value
                    .capacity()
                    .checked_mul(2)
                    .ok_or(Error::ResourceLimit)?)?;
            }
            add(fake
                .exclude
                .capacity()
                .checked_mul(std::mem::size_of::<String>() * 2)
                .ok_or(Error::ResourceLimit)?)?;
            for value in &fake.exclude {
                add(value
                    .capacity()
                    .checked_mul(2)
                    .ok_or(Error::ResourceLimit)?)?;
            }
        }
        if let Some(service) = &self.service {
            add(service.listen.capacity())?;
            add(service
                .allowed_clients
                .capacity()
                .checked_mul(std::mem::size_of::<String>())
                .ok_or(Error::ResourceLimit)?)?;
            for value in &service.allowed_clients {
                add(value.capacity())?;
            }
        }
        Ok(bytes)
    }
    pub fn validate(&self) -> Result<()> {
        self.build().map(|_| ())
    }
    pub(super) fn build(&self) -> Result<(ResolverConfig, ResolverOpts)> {
        if self.servers.is_empty()
            || self.servers.len() > 8
            || !(100..=30000).contains(&self.timeout_ms)
        {
            return Err(Error::InvalidInput);
        }
        if let Some(fake_ip) = &self.fake_ip {
            fake_ip.validate()?;
        }
        if let Some(service) = &self.service {
            service.validate()?;
        }
        let encrypted = self
            .servers
            .iter()
            .any(|s| matches!(s, Server::Dot { .. } | Server::Doh { .. }));
        if encrypted
            && self
                .servers
                .iter()
                .any(|s| matches!(s, Server::Udp { .. } | Server::Tcp { .. }))
        {
            return Err(Error::InvalidInput);
        }
        let mut servers = vec![];
        for server in &self.servers {
            let (address, mut connections) = match server {
                Server::Udp { address } => (
                    endpoint(address)?,
                    vec![ConnectionConfig::udp(), ConnectionConfig::tcp()],
                ),
                Server::Tcp { address } => (endpoint(address)?, vec![ConnectionConfig::tcp()]),
                Server::Dot {
                    address,
                    server_name,
                } => {
                    super::normalize(server_name)?;
                    (
                        endpoint(address)?,
                        vec![ConnectionConfig::tls(Arc::from(server_name.as_str()))],
                    )
                }
                Server::Doh { url, bootstrap } => {
                    if url.len() > 2048 {
                        return Err(Error::ResourceLimit);
                    }
                    let target = Url::parse(url).map_err(|_| Error::InvalidInput)?;
                    if target.scheme() != "https"
                        || !target.username().is_empty()
                        || target.password().is_some()
                        || target.fragment().is_some()
                    {
                        return Err(Error::InvalidInput);
                    }
                    let host = target.host_str().ok_or(Error::InvalidInput)?;
                    let address = endpoint(bootstrap)?;
                    if target.port_or_known_default() != Some(address.port()) {
                        return Err(Error::InvalidInput);
                    }
                    let mut path = target.path().to_owned();
                    if let Some(query) = target.query() {
                        path.push('?');
                        path.push_str(query);
                    }
                    (
                        address,
                        vec![ConnectionConfig::https(
                            Arc::from(host),
                            Some(Arc::from(path)),
                        )],
                    )
                }
            };
            for connection in &mut connections {
                connection.port = address.port();
            }
            servers.push(NameServerConfig::new(address.ip(), true, connections));
        }
        let mut options = ResolverOpts::default();
        // 每个上游共享总期限，给后续配置的上游保留重试时间。
        options.timeout =
            Duration::from_millis((self.timeout_ms / self.servers.len() as u64).max(50));
        options.attempts = 1;
        options.validate = self.dnssec;
        options.cache_size = 0;
        options.use_hosts_file = ResolveHosts::Never;
        options.preserve_intermediates = true;
        options.num_concurrent_reqs = 1;
        options.max_active_requests = 16;
        options.server_ordering_strategy = ServerOrderingStrategy::UserProvidedOrder;
        options.try_tcp_on_error = false;
        options.edns_payload_len = 1232;
        Ok((ResolverConfig::from_name_servers(servers), options))
    }
}
fn endpoint(value: &str) -> Result<SocketAddr> {
    if value.len() > 128 {
        return Err(Error::InvalidInput);
    }
    let address: SocketAddr = value.parse().map_err(|_| Error::InvalidInput)?;
    if address.port() == 0
        || address.ip().is_unspecified()
        || address.ip().is_multicast()
        || matches!(address.ip(), std::net::IpAddr::V4(ip) if ip.is_broadcast())
    {
        return Err(Error::InvalidInput);
    }
    Ok(address)
}
