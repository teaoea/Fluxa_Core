use super::{
    Family, Manager, Opened, duration,
    resolution::{candidates, resolve},
};
use crate::{
    dns::Resolver,
    error::{Error, Result},
    routing::{Decision, Policy, Router, Target, Transport},
};
use serde::Deserialize;
use std::net::{IpAddr, SocketAddr};
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoutedTarget {
    pub target: Target,
    pub timeout_ms: u64,
    #[serde(default)]
    pub family: Family,
    #[serde(default)]
    pub mapping_generation: Option<u64>,
}
#[derive(Debug)]
pub struct RoutedOpened {
    pub connection: Opened,
    pub decision: Decision,
}
impl Manager {
    /// 节点 TCP 建连耗时；测量后立即关闭，不发送应用数据，不冒充代理握手验证。
    pub async fn probe_latency(&self, host: &str, port: u16, timeout_ms: u64) -> Result<u64> {
        let host = crate::network::normalize_host(host)?;
        if port == 0 {
            return Err(Error::InvalidInput);
        }
        let started = std::time::Instant::now();
        let id = if let Ok(ip) = host.parse::<IpAddr>() {
            self.open(
                SocketAddr::new(ip, port),
                Transport::Tcp,
                Policy::Direct,
                timeout_ms,
            )
            .await?
        } else {
            let snapshot = self.network_snapshot()?;
            let resolver = snapshot.resolver.as_ref().ok_or(Error::InvalidState)?;
            self.connect(
                resolver,
                super::DomainTarget {
                    domain: host,
                    port,
                    transport: Transport::Tcp,
                    policy: Policy::Direct,
                    timeout_ms,
                    family: Family::Auto,
                },
            )
            .await?
            .id
        };
        self.close(id)?;
        Ok(u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX))
    }

    /// 全部地址候选可执行后才拨号，不通过切换地址族绕过拒绝策略。
    pub async fn connect_routed(
        &self,
        router: &Router,
        resolver: Option<&Resolver>,
        request: RoutedTarget,
    ) -> Result<RoutedOpened> {
        self.connect_with_snapshot(router, resolver, request, self.network_snapshot()?, None)
            .await
    }
    /// 同一次读取 DNS、路由和资源参数，避免配置替换时混用。
    pub async fn connect_configured(&self, request: RoutedTarget) -> Result<RoutedOpened> {
        self.connect_configured_hop(request, None).await
    }
    pub(super) async fn connect_configured_hop(
        &self,
        request: RoutedTarget,
        hop: Option<SocketAddr>,
    ) -> Result<RoutedOpened> {
        let snapshot = self.network_snapshot()?;
        let router = snapshot.router.as_ref().ok_or(Error::InvalidState)?;
        self.connect_with_snapshot(
            router,
            snapshot.resolver.as_deref(),
            request,
            std::sync::Arc::clone(&snapshot),
            hop,
        )
        .await
    }
    async fn connect_with_snapshot(
        &self,
        router: &Router,
        resolver: Option<&Resolver>,
        request: RoutedTarget,
        snapshot: std::sync::Arc<super::NetworkSnapshot>,
        hop: Option<SocketAddr>,
    ) -> Result<RoutedOpened> {
        let deadline = duration(request.timeout_ms)?;
        let mut metadata = request.target;
        router.check_transport(metadata.transport)?;
        if metadata.port == 0 || metadata.domain.is_some() == metadata.address.is_some() {
            return Err(Error::InvalidInput);
        }
        let mut numeric = metadata
            .address
            .as_deref()
            .map(|value| {
                if value.len() > 64 {
                    return Err(Error::InvalidInput);
                }
                value.parse::<IpAddr>().map_err(|_| Error::InvalidInput)
            })
            .transpose()?;
        if let (Some(ip), Some(resolver)) = (numeric, resolver) {
            if resolver.is_fake_ip(ip) {
                let generation = request.mapping_generation.ok_or(Error::InvalidState)?;
                metadata.domain = Some(resolver.restore_fake_ip(&ip.to_string(), generation)?);
                metadata.address = None;
                numeric = None;
            } else if request.mapping_generation.is_some() {
                return Err(Error::InvalidInput);
            }
        } else if request.mapping_generation.is_some() {
            return Err(Error::InvalidInput);
        }
        if let Some(name) = &metadata.domain {
            if name.len() > 254 {
                return Err(Error::InvalidInput);
            }
            let name = crate::network::normalize_host(name)?;
            if name.parse::<IpAddr>().is_ok() {
                return Err(Error::InvalidInput);
            }
            metadata.domain = Some(name);
        }
        if numeric.is_some_and(|ip| {
            ip.is_unspecified()
                || matches!(
                    (request.family, ip),
                    (Family::Ipv4, IpAddr::V6(_)) | (Family::Ipv6, IpAddr::V4(_))
                )
        }) {
            return Err(Error::InvalidInput);
        }
        let mut decision = None;
        let opened = self
            .establish_guarded(
                std::sync::Arc::clone(&snapshot),
                deadline,
                router.blocks_quic(),
                async {
                    if let Some(early) = router.evaluate_before_dns(metadata.clone())? {
                        check(&snapshot, &early, metadata.transport)?;
                    }
                    let addresses = if let Some(ip) = numeric {
                        vec![ip]
                    } else {
                        let resolver = resolver.ok_or(Error::InvalidState)?;
                        resolve(
                            resolver,
                            metadata.domain.as_deref().ok_or(Error::InvalidInput)?,
                            request.family,
                            true,
                        )
                        .await?
                    };
                    let mut plans = Vec::with_capacity(addresses.len());
                    for ip in addresses {
                        let mut target = metadata.clone();
                        target.address = Some(ip.to_string());
                        let route = router.evaluate(target)?;
                        check(&snapshot, &route, metadata.transport)?;
                        plans.push((SocketAddr::new(ip, metadata.port), route));
                    }
                    let preferred = plans.first().ok_or(Error::DnsResponse)?;
                    // VLESS 无独立目标连接确认；自动模式优先同一路由的 IPv4，不重放已发送的数据。
                    let first = if preferred.1.policy == Policy::Proxy
                        && matches!(request.family, Family::Auto)
                    {
                        plans
                            .iter()
                            .find(|(a, r)| {
                                a.is_ipv4()
                                    && r.policy == preferred.1.policy
                                    && r.outbound == preferred.1.outbound
                            })
                            .unwrap_or(preferred)
                    } else {
                        preferred
                    };
                    let dialed = if first.1.policy == Policy::Proxy {
                        if hop.is_some() {
                            return Err(Error::Unsupported);
                        }
                        let node = snapshot
                            .outbounds
                            .get(first.1.outbound.as_deref().ok_or(Error::InvalidInput)?)
                            .ok_or(Error::Unsupported)?;
                        let node_ips = if let Ok(ip) = node.host.parse::<IpAddr>() {
                            vec![ip]
                        } else {
                            resolve(
                                resolver.ok_or(Error::InvalidState)?,
                                &node.host,
                                super::Family::Auto,
                                true,
                            )
                            .await?
                        };
                        if node_ips.iter().any(|ip| {
                            ip.is_unspecified() || resolver.is_some_and(|r| r.is_fake_ip(*ip))
                        }) {
                            return Err(Error::InvalidInput);
                        }
                        let lease = super::CONNECTIONS.reserve(1)?;
                        let mut connected = None;
                        for ip in node_ips {
                            match tokio::net::TcpStream::connect(SocketAddr::new(ip, node.port))
                                .await
                            {
                                Ok(socket) => {
                                    connected = Some(socket);
                                    break;
                                }
                                Err(_) => continue,
                            }
                        }
                        let socket = connected.ok_or(Error::Network)?;
                        socket.set_nodelay(true)?;
                        // 已检查的真实候选作为 VLESS 目标，不让远端重新解析后切换到未经检查的地址。
                        // 协议 future 独立置于有界连接预算内，避免放大 Swift 宿主线程的调用栈。
                        let outbound = match metadata.transport {
                            Transport::Tcp => {
                                let stream = Box::pin(crate::protocol::connect(
                                    socket,
                                    node,
                                    &first.0.ip().to_string(),
                                    metadata.port,
                                ))
                                .await?;
                                super::tcp_socket(Box::new(stream))
                            }
                            Transport::Udp => super::Socket::ProxyUdp {
                                socket: Box::pin(crate::protocol::connect_udp(
                                    socket, node, first.0,
                                ))
                                .await?,
                            },
                        };
                        (outbound, lease, first.0)
                    } else {
                        let approved = plans
                            .iter()
                            .filter(|(_, r)| r.policy == first.1.policy)
                            .map(|(a, _)| *a)
                            .collect();
                        candidates(
                            hop.map(|hop| vec![hop]).unwrap_or(approved),
                            metadata.transport,
                        )
                        .await?
                    };
                    decision = Some(
                        plans
                            .into_iter()
                            .find(|(address, _)| hop.is_some() || *address == dialed.2)
                            .ok_or(Error::Internal)?
                            .1,
                    );
                    Ok(dialed)
                },
            )
            .await?;
        let Some(decision) = decision else {
            self.close(opened.id)?;
            return Err(Error::Internal);
        };
        Ok(RoutedOpened {
            connection: opened,
            decision,
        })
    }
}

fn check(
    snapshot: &super::NetworkSnapshot,
    decision: &Decision,
    _transport: Transport,
) -> Result<()> {
    match decision.policy {
        Policy::Reject => Err(Error::Rejected),
        Policy::Direct => Ok(()),
        Policy::Proxy => {
            let id = decision.outbound.as_deref().ok_or(Error::InvalidInput)?;
            if snapshot.outbounds.contains_key(id) {
                Ok(())
            } else {
                Err(Error::Unsupported)
            }
        }
    }
}
