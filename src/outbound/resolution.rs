//! 配置 DNS 到出站的桥接；没有系统解析和 hosts 回退，尚不消费 Fake-IP。
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
async fn resolve(resolver: &Resolver, name: &str, family: Family) -> Result<Vec<IpAddr>> {
    let query = |kind: &str| Query {
        name: name.into(),
        query_type: kind.into(),
    };
    let (v4, v6) = match family {
        Family::Auto => {
            let (v4, v6) = tokio::join!(
                resolver.resolve(query("A")),
                resolver.resolve(query("AAAA"))
            );
            (Some(v4), Some(v6))
        }
        Family::Ipv4 => (Some(resolver.resolve(query("A")).await), None),
        Family::Ipv6 => (None, Some(resolver.resolve(query("AAAA")).await)),
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
            let addresses = resolve(resolver, &name, target.family).await?;
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
                        let address = SocketAddr::new(addresses[next], target.port);
                        let transport = target.transport;
                        tasks.spawn(async move { dial(address, transport).await });
                        next += 1;
                        due = tokio::time::Instant::now() + Duration::from_millis(250);
                    }
                    else => return Err(failure),
                }
            }
        }).await
    }
}
