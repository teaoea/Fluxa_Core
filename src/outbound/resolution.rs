//! 配置 DNS 与 Fake-IP 到出站的桥接；不使用系统解析或 hosts 回退。
use super::{Manager, dial, duration};
use crate::{
    dns::{Answer, Query, Resolver},
    error::{Error, Result},
    routing::{Policy, Transport},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    net::{IpAddr, SocketAddr},
    time::Duration,
};
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Family {
    #[default]
    Auto,
    Ipv4,
    Ipv6,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DomainTarget {
    pub domain: String,
    pub port: u16,
    pub transport: Transport,
    pub policy: Policy,
    pub timeout_ms: u64,
    #[serde(default)]
    pub family: Family,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FakeIPTarget {
    pub address: String,
    pub mapping_generation: u64,
    pub transport: Transport,
    pub policy: Policy,
    pub timeout_ms: u64,
    #[serde(default)]
    pub family: Family,
}
#[derive(Debug, Serialize)]
pub struct Opened {
    pub id: u64,
    pub address: String,
}
fn addresses(answer: Answer, v4: bool) -> Result<Vec<IpAddr>> {
    if answer.validation == "synthetic" {
        return Err(Error::Unsupported);
    }
    if answer.rcode != 0 {
        return Err(Error::DnsResponse);
    }
    if answer.addresses.len() > 16 {
        return Err(Error::ResourceLimit);
    }
    let mut seen = BTreeSet::new();
    let mut values = Vec::new();
    for value in answer.addresses {
        let ip: IpAddr = value.parse().map_err(|_| Error::DnsResponse)?;
        if ip.is_ipv4() != v4 || ip.is_unspecified() {
            return Err(Error::DnsResponse);
        }
        if seen.insert(ip) {
            values.push(ip);
        }
    }
    Ok(values)
}
pub(super) async fn resolve(
    resolver: &Resolver,
    name: &str,
    family: Family,
    real: bool,
) -> Result<Vec<IpAddr>> {
    let query = |kind: &str| Query {
        name: name.into(),
        query_type: kind.into(),
    };
    let lookup = |query| async move {
        if real {
            resolver.resolve_real(query).await
        } else {
            resolver.resolve(query).await
        }
    };
    let (v4, v6) = match family {
        Family::Auto => {
            let (v4, v6) = tokio::join!(lookup(query("A")), lookup(query("AAAA")));
            (Some(v4), Some(v6))
        }
        Family::Ipv4 => (Some(lookup(query("A")).await), None),
        Family::Ipv6 => (None, Some(lookup(query("AAAA")).await)),
    };
    // 验证/资源错误不能被另一地址族掩盖；单族网络失败可以使用另一族的有效记录。
    let mut failure = Error::DnsResponse;
    let mut lists = Vec::new();
    for (answer, v4) in [(v6, false), (v4, true)] {
        let Some(answer) = answer else {
            lists.push(Vec::new());
            continue;
        };
        match answer {
            Ok(answer) => lists.push(addresses(answer, v4)?),
            Err(
                error @ (Error::DnsValidation
                | Error::ResourceLimit
                | Error::InvalidInput
                | Error::Unsupported),
            ) => return Err(error),
            Err(error) => {
                failure = error;
                lists.push(Vec::new());
            }
        }
    }
    let mut values = Vec::new();
    let maximum = lists.iter().map(Vec::len).max().unwrap_or(0);
    for index in 0..maximum {
        for list in &lists {
            if let Some(ip) = list.get(index) {
                values.push(*ip);
            }
        }
    }
    if values.len() > 16 {
        return Err(Error::ResourceLimit);
    }
    if values.is_empty() {
        return Err(failure);
    }
    Ok(values)
}
impl Manager {
    pub async fn connect(&self, resolver: &Resolver, target: DomainTarget) -> Result<Opened> {
        self.connect_domain(resolver, target, false).await
    }
    pub async fn connect_fake_ip(
        &self,
        resolver: &Resolver,
        target: FakeIPTarget,
    ) -> Result<Opened> {
        target.policy.check_support()?;
        let address = super::endpoint(&target.address)?;
        let domain =
            resolver.restore_fake_ip(&address.ip().to_string(), target.mapping_generation)?;
        self.connect_domain(
            resolver,
            DomainTarget {
                domain,
                port: address.port(),
                transport: target.transport,
                policy: target.policy,
                timeout_ms: target.timeout_ms,
                family: target.family,
            },
            true,
        )
        .await
    }
    async fn connect_domain(
        &self,
        resolver: &Resolver,
        target: DomainTarget,
        real: bool,
    ) -> Result<Opened> {
        let deadline = duration(target.timeout_ms)?;
        target.policy.check_support()?;
        if target.domain.len() > 254 || target.port == 0 {
            return Err(Error::InvalidInput);
        }
        let name = crate::network::normalize_host(&target.domain)?;
        if name.parse::<IpAddr>().is_ok() {
            return Err(Error::InvalidInput);
        }
        self.establish(deadline, async {
            let addresses = resolve(resolver, &name, target.family, real).await?;
            candidates(
                addresses
                    .into_iter()
                    .map(|ip| SocketAddr::new(ip, target.port))
                    .collect(),
                target.transport,
            )
            .await
        })
        .await
    }
}

pub(super) async fn candidates(
    addresses: Vec<SocketAddr>,
    transport: Transport,
) -> Result<super::Dialed> {
    let mut tasks = tokio::task::JoinSet::new();
    let mut next = 0;
    let mut failure = Error::Network;
    let mut due = tokio::time::Instant::now();
    loop {
        tokio::select! {
            result = tasks.join_next(), if !tasks.is_empty() => {
                match result {
                    Some(Ok(Ok(socket))) => return Ok(socket),
                    Some(Ok(Err(error))) => { failure = error; if tasks.is_empty() { due = tokio::time::Instant::now(); } },
                    _ => return Err(Error::Internal),
                }
            }
            _ = tokio::time::sleep_until(due), if next < addresses.len() => {
                let address = addresses[next];
                tasks.spawn(async move { dial(address, transport).await });
                next += 1;
                due = tokio::time::Instant::now() + Duration::from_millis(250);
            }
            else => return Err(failure),
        }
    }
}
