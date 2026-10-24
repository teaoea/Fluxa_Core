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
/// 只能由内核入口补充的报文/系统元组，不参与公开请求反序列化。
#[derive(Clone, Copy, Default)]
struct FlowFacts<'a> {
    connection_id: Option<u64>,
    datagram: Option<&'a [u8]>,
    source: Option<crate::platform::process::Flow>,
    permission: Option<&'a crate::dns::ResolverOperation>,
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
        self.close(id).await?;
        Ok(u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX))
    }

    /// 逐个排除被 IP 规则拒绝的候选，仅对剩余允许地址拨号；域名级 REJECT 仍拒绝整个请求。
    pub async fn connect_routed(
        &self,
        router: &Router,
        resolver: Option<&Resolver>,
        request: RoutedTarget,
    ) -> Result<RoutedOpened> {
        self.connect_with_snapshot(
            router,
            resolver,
            request,
            self.network_snapshot()?,
            None,
            FlowFacts::default(),
        )
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
        self.connect_configured_snapshot_hop(request, snapshot, hop)
            .await
    }
    /// TUN 捕获同一份网络配置，复用既有路由和出站，不复制连接流程。
    pub(crate) async fn connect_configured_snapshot(
        &self,
        request: RoutedTarget,
        snapshot: std::sync::Arc<super::NetworkSnapshot>,
        source: SocketAddr,
    ) -> Result<RoutedOpened> {
        let destination = SocketAddr::new(
            request
                .target
                .address
                .as_deref()
                .ok_or(Error::InvalidInput)?
                .parse()
                .map_err(|_| Error::InvalidInput)?,
            request.target.port,
        );
        let flow = crate::platform::process::Flow {
            local: source,
            remote: destination,
            transport: request.target.transport,
        };
        let router = snapshot.router.as_ref().ok_or(Error::InvalidState)?;
        self.connect_with_snapshot(
            router,
            snapshot.resolver.as_deref(),
            request,
            snapshot.clone(),
            None,
            FlowFacts {
                connection_id: None,
                datagram: None,
                source: Some(flow),
                permission: None,
            },
        )
        .await
    }
    pub(super) async fn connect_configured_snapshot_hop(
        &self,
        request: RoutedTarget,
        snapshot: std::sync::Arc<super::NetworkSnapshot>,
        hop: Option<SocketAddr>,
    ) -> Result<RoutedOpened> {
        let router = snapshot.router.as_ref().ok_or(Error::InvalidState)?;
        self.connect_with_snapshot(
            router,
            snapshot.resolver.as_deref(),
            request,
            std::sync::Arc::clone(&snapshot),
            hop,
            FlowFacts::default(),
        )
        .await
    }
    /// TUN 已有真实首包时先裁决再拨号；不接受 JSON 自报的 QUIC 标志。
    pub(crate) async fn connect_configured_datagram_snapshot(
        &self,
        request: RoutedTarget,
        snapshot: std::sync::Arc<super::NetworkSnapshot>,
        bytes: &[u8],
        source: SocketAddr,
    ) -> Result<RoutedOpened> {
        let destination = SocketAddr::new(
            request
                .target
                .address
                .as_deref()
                .ok_or(Error::InvalidInput)?
                .parse()
                .map_err(|_| Error::InvalidInput)?,
            request.target.port,
        );
        let flow = crate::platform::process::Flow {
            local: source,
            remote: destination,
            transport: request.target.transport,
        };
        let router = snapshot.router.as_ref().ok_or(Error::InvalidState)?;
        self.connect_with_snapshot(
            router,
            snapshot.resolver.as_deref(),
            request,
            snapshot.clone(),
            None,
            FlowFacts {
                connection_id: None,
                datagram: Some(bytes),
                source: Some(flow),
                permission: None,
            },
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
        facts: FlowFacts<'_>,
    ) -> Result<RoutedOpened> {
        let connection_id = crate::lifecycle::identifier().ok_or(Error::ResourceLimit)?;
        let revision = snapshot.revision;
        self.logs.event(
            crate::logging::Event::ConnectionCreated,
            None,
            Some(revision),
            None,
            None,
            crate::logging::Context {
                connection_id: Some(connection_id),
                ..Default::default()
            },
        );
        let result = async {
            // 两族尝试属于同一建连事务；退休仅禁止新的请求，不能在族切换时取消已准入请求。
            let admitted = snapshot
                .resolver
                .as_ref()
                .map(|r| r.operation())
                .transpose()?;
            let facts = FlowFacts {
                connection_id: Some(connection_id),
                permission: admitted.as_ref(),
                ..facts
            };
            use crate::configuration::models::IpStrategy;
            if snapshot.configuration_complete {
                let enforced = match snapshot.ip_strategy {
                    IpStrategy::OnlyIpv4 => Some(Family::Ipv4),
                    IpStrategy::OnlyIpv6 => Some(Family::Ipv6),
                    _ => None,
                };
                if let Some(family) = enforced
                    && !matches!(request.family, Family::Auto)
                    && std::mem::discriminant(&request.family) != std::mem::discriminant(&family)
                {
                    return Err(Error::InvalidInput);
                }
                if request.target.address.is_some() && request.mapping_generation.is_none() {
                    let mut request = request;
                    if let Some(family) = enforced {
                        request.family = family;
                    }
                    return self
                        .connect_family(
                            router,
                            resolver,
                            request,
                            snapshot,
                            (hop, facts),
                            &mut false,
                        )
                        .await;
                }
            }
            // 主配置的地址族策略由内核裁决；不能由普通 API 的 Auto 实现并发查询两族。
            if !snapshot.configuration_complete || !matches!(request.family, Family::Auto) {
                return self
                    .connect_family(
                        router,
                        resolver,
                        request,
                        snapshot,
                        (hop, facts),
                        &mut false,
                    )
                    .await;
            }
            let (first, second) = match snapshot.ip_strategy {
                IpStrategy::PreferIpv4 => (Family::Ipv4, Some(Family::Ipv6)),
                IpStrategy::PreferIpv6 => (Family::Ipv6, Some(Family::Ipv4)),
                IpStrategy::OnlyIpv4 => (Family::Ipv4, None),
                IpStrategy::OnlyIpv6 => (Family::Ipv6, None),
            };
            let mut attempt = request.clone();
            attempt.family = first;
            let mut empty = false;
            let result = self
                .connect_family(
                    router,
                    resolver,
                    attempt,
                    snapshot.clone(),
                    (hop, facts),
                    &mut empty,
                )
                .await;
            let retry = empty
                || matches!(
                    result,
                    Err(Error::Network
                        | Error::NetworkTimeout
                        | Error::NetworkConnectRefused
                        | Error::DnsTimeout)
                );
            // NXDOMAIN、DNSSEC、资源/状态失败以及所有候选被拦截均不可触发另一族放行。
            if result.is_err()
                && retry
                && let Some(second) = second
            {
                let mut request = request;
                request.family = second;
                return self
                    .connect_family(
                        router,
                        resolver,
                        request,
                        snapshot,
                        (hop, facts),
                        &mut false,
                    )
                    .await;
            }
            result
        }
        .await;
        if let Err(error) = result {
            self.logs.event(
                if error == Error::Rejected {
                    crate::logging::Event::PolicyBlocked
                } else {
                    crate::logging::Event::ConnectionFailed
                },
                None,
                Some(revision),
                None,
                Some(error),
                crate::logging::Context {
                    connection_id: Some(connection_id),
                    ..Default::default()
                },
            );
        }
        result
    }
    async fn connect_family(
        &self,
        router: &Router,
        resolver: Option<&Resolver>,
        request: RoutedTarget,
        snapshot: std::sync::Arc<super::NetworkSnapshot>,
        context: (Option<SocketAddr>, FlowFacts<'_>),
        empty_family: &mut bool,
    ) -> Result<RoutedOpened> {
        let (hop, facts) = context;
        let FlowFacts {
            connection_id,
            datagram,
            source,
            permission: admitted,
        } = facts;
        let connection_id = connection_id.ok_or(Error::Internal)?;
        snapshot.ensure_operation(admitted)?;
        let deadline = duration(request.timeout_ms)?;
        let mut metadata = request.target.clone();

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
        let quic = datagram
            .map(|bytes| crate::routing::runtime::Snapshot::identify_datagram(&metadata, bytes))
            .transpose()?
            .unwrap_or(false);
        if datagram.is_none() && router.needs_datagram(&metadata)? {
            return Err(Error::Unsupported);
        }
        router.check_before_dns_transport(&metadata)?;
        let mut decision = None;
        let expires = tokio::time::Instant::now() + deadline;
        let opened = self
            .establish_guarded(
                std::sync::Arc::clone(&snapshot),
                deadline,
                admitted,
                connection_id,
                async |permission| {
                    // 系统归属查询也属于本次准入/总超时，不在会话取消与建连截止之外运行。
                    let identity = if router.needs_process() {
                        crate::platform::process::identify(source).await?
                    } else {
                        crate::platform::process::Identity {
                            name: None,
                            token: None,
                        }
                    };
                    if let Some(early) = router.before_dns_flow(
                        metadata.clone(),
                        datagram,
                        identity.name.as_deref(),
                    )? {
                        self.logs.event(
                            crate::logging::Event::RouteMatched,
                            None,
                            Some(snapshot.revision),
                            None,
                            None,
                            crate::logging::Context::route(connection_id, &early),
                        );
                        check(&snapshot, &early, metadata.transport)?;
                    }
                    let addresses = if let Some(ip) = numeric {
                        vec![ip]
                    } else {
                        if let Some(resolver) = resolver {
                            resolve(
                                resolver,
                                metadata.domain.as_deref().ok_or(Error::InvalidInput)?,
                                request.family,
                                true,
                                permission,
                                Some(crate::dns::logging::Scope::new(
                                    self.logs.clone(),
                                    Some(connection_id),
                                    Some(snapshot.revision),
                                )),
                            )
                            .await?
                        } else if snapshot.system_dns {
                            super::resolution::resolve_system_logged(
                                metadata.domain.as_deref().ok_or(Error::InvalidInput)?,
                                request.family,
                                snapshot.owner()?,
                                Some(crate::dns::logging::Scope::new(
                                    self.logs.clone(),
                                    Some(connection_id),
                                    Some(snapshot.revision),
                                )),
                            )
                            .await?
                        } else {
                            return Err(Error::InvalidState);
                        }
                    };
                    if addresses.is_empty() {
                        *empty_family = true;
                        return Err(Error::DnsResponse);
                    }
                    let mut plans = Vec::with_capacity(addresses.len());
                    for ip in addresses {
                        let mut target = metadata.clone();
                        target.address = Some(ip.to_string());
                        match router
                            .protection_facts(target.clone(), quic, identity.name.as_deref())?
                            .check(quic)
                        {
                            Ok(()) => (),
                            // IP REJECT 只排除此候选，不能扩大为整个域名请求的 REJECT。
                            Err(Error::Rejected) => continue,
                            Err(error) => return Err(error),
                        }
                        let route =
                            router.evaluate_flow(target, datagram, identity.name.as_deref())?;
                        match check(&snapshot, &route, metadata.transport) {
                            Ok(()) => (),
                            Err(Error::Rejected) => continue,
                            Err(error) => return Err(error),
                        }
                        plans.push((SocketAddr::new(ip, metadata.port), route));
                    }
                    let preferred = plans.first().ok_or(Error::Rejected)?;
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
                    self.logs.event(
                        crate::logging::Event::RouteMatched,
                        None,
                        Some(snapshot.revision),
                        None,
                        None,
                        crate::logging::Context::route(connection_id, &first.1),
                    );
                    // 路由优先级只计算一次；显式候选顺序在同一操作/预算/总截止内执行。
                    let chain = first.1.outbound.as_deref().is_some_and(|v| v.contains(','));
                    let members = if first.1.policy == Policy::Proxy {
                        crate::routing::strategy::members(
                            first.1.outbound.as_deref().ok_or(Error::InvalidInput)?,
                        )?
                    } else {
                        vec!["direct"]
                    };
                    let mut selected = None;
                    let mut last = Error::Network;
                    for (index, candidate) in members.iter().copied().enumerate() {
                        snapshot.ensure_operation(permission)?;
                        if index > 0 {
                            self.logs.event(
                                crate::logging::Event::NodeRetry,
                                None,
                                Some(snapshot.revision),
                                None,
                                Some(last),
                                crate::logging::Context {
                                    connection_id: Some(connection_id),
                                    node: crate::logging::NodeName::new(members[index - 1]),
                                    next_node: crate::logging::NodeName::new(candidate),
                                    ..Default::default()
                                },
                            );
                        }
                        self.logs.event(
                            crate::logging::Event::NodeConnecting,
                            None,
                            Some(snapshot.revision),
                            None,
                            None,
                            crate::logging::Context {
                                connection_id: Some(connection_id),
                                node: crate::logging::NodeName::new(candidate),
                                ..Default::default()
                            },
                        );
                        let direct =
                            first.1.policy == Policy::Direct || (chain && candidate == "direct");
                        if chain && candidate == "reject" {
                            return Err(Error::Rejected);
                        }
                        let remaining =
                            expires.saturating_duration_since(tokio::time::Instant::now());
                        let allowance = remaining
                            / u32::try_from(members.len() - index)
                                .map_err(|_| Error::ResourceLimit)?;
                        // 每次克隆连接请求元数据；没有应用正文缓存，也不重放已有业务流。
                        let mut retry_request = request.clone();
                        retry_request.target = metadata.clone();
                        let attempt = tokio::time::timeout(allowance, async {
                            if !direct {
                                if hop.is_some() {
                                    return Err(Error::Unsupported);
                                }
                                let node = snapshot
                                    .outbounds
                                    .get(candidate)
                                    .ok_or(Error::Unsupported)?;
                                let node_ips = if let Ok(ip) = node.host.parse::<IpAddr>() {
                                    vec![ip]
                                } else {
                                    if let Some(resolver) = resolver {
                                        resolve(
                                            resolver,
                                            &node.host,
                                            retry_request.family,
                                            true,
                                            permission,
                                            Some(crate::dns::logging::Scope::new(
                                                self.logs.clone(),
                                                Some(connection_id),
                                                Some(snapshot.revision),
                                            )),
                                        )
                                        .await?
                                    } else if snapshot.system_dns {
                                        super::resolution::resolve_system_logged(
                                            &node.host,
                                            retry_request.family,
                                            snapshot.owner()?,
                                            Some(crate::dns::logging::Scope::new(
                                                self.logs.clone(),
                                                Some(connection_id),
                                                Some(snapshot.revision),
                                            )),
                                        )
                                        .await?
                                    } else {
                                        return Err(Error::InvalidState);
                                    }
                                };
                                if node_ips.iter().any(|ip| {
                                    ip.is_unspecified()
                                        || resolver.is_some_and(|r| r.is_fake_ip(*ip))
                                }) {
                                    return Err(Error::InvalidInput);
                                }
                                let lease = super::CONNECTIONS.reserve(1)?;
                                let mut connected = None;
                                for ip in node_ips {
                                    match tokio::net::TcpStream::connect(SocketAddr::new(
                                        ip, node.port,
                                    ))
                                    .await
                                    {
                                        Ok(socket) => {
                                            connected = Some(socket);
                                            break;
                                        }
                                        Err(error) => {
                                            crate::error::diagnostics::record(
                                        error.into(),
                                        crate::error::diagnostics::DiagnosticSite::IoConversion,
                                    );
                                            continue;
                                        }
                                    }
                                }
                                let socket = connected.ok_or(Error::Network)?;
                                socket.set_nodelay(true)?;
                                self.logs.event(
                                    crate::logging::Event::NodeHandshake,
                                    None,
                                    Some(snapshot.revision),
                                    None,
                                    None,
                                    crate::logging::Context {
                                        connection_id: Some(connection_id),
                                        node: crate::logging::NodeName::new(candidate),
                                        ..Default::default()
                                    },
                                );
                                // 已检查的真实候选作为 VLESS 目标，不让远端重新解析后切换到未经检查的地址。
                                // 协议 future 独立置于有界连接预算内，避免放大 Swift 宿主线程的调用栈。
                                let outbound = match retry_request.target.transport {
                                    Transport::Tcp => {
                                        let stream = Box::pin(crate::protocol::connect(
                                            socket,
                                            node,
                                            &first.0.ip().to_string(),
                                            retry_request.target.port,
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
                                Ok((outbound, lease, first.0))
                            } else {
                                let approved = plans
                                    .iter()
                                    .filter(|(_, route)| {
                                        route.policy == first.1.policy
                                            && route.outbound == first.1.outbound
                                    })
                                    .map(|(address, _)| *address)
                                    .collect();
                                candidates(
                                    hop.map(|hop| vec![hop]).unwrap_or(approved),
                                    retry_request.target.transport,
                                )
                                .await
                            }
                        })
                        .await
                        .unwrap_or(Err(Error::NetworkTimeout));
                        match attempt {
                            Ok(dialed) => {
                                self.logs.event(
                                    crate::logging::Event::NodeConnected,
                                    None,
                                    Some(snapshot.revision),
                                    None,
                                    None,
                                    crate::logging::Context {
                                        connection_id: Some(connection_id),
                                        node: crate::logging::NodeName::new(candidate),
                                        ..Default::default()
                                    },
                                );
                                selected = Some((dialed, direct, candidate.to_owned()));
                                break;
                            }
                            Err(error) => {
                                self.logs.event(
                                    crate::logging::Event::NodeFailed,
                                    None,
                                    Some(snapshot.revision),
                                    None,
                                    Some(error),
                                    crate::logging::Context {
                                        connection_id: Some(connection_id),
                                        node: crate::logging::NodeName::new(candidate),
                                        ..Default::default()
                                    },
                                );
                                // 只允许尚未交付业务流时的建连故障切换；状态/预算/取消绝不重试。
                                if direct
                                    || !matches!(
                                        error,
                                        Error::Network
                                            | Error::NetworkTimeout
                                            | Error::NetworkConnectRefused
                                            | Error::DnsTimeout
                                    )
                                {
                                    return Err(error);
                                }
                                crate::error::diagnostics::record(
                                    error,
                                    crate::error::diagnostics::DiagnosticSite::IoConversion,
                                );
                                last = error;
                            }
                        }
                    }
                    let (dialed, direct, candidate) = selected.ok_or(last)?;
                    let mut checked_target = metadata.clone();
                    checked_target.address = Some(
                        plans
                            .iter()
                            .find(|(address, _)| hop.is_some() || *address == dialed.2)
                            .ok_or(Error::Internal)?
                            .0
                            .ip()
                            .to_string(),
                    );
                    // OS 身份仅用于这次新连接准入，DNS/拨号期间发生 FD/PID 复用则不能提交。
                    // 已建立连接的权限和释放由内核会话租约负责，不把原进程 FD 生命周期变成连接控制权。
                    if let Some(owner) = &identity.token
                        && !owner.validate()?
                    {
                        return Err(Error::InvalidState);
                    }
                    let protection =
                        router.protection_facts(checked_target, quic, identity.name.as_deref())?;
                    protection.check(false)?;
                    let mut actual = plans
                        .into_iter()
                        .find(|(address, _)| hop.is_some() || *address == dialed.2)
                        .ok_or(Error::Internal)?
                        .1;
                    actual.policy = if direct {
                        Policy::Direct
                    } else {
                        Policy::Proxy
                    };
                    self.logs.event(
                        crate::logging::Event::RouteSelected,
                        None,
                        Some(snapshot.revision),
                        None,
                        None,
                        crate::logging::Context::route(connection_id, &actual),
                    );
                    actual.outbound = (!direct).then_some(candidate);
                    decision = Some(actual);
                    Ok((dialed, protection))
                },
            )
            .await?;
        let Some(decision) = decision else {
            self.close(opened.id).await?;
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
            let members = crate::routing::strategy::members(id)?;
            let chain = id.contains(',');
            if members.iter().all(|name| {
                (chain && matches!(*name, "direct" | "reject"))
                    || snapshot.outbounds.contains_key(*name)
            }) {
                Ok(())
            } else {
                Err(Error::Unsupported)
            }
        }
    }
}

#[cfg(test)]
#[path = "../../tests/test_outbound_execution.rs"]
mod test_outbound_execution;
