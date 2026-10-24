//! 主配置 DNS 编排：按列表逐个尝试、独立期限、受限引导与指定节点；不隐式系统回退。
use super::*;
use crate::configuration::models::Dns;
use hickory_resolver::{
    lookup::Lookup,
    proto::{
        op::Query as Question,
        rr::{Name, Record},
    },
};
use std::net::{IpAddr, SocketAddr};

struct Endpoint {
    resolver: Option<UpstreamResolver<ManagedRuntime>>,
    expires: Instant,
    network: Option<Arc<crate::outbound::Manager>>,
}
pub(super) struct Plan {
    settings: Vec<String>,
    bootstrap: Option<Arc<Resolver>>,
    hosts: BTreeMap<String, String>,
    endpoints: tokio::sync::RwLock<Vec<Endpoint>>,
    nodes: Arc<std::collections::HashMap<String, Arc<crate::protocol::config::Node>>>,
    tls: rustls::ClientConfig,
    timeout: Duration,
    dnssec: bool,
    authority: Arc<Authority>,
    // 字符串、节点表引用与端点控制对象在真实销毁前始终保留预算。
    _memory: ByteLease,
}
fn endpoint(value: &str, port: u16) -> Result<SocketAddr> {
    value
        .parse::<IpAddr>()
        .map(|ip| SocketAddr::new(ip, port))
        .or_else(|_| value.parse())
        .map_err(|_| Error::InvalidInput)
}
impl Resolver {
    /// 内核候选创建解析器；启动时完成所有上游引导，任何失败拒绝整个配置。
    pub(crate) async fn from_main(
        config: &Dns,
        nodes: Arc<std::collections::HashMap<String, Arc<crate::protocol::config::Node>>>,
        tls: rustls::ClientConfig,
        scope: Option<logging::Scope>,
    ) -> Result<Option<Arc<Self>>> {
        if !config.enable {
            return Ok(None);
        }
        let timeout_ms = config.timeout as u64 * 1000;
        let bootstrap = config
            .default_nameserver
            .as_ref()
            .map(|servers| {
                Resolver::with_tls_config(
                    Config {
                        servers: servers
                            .iter()
                            .map(|v| {
                                Ok(Server::Udp {
                                    address: endpoint(v, 53)?.to_string(),
                                })
                            })
                            .collect::<Result<_>>()?,
                        timeout_ms,
                        dnssec: false,
                        fake_ip: None,
                        service: None,
                    },
                    tls.clone(),
                )
                .map(Arc::new)
            })
            .transpose()?;
        let fake_ip = config
            .fake_ip
            .as_ref()
            .filter(|f| f.enable)
            .map(|f| fake_ip::Config::automatic(&f.ipv4_range, &f.ipv6_range, &f.exclude))
            .transpose()?;
        // 占位仅供沿用共用配置/预算；主查询始终通过 Plan，不会连接此地址。
        let mut resolver = Self::with_tls_config(
            Config {
                servers: vec![Server::Udp {
                    address: "127.0.0.1:53".into(),
                }],
                timeout_ms,
                dnssec: config.dnssec,
                fake_ip,
                service: None,
            },
            tls.clone(),
        )?;
        let memory = METADATA_BUDGET.reserve(
            65536usize
                .checked_add(nodes.len().checked_mul(16384).ok_or(Error::ResourceLimit)?)
                .ok_or(Error::ResourceLimit)?,
        )?;
        let plan = Arc::new(Plan {
            settings: config.nameserver.clone(),
            bootstrap,
            hosts: config
                .hosts
                .iter()
                .map(|(k, v)| Ok((normalize(k)?, v.clone())))
                .collect::<Result<_>>()?,
            endpoints: tokio::sync::RwLock::new(Vec::new()),
            nodes,
            tls,
            timeout: Duration::from_millis(timeout_ms),
            dnssec: config.dnssec,
            authority: resolver.authority.clone(),
            _memory: memory,
        });
        let runtime = resolver
            .resources
            .read()
            .await
            .as_ref()
            .ok_or(Error::InvalidState)?
            .runtime
            .clone();
        if let Err(error) = plan.initialize(&runtime, scope).await {
            plan.close().await?;
            return Err(error);
        }
        resolver.main = Some(plan);
        Ok(Some(Arc::new(resolver)))
    }
}
impl Plan {
    /// 新会话只复制不可变配置，使用新的权限/连接/引导缓存；旧 Arc 不控制新准入。
    pub(super) fn replacement(&self, authority: Arc<Authority>) -> Result<Self> {
        Ok(Self {
            settings: self.settings.clone(),
            bootstrap: self
                .bootstrap
                .as_ref()
                .map(|r| r.replacement().map(Arc::new))
                .transpose()?,
            hosts: self.hosts.clone(),
            endpoints: tokio::sync::RwLock::new(Vec::new()),
            nodes: self.nodes.clone(),
            tls: self.tls.clone(),
            timeout: self.timeout,
            dnssec: self.dnssec,
            authority,
            _memory: METADATA_BUDGET.reserve(
                65536usize
                    .checked_add(
                        self.nodes
                            .len()
                            .checked_mul(16384)
                            .ok_or(Error::ResourceLimit)?,
                    )
                    .ok_or(Error::ResourceLimit)?,
            )?,
        })
    }
    pub(super) fn setting(&self, index: usize) -> Option<&str> {
        self.settings.get(index).map(String::as_str)
    }
    pub(super) fn len(&self) -> usize {
        self.settings.len()
    }
    async fn bootstrap(
        &self,
        host: &str,
        scope: Option<logging::Scope>,
    ) -> Result<(Vec<IpAddr>, u32)> {
        if let Ok(ip) = host.parse() {
            return Ok((vec![ip], 3600));
        }
        if let Some(resolver) = &self.bootstrap {
            let mut addresses = Vec::new();
            let mut ttl = 3600;
            let mut last = Error::DnsResponse;
            for kind in ["A", "AAAA"] {
                match Box::pin(resolver.resolve_logged(
                    Query {
                        name: host.into(),
                        query_type: kind.into(),
                    },
                    false,
                    None,
                    scope.clone(),
                ))
                .await
                {
                    Ok(answer) if answer.rcode == 3 => return Err(Error::DnsResponse),
                    Ok(answer) => {
                        // 显式 DNS 的零 TTL 表示不能缓存，不得替换为默认有效期。
                        ttl = ttl.min(answer.ttl);
                        for ip in answer.addresses {
                            if addresses.len() >= 16 {
                                return Err(Error::ResourceLimit);
                            }
                            addresses.push(ip.parse().map_err(|_| Error::DnsResponse)?);
                        }
                    }
                    Err(error @ (Error::DnsTimeout | Error::DnsResponse)) => last = error,
                    Err(error) => return Err(error),
                }
            }
            if addresses.is_empty() {
                return Err(last);
            }
            return Ok((addresses, ttl));
        }
        let values =
            crate::outbound::resolve_bootstrap_system(host, self.authority.clone(), scope).await?;
        Ok((values, 3600))
    }
    async fn build(
        &self,
        index: usize,
        runtime: &ManagedRuntime,
        scope: Option<logging::Scope>,
    ) -> Result<Endpoint> {
        let setting = &self.settings[index];
        if setting == "system" {
            return Ok(Endpoint {
                resolver: None,
                expires: Instant::now() + Duration::from_secs(3600),
                network: None,
            });
        }
        let (url, route) = setting.split_once(',').unwrap_or((setting, "direct"));
        let mut network = None;
        let mut ttl = 3600;
        if route != "direct" {
            let node = self.nodes.get(route).ok_or(Error::InvalidInput)?;
            let (_node_ips, node_ttl) = self.bootstrap(&node.host, scope.clone()).await?;
            ttl = ttl.min(node_ttl);
            let resolved = Arc::new(crate::protocol::config::Node {
                host: node.host.clone(),
                port: node.port,
                sni: node.sni.clone(),
                public_key: node.public_key,
                short_id: node.short_id,
                uuid: node.uuid,
                fingerprint: node.fingerprint,
            });
            let manager = Arc::new(crate::outbound::Manager::default());
            manager.configure_dns_transport(
                Some((route.into(), resolved)),
                self.bootstrap.clone(),
                self.authority.clone(),
            )?;
            network = Some(manager);
        }
        let servers = if let Ok(address) = endpoint(url, 53) {
            vec![Server::Udp {
                address: address.to_string(),
            }]
        } else {
            let target = url::Url::parse(url).map_err(|_| Error::InvalidInput)?;
            let host = target
                .host_str()
                .ok_or(Error::InvalidInput)?
                .trim_matches(['[', ']']);
            let (ips, life) = self.bootstrap(host, scope.clone()).await?;
            ttl = ttl.min(life);
            let port = target
                .port()
                .unwrap_or(if target.scheme() == "tls" { 853 } else { 443 });
            ips.into_iter()
                .map(|ip| {
                    if target.scheme() == "tls" {
                        Server::Dot {
                            address: SocketAddr::new(ip, port).to_string(),
                            server_name: host.into(),
                        }
                    } else {
                        Server::Doh {
                            url: url.into(),
                            bootstrap: SocketAddr::new(ip, port).to_string(),
                        }
                    }
                })
                .collect()
        };
        let config = Config {
            servers,
            timeout_ms: self.timeout.as_millis() as u64,
            dnssec: self.dnssec,
            fake_ip: None,
            service: None,
        };
        let (servers, options) = config.build()?;
        let runtime = network
            .as_ref()
            .map_or_else(|| runtime.clone(), |n| runtime.routed(n.clone()));
        let mut tls = self.tls.clone();
        tls.alpn_protocols.clear();
        let resolver = UpstreamResolver::builder_with_config(servers, runtime)
            .with_options(options)
            .with_tls_config(tls)
            .build()
            .map_err(|_| Error::InvalidInput)?;
        Ok(Endpoint {
            resolver: Some(resolver),
            expires: Instant::now() + Duration::from_secs(ttl.into()),
            network,
        })
    }
    async fn initialize(
        &self,
        runtime: &ManagedRuntime,
        scope: Option<logging::Scope>,
    ) -> Result<()> {
        let mut endpoints = self.endpoints.write().await;
        if !endpoints.is_empty() {
            return Ok(());
        }
        let mut prepared: Vec<Endpoint> = Vec::with_capacity(self.len());
        for index in 0..self.len() {
            match self.build(index, runtime, scope.clone()).await {
                Ok(endpoint) => prepared.push(endpoint),
                Err(error) => {
                    for endpoint in prepared {
                        if let Some(network) = endpoint.network {
                            let _ = crate::error::diagnostics::observe(
                                network.stop_provider_download().await,
                                crate::error::diagnostics::DiagnosticSite::Cleanup,
                            );
                        }
                    }
                    return Err(error);
                }
            }
        }
        // 全部端点验证完成后一次发布；失败不能留下短于设置列表的部分索引。
        *endpoints = prepared;
        Ok(())
    }
    pub(super) fn hosts(&self, name: &str, kind: RecordType) -> Result<Option<Lookup>> {
        let Some(ip) = self.hosts.get(name) else {
            return Ok(None);
        };
        let ip: IpAddr = ip.parse().map_err(|_| Error::Internal)?;
        let data = match (kind, ip) {
            (RecordType::A, IpAddr::V4(ip)) => Some(RData::A(ip.into())),
            (RecordType::AAAA, IpAddr::V6(ip)) => Some(RData::AAAA(ip.into())),
            _ => None,
        };
        let question = Question::query(
            Name::from_ascii(name).map_err(|_| Error::InvalidInput)?,
            kind,
        );
        Ok(Some(Lookup::new_with_deadline(
            question.clone(),
            data.into_iter()
                .map(|data| Record::from_rdata(question.name().clone(), 60, data)),
            Instant::now() + Duration::from_secs(60),
        )))
    }
    pub(super) async fn lookup(
        &self,
        index: usize,
        name: &str,
        kind: RecordType,
        runtime: &ManagedRuntime,
        scope: Option<logging::Scope>,
    ) -> Result<std::result::Result<Lookup, NetError>> {
        if self.settings[index] == "system" {
            // OS 解析接口不提供 DNSSEC 证明；不得把无证明结果标成已验证。
            if self.dnssec {
                return Err(Error::DnsValidation);
            }
            let family = match kind {
                RecordType::A => crate::outbound::Family::Ipv4,
                RecordType::AAAA => crate::outbound::Family::Ipv6,
                _ => return Err(Error::Unsupported),
            };
            let values =
                crate::outbound::resolve_explicit_system(name, family, self.authority.clone())
                    .await?;
            let q = Question::query(
                Name::from_ascii(name).map_err(|_| Error::InvalidInput)?,
                kind,
            );
            let records = values
                .into_iter()
                .map(|ip| {
                    Record::from_rdata(
                        q.name().clone(),
                        0,
                        match ip {
                            IpAddr::V4(ip) => RData::A(ip.into()),
                            IpAddr::V6(ip) => RData::AAAA(ip.into()),
                        },
                    )
                })
                .collect::<Vec<_>>();
            return Ok(Ok(Lookup::new_with_deadline(q, records, Instant::now())));
        }
        if self.endpoints.read().await.is_empty() {
            self.initialize(runtime, scope.clone()).await?;
        }
        let mut endpoints = self.endpoints.read().await;
        if endpoints[index].expires <= Instant::now() {
            drop(endpoints);
            let mut update = self.endpoints.write().await;
            if update[index].expires <= Instant::now() {
                let replacement = self.build(index, runtime, scope.clone()).await?;
                let old = std::mem::replace(&mut update[index], replacement);
                if let Some(network) = old.network {
                    network.stop_provider_download().await?;
                }
            }
            drop(update);
            endpoints = self.endpoints.read().await;
        }
        let resolver = endpoints[index].resolver.as_ref().ok_or(Error::Internal)?;
        Ok(resolver.lookup(name.to_owned(), kind).await)
    }
    pub(super) fn close(
        &self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send + '_>> {
        Box::pin(async move {
            let mut result = Ok(());
            let endpoints = std::mem::take(&mut *self.endpoints.write().await);
            for endpoint in endpoints {
                if let Some(network) = endpoint.network
                    && let Err(e) = network.stop_provider_download().await
                {
                    result = Err(e);
                }
            }
            if let Some(bootstrap) = &self.bootstrap
                && let Err(e) = bootstrap.close().await
            {
                result = Err(e);
            }
            result
        })
    }
}

#[cfg(test)]
#[path = "../../tests/test_dns_main.rs"]
mod test_dns_main;
