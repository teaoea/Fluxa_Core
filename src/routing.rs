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
    #[serde(rename = "DOMAIN")]
    Domain,
    #[serde(rename = "DOMAIN-SUFFIX")]
    DomainSuffix,
    #[serde(rename = "DOMAIN-KEYWORD")]
    DomainKeyword,
    #[serde(rename = "IP-CIDR")]
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
    #[serde(rename = "debug-rule")]
    DebugRule,
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
    Quic,
}
#[derive(Clone)]
pub struct Router {
    runtime: std::sync::Arc<runtime::Snapshot>,
    debug: Option<debug::View>,
}
pub(super) enum DebugPreDecision {
    Continue,
    WaitForDns,
    Decided(Decision),
}
pub(crate) fn identifier(value: &str) -> Result<()> {
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
        Match::Transport if value == "quic" => Matcher::Quic,
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
    pub fn check_transport(&self, transport: Transport) -> Result<()> {
        if self.runtime.blocks_transport(transport) {
            Err(Error::Rejected)
        } else {
            Ok(())
        }
    }
    /// 只对当前目标核验临时例外；IP 调试可能等待 DNS，不能提前执行正式拦截。
    pub(crate) fn check_before_dns_transport(&self, target: &Target) -> Result<()> {
        if let Some(debug) = &self.debug {
            match debug.before_dns(target)? {
                DebugPreDecision::Decided(decision) => {
                    return if decision.policy == Policy::Reject {
                        Err(Error::Rejected)
                    } else {
                        Ok(())
                    };
                }
                DebugPreDecision::WaitForDns => return Ok(()),
                DebugPreDecision::Continue => (),
            }
        }
        self.check_transport(target.transport)
    }
    pub(crate) fn protection(&self, target: Target) -> Result<TrafficProtection> {
        let rejected =
            self.debug.is_some() && self.runtime.evaluate(&target)?.policy == Policy::Reject;
        Ok(TrafficProtection {
            transport_reject: rejected || self.runtime.blocks_transport(target.transport),
            quic_reject: self.runtime.blocks_quic(),
            grant: match &self.debug {
                Some(view) => view.evaluate(&target)?.map(|decision| DebugProtection {
                    view: view.clone(),
                    target,
                    policy: decision.policy,
                    outbound: decision.outbound,
                }),
                None => None,
            },
        })
    }
    pub fn blocks_quic(&self) -> bool {
        self.runtime.blocks_quic()
    }

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
            debug: None,
        })
    }
    pub fn from_runtime(snapshot: std::sync::Arc<runtime::Snapshot>) -> Result<Self> {
        runtime::register(&snapshot)?;
        Ok(Self {
            runtime: snapshot,
            debug: None,
        })
    }
    /// 只通过显式 API 为后续决策绑定当前调试视图；不存在配置字段或隐式入口。
    pub(crate) fn with_debug(mut self, session: &debug::Session) -> Result<Self> {
        self.debug = Some(session.view()?);
        Ok(self)
    }
    pub fn evaluate(&self, target: Target) -> Result<Decision> {
        if let Some(debug) = &self.debug
            && let Some(decision) = debug.evaluate(&target)?
        {
            return Ok(decision);
        }
        self.runtime.evaluate(&target)
    }
    /// 域名命中直接决定动作，未命中才进入真实 IP 规则阶段。
    pub fn evaluate_before_dns(&self, target: Target) -> Result<Option<Decision>> {
        if let Some(debug) = &self.debug {
            match debug.before_dns(&target)? {
                DebugPreDecision::Decided(decision) => return Ok(Some(decision)),
                DebugPreDecision::WaitForDns => return Ok(None),
                DebugPreDecision::Continue => (),
            }
        }
        self.runtime.evaluate_before_dns(&target)
    }
}
/// API 专用本地临时调试会话，不从 YAML 创建或持久化。
pub mod debug;

/// 运行时规则生成及来源感知索引；所有宿主复用此模块。
pub mod runtime;

/// 原子文件提交和不可变规则快照生命周期。
pub mod store;

/// 有界后台生成任务；提交期间连接继续使用原子快照。
pub mod background;

#[path = "routing/cache.rs"]
mod cache;

/// 正式保护与限时例外分离；不因调试建连而永久撤销 QUIC/传输安全检查。
#[derive(Default)]
pub(crate) struct TrafficProtection {
    transport_reject: bool,
    quic_reject: bool,
    grant: Option<DebugProtection>,
}
struct DebugProtection {
    view: debug::View,
    target: Target,
    policy: Policy,
    outbound: Option<String>,
}
impl TrafficProtection {
    pub(crate) fn has_debug_grant(&self) -> bool {
        self.grant.is_some()
    }
    pub(crate) fn revoked_debug(&self) -> bool {
        self.grant
            .as_ref()
            .is_some_and(|grant| grant.view.revoked())
    }
    pub(crate) async fn invalidated(&self) -> Result<()> {
        let Some(grant) = &self.grant else {
            return std::future::pending().await;
        };
        loop {
            self.check(false)?;
            grant.view.changed().await?;
        }
    }

    pub(crate) fn check(&self, quic: bool) -> Result<()> {
        if let Some(grant) = &self.grant {
            match grant.view.evaluate(&grant.target)? {
                Some(decision) if decision.policy == Policy::Reject => return Err(Error::Rejected),
                Some(decision)
                    if decision.policy == grant.policy && decision.outbound == grant.outbound =>
                {
                    return Ok(());
                }
                _ => {
                    return Err(if self.transport_reject || (quic && self.quic_reject) {
                        Error::Rejected
                    } else {
                        Error::Cancelled
                    });
                }
            }
        }
        if self.transport_reject || (quic && self.quic_reject) {
            Err(Error::Rejected)
        } else {
            Ok(())
        }
    }
}
