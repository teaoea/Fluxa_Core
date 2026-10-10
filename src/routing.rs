//! 纯路由决策：编译一次、按来源和类型优先级索引匹配，不解析 DNS、不建连接、不隐式切换出站。
use crate::{
    error::{Error, Result},
    network::Network,
};
use serde::{Deserialize, Serialize};
use std::net::IpAddr;
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
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "kebab-case")]
pub enum Match {
    Domain,
    DomainSuffix,
    #[serde(alias = "key")]
    DomainKeyword,
    IpCidr,
    DestinationPort,
    Transport,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Rule {
    pub r#type: Match,
    pub value: String,
    pub policy: Policy,
    #[serde(default)]
    pub outbound: Option<String>,
}
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default)]
    pub runtime_handle: Option<u64>,
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
    #[serde(default)]
    pub rule_version: Option<u64>,
    #[serde(default)]
    pub origin: Option<runtime::Origin>,
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
    Keyword(String),
    Network(Network),
    Port(u16, u16),
    Transport(Transport),
}
pub struct Router {
    runtime: std::sync::Arc<runtime::Snapshot>,
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
        Match::DomainKeyword => {
            if !value
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b".-".contains(&b))
            {
                return Err(Error::InvalidInput);
            }
            Matcher::Keyword(value.to_ascii_lowercase())
        }
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
    /// 单来源配置也使用统一新语义，不保留可绕过拦截的旧匹配路径。
    pub fn compile(mut config: Config) -> Result<Self> {
        if let Some(handle) = config.runtime_handle {
            if !config.rules.is_empty() {
                return Err(Error::InvalidInput);
            }
            let snapshot = runtime::lookup(handle)?;
            let settings = &snapshot.document().settings;
            if settings.mode != config.mode
                || settings.fallback != config.fallback
                || settings.outbound != config.outbound
            {
                return Err(Error::InvalidInput);
            }
            return Self::from_runtime(snapshot);
        }
        let rules = std::mem::take(&mut config.rules);
        let snapshot = runtime::Snapshot::generate(
            config,
            vec![runtime::Source {
                id: "local".into(),
                layer: runtime::Layer::Local,
                enabled: true,
                rules,
            }],
            1,
        )?;
        Ok(Self {
            runtime: std::sync::Arc::new(snapshot),
        })
    }
    pub fn from_runtime(snapshot: std::sync::Arc<runtime::Snapshot>) -> Result<Self> {
        runtime::register(&snapshot)?;
        Ok(Self { runtime: snapshot })
    }
    pub fn evaluate(&self, target: Target) -> Result<Decision> {
        self.runtime.evaluate(&target)
    }
    /// 域名命中直接决定动作，未命中才进入真实 IP 规则阶段。
    pub fn evaluate_before_dns(&self, target: Target) -> Result<Option<Decision>> {
        self.runtime.evaluate_before_dns(&target)
    }
}

/// 运行时规则生成及来源感知索引；所有宿主复用此模块。
pub mod runtime;

/// 原子文件提交和不可变规则快照生命周期。
pub mod store;

/// 有界后台生成任务；提交期间连接继续使用原子快照。
pub mod background;
