//! 纯路由决策：编译一次、按顺序匹配，不解析 DNS、不建连接、不隐式切换出站。
use crate::{
    budget::{ByteBudget, ByteLease},
    error::{Error, Result},
    network::Network,
};
use serde::{Deserialize, Serialize};
use std::net::IpAddr;
static BUDGET: ByteBudget = ByteBudget::new(16 * 1024 * 1024);
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Direct,
    #[default]
    Rules,
    Global,
}
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Policy {
    Direct,
    #[default]
    Reject,
    Proxy,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Transport {
    Tcp,
    Udp,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Match {
    Domain,
    DomainSuffix,
    IpCidr,
    DestinationPort,
    Transport,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Rule {
    pub r#type: Match,
    pub value: String,
    pub policy: Policy,
    #[serde(default)]
    pub outbound: Option<String>,
}
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default)]
    pub mode: Mode,
    #[serde(default)]
    pub fallback: Policy,
    #[serde(default)]
    pub outbound: Option<String>,
    #[serde(default)]
    pub rules: Vec<Rule>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Target {
    #[serde(default)]
    pub domain: Option<String>,
    #[serde(default)]
    pub address: Option<String>,
    pub port: u16,
    pub transport: Transport,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Reason {
    Mode,
    Rule,
    Fallback,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Decision {
    pub policy: Policy,
    pub outbound: Option<String>,
    pub rule_index: Option<usize>,
    pub reason: Reason,
}
impl Policy {
    /// 共用策略能力检查，不建立连接，不将未实现代理转换为直连。
    pub fn check_support(self) -> Result<()> {
        match self {
            Self::Direct => Ok(()),
            Self::Reject => Err(Error::Rejected),
            Self::Proxy => Err(Error::Unsupported),
        }
    }
}
impl Decision {
    pub fn check_support(&self) -> Result<()> {
        self.policy.check_support()
    }
}

enum Matcher {
    Domain(String, bool),
    Network(Network),
    Port(u16, u16),
    Transport(Transport),
}
struct CompiledRule {
    matcher: Matcher,
    policy: Policy,
    outbound: Option<String>,
}
pub struct Router {
    mode: Mode,
    fallback: Policy,
    outbound: Option<String>,
    rules: Vec<CompiledRule>,
    _lease: ByteLease,
}
fn identifier(value: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > 64
        || !value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
    {
        return Err(Error::InvalidInput);
    }
    Ok(())
}
fn domain(value: &str) -> Result<String> {
    if value.len() > 254 {
        return Err(Error::InvalidInput);
    }
    let value = crate::network::normalize_host(value)?;
    if value.parse::<IpAddr>().is_ok() {
        return Err(Error::InvalidInput);
    }
    Ok(value)
}
fn policy_outbound(
    policy: Policy,
    own: Option<String>,
    default: &Option<String>,
) -> Result<Option<String>> {
    if policy != Policy::Proxy {
        if own.is_some() {
            return Err(Error::InvalidInput);
        }
        return Ok(None);
    }
    let result = own.or_else(|| default.clone()).ok_or(Error::InvalidInput)?;
    identifier(&result)?;
    Ok(Some(result))
}
fn matcher(kind: Match, value: &str) -> Result<Matcher> {
    if value.is_empty() || value.len() > 253 {
        return Err(Error::InvalidInput);
    }
    Ok(match kind {
        Match::Domain => Matcher::Domain(domain(value)?, false),
        Match::DomainSuffix => Matcher::Domain(domain(value)?, true),
        Match::IpCidr => Matcher::Network(Network::parse(value)?),
        Match::Transport => Matcher::Transport(match value {
            "tcp" => Transport::Tcp,
            "udp" => Transport::Udp,
            _ => return Err(Error::InvalidInput),
        }),
        Match::DestinationPort => {
            let (first, last) = value.split_once('-').unwrap_or((value, value));
            if !first.bytes().all(|b| b.is_ascii_digit())
                || !last.bytes().all(|b| b.is_ascii_digit())
            {
                return Err(Error::InvalidInput);
            }
            let first: u16 = first.parse().map_err(|_| Error::InvalidInput)?;
            let last: u16 = last.parse().map_err(|_| Error::InvalidInput)?;
            if first == 0 || first > last {
                return Err(Error::InvalidInput);
            }
            Matcher::Port(first, last)
        }
    })
}
impl Router {
    pub fn compile(config: Config) -> Result<Self> {
        if config.rules.len() > 10000 {
            return Err(Error::ResourceLimit);
        }
        if let Some(outbound) = &config.outbound {
            identifier(outbound)?;
        }
        if (config.mode == Mode::Global || config.fallback == Policy::Proxy)
            && config.outbound.is_none()
        {
            return Err(Error::InvalidInput);
        }
        // 单条预留覆盖值、匹配器、出站和 Vec 空间；临时 JSON 输入另受 FFI 上限控制。
        let lease = BUDGET.reserve(1024 + config.rules.len() * 1024)?;
        let mut rules = Vec::with_capacity(config.rules.len());
        for rule in config.rules {
            rules.push(CompiledRule {
                matcher: matcher(rule.r#type, &rule.value)?,
                policy: rule.policy,
                outbound: policy_outbound(rule.policy, rule.outbound, &config.outbound)?,
            });
        }
        Ok(Self {
            mode: config.mode,
            fallback: config.fallback,
            outbound: config.outbound,
            rules,
            _lease: lease,
        })
    }
    pub fn evaluate(&self, target: Target) -> Result<Decision> {
        self.evaluate_inner(target, false)?.ok_or(Error::Internal)
    }
    /// None 表示前序 CIDR 尚无地址，不允许跳过它匹配后面的规则。
    pub fn evaluate_before_dns(&self, target: Target) -> Result<Option<Decision>> {
        self.evaluate_inner(target, true)
    }
    fn evaluate_inner(&self, target: Target, defer_ip: bool) -> Result<Option<Decision>> {
        if target.port == 0 || (target.domain.is_none() && target.address.is_none()) {
            return Err(Error::InvalidInput);
        }
        let name = target.domain.as_deref().map(domain).transpose()?;
        let address = target
            .address
            .as_deref()
            .map(|s| {
                if s.len() > 64 {
                    return Err(Error::InvalidInput);
                }
                s.parse::<IpAddr>().map_err(|_| Error::InvalidInput)
            })
            .transpose()?;
        if self.mode != Mode::Rules {
            let policy = if self.mode == Mode::Direct {
                Policy::Direct
            } else {
                Policy::Proxy
            };
            return Ok(Some(self.decision(policy, None, Reason::Mode, None)));
        }
        for (index, rule) in self.rules.iter().enumerate() {
            if defer_ip && address.is_none() && matches!(&rule.matcher, Matcher::Network(_)) {
                return Ok(None);
            }
            let matched = match &rule.matcher {
                Matcher::Domain(value, suffix) => name.as_ref().is_some_and(|name| {
                    name == value
                        || (*suffix
                            && name
                                .strip_suffix(value)
                                .is_some_and(|prefix| prefix.ends_with('.')))
                }),
                Matcher::Network(network) => address.is_some_and(|ip| network.includes(ip)),
                Matcher::Port(first, last) => (*first..=*last).contains(&target.port),
                Matcher::Transport(value) => *value == target.transport,
            };
            if matched {
                return Ok(Some(self.decision(
                    rule.policy,
                    Some(index),
                    Reason::Rule,
                    rule.outbound.clone(),
                )));
            }
        }
        Ok(Some(self.decision(
            self.fallback,
            None,
            Reason::Fallback,
            None,
        )))
    }
    fn decision(
        &self,
        policy: Policy,
        index: Option<usize>,
        reason: Reason,
        outbound: Option<String>,
    ) -> Decision {
        Decision {
            policy,
            rule_index: index,
            reason,
            outbound: if policy == Policy::Proxy {
                outbound.or_else(|| self.outbound.clone())
            } else {
                None
            },
        }
    }
}
