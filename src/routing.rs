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
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, Hash)]
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
    /// 操作系统可执行文件名；精确、区分大小写，不接受路径或通配符。
    ProcessName,
    #[serde(rename = "DOMAIN-SUFFIX")]
    DomainSuffix,
    #[serde(rename = "DOMAIN-KEYWORD")]
    DomainKeyword,
    #[serde(rename = "IP-CIDR")]
    IpCidr,
    /// 已就绪 MMDB 的国家条件；与显式 CIDR 同属 IP 阶段，同来源显式 CIDR 优先。
    #[serde(rename = "GEOIP")]
    GeoIp,
    DestinationPort,
    /// 端口与基域名后缀的 AND 条件，内部规范值为 `端口,域名`。
    DomainPort,
    /// 域名后缀与实际 QUIC 报文的 AND 条件；不把 UDP/443 视为 QUIC。
    DomainQuic,
    Transport,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Rule {
    /// 域名与 TCP/UDP 的 AND 条件；仅 DOMAIN、DOMAIN-SUFFIX 可设置，缺省不限制协议。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protocol: Option<Transport>,
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
    GeoIp(String),
    ProcessName(String),
    Port(u16, u16),
    DomainPort(String, u16),
    DomainQuic(String),
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
/// 关键词只观察可注册域名的租户标签；PSL 包括 ICANN 与 PRIVATE 后缀。
/// 例如 a.foo.github.io -> foo、a.example.co.uk -> example；不观察子域名或后缀。
/// 返回借用切片，不下载列表、不分配内存；输入必须已经通过域名规范化。
pub(super) fn keyword_subject(name: &str) -> Option<&str> {
    let registered = psl::domain_str(name)?;
    registered.split_once('.').map(|(label, _)| label)
}
/// 节点名用于逻辑引用，不作为文件名；保持主配置的 Unicode、大小写及 128 字节约束。
pub(crate) fn outbound_name(value: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > 128
        || value != value.trim()
        || value
            .chars()
            .any(|c| c.is_control() || matches!(c, ',' | '/' | '\\'))
    {
        return Err(Error::InvalidInput);
    }
    Ok(())
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
    outbound_name(&result)?;
    Ok(Some(result))
}
// DNS 名称最大 253 字节；组合值额外包含 u16 端口的十进制字符和一个逗号。
const MAX_DOMAIN_BYTES: usize = 253;
/// 操作系统可执行文件名的 UTF-8 字节上限，不截断参与匹配。
pub(crate) const MAX_PROCESS_NAME_BYTES: usize = 255;
const MAX_PORT_DIGITS: usize = 5;
pub(super) const MAX_DOMAIN_PORT_BYTES: usize = MAX_DOMAIN_BYTES + MAX_PORT_DIGITS + 1;
fn matcher(kind: Match, value: &str) -> Result<Matcher> {
    if value.is_empty()
        || value.len()
            > if kind == Match::ProcessName {
                MAX_PROCESS_NAME_BYTES
            } else if kind == Match::DomainPort {
                MAX_DOMAIN_PORT_BYTES
            } else {
                MAX_DOMAIN_BYTES
            }
    {
        return Err(Error::InvalidInput);
    }
    Ok(match kind {
        Match::DomainQuic => Matcher::DomainQuic(domain(value)?),
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
        Match::ProcessName => {
            if value
                .chars()
                .any(|c| c.is_control() || matches!(c, '/' | '\\' | '*' | ','))
            {
                return Err(Error::InvalidInput);
            }
            Matcher::ProcessName(value.to_owned())
        }
        Match::GeoIp => {
            if value.len() != 2 || !value.bytes().all(|byte| byte.is_ascii_uppercase()) {
                return Err(Error::InvalidInput);
            }
            Matcher::GeoIp(value.to_owned())
        }
        Match::IpCidr => Matcher::Network(Network::parse(value)?),
        Match::Transport if value == "quic" => Matcher::Quic,
        Match::Transport => Matcher::Transport(match value {
            "tcp" => Transport::Tcp,
            "udp" => Transport::Udp,
            _ => return Err(Error::InvalidInput),
        }),
        Match::DomainPort => {
            let (port, host) = value.split_once(',').ok_or(Error::InvalidInput)?;
            if !port.bytes().all(|b| b.is_ascii_digit()) {
                return Err(Error::InvalidInput);
            }
            let port: u16 = port.parse().map_err(|_| Error::InvalidInput)?;
            if port == 0 {
                return Err(Error::InvalidInput);
            }
            Matcher::DomainPort(domain(host)?, port)
        }
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
        self.protection_identified(target, false)
    }
    pub(crate) fn protection_identified(
        &self,
        target: Target,
        quic: bool,
    ) -> Result<TrafficProtection> {
        self.protection_facts(target, quic, None)
    }
    pub(crate) fn protection_facts(
        &self,
        target: Target,
        quic: bool,
        process: Option<&str>,
    ) -> Result<TrafficProtection> {
        let first = self.runtime.evaluate_facts(&target, quic, process)?;
        let identified =
            self.runtime
                .evaluate_facts(&target, target.transport == Transport::Udp, process)?;
        let quic_changes_route =
            first.policy != identified.policy || first.outbound != identified.outbound;
        let rejected = self.debug.is_some() && first.policy == Policy::Reject;
        Ok(TrafficProtection {
            transport_reject: rejected || self.runtime.blocks_transport(target.transport),
            quic_reject: identified.policy == Policy::Reject,
            quic_changes_route,
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
    /// HTTP 复写只替换本次正式路由，保留 Debug 授权、到期和撤销检查。
    pub(crate) fn for_http_rewrite(
        &self,
        policy: Policy,
        outbound: Option<String>,
    ) -> Result<Self> {
        let mut router = Self::compile(Config {
            mode: Mode::Rules,
            fallback: policy,
            outbound,
            ..Config::default()
        })?;
        router.debug = self.debug.clone();
        Ok(router)
    }
    pub(crate) fn needs_process(&self) -> bool {
        self.runtime.needs_process()
    }
    pub(crate) fn evaluate_flow(
        &self,
        target: Target,
        bytes: Option<&[u8]>,
        process: Option<&str>,
    ) -> Result<Decision> {
        let quic = bytes
            .map(|bytes| runtime::Snapshot::identify_datagram(&target, bytes))
            .transpose()?
            .unwrap_or(false);
        if let Some(debug) = &self.debug
            && let Some(decision) = debug.evaluate(&target)?
        {
            return Ok(decision);
        }
        self.runtime.evaluate_facts(&target, quic, process)
    }
    pub(crate) fn before_dns_flow(
        &self,
        target: Target,
        bytes: Option<&[u8]>,
        process: Option<&str>,
    ) -> Result<Option<Decision>> {
        let quic = bytes
            .map(|bytes| runtime::Snapshot::identify_datagram(&target, bytes))
            .transpose()?
            .unwrap_or(false);
        if let Some(debug) = &self.debug {
            match debug.before_dns(&target)? {
                DebugPreDecision::Decided(decision) => return Ok(Some(decision)),
                DebugPreDecision::WaitForDns => return Ok(None),
                DebugPreDecision::Continue => (),
            }
        }
        self.runtime.before_dns_facts(&target, quic, process)
    }
    /// 纯报文决策入口，Debug 仍先于全部正式规则；不创建或扩大 Debug 授权。
    pub fn evaluate_datagram(&self, target: Target, bytes: &[u8]) -> Result<Decision> {
        runtime::Snapshot::identify_datagram(&target, bytes)?;
        if let Some(debug) = &self.debug
            && let Some(decision) = debug.evaluate(&target)?
        {
            return Ok(decision);
        }
        self.runtime.evaluate_datagram(&target, bytes)
    }
    pub(crate) fn needs_datagram(&self, target: &Target) -> Result<bool> {
        // 已命中的 Debug 决策仍最高；报文写入时保留授权/到期/协议安全检查。
        if let Some(debug) = &self.debug
            && debug.evaluate(target)?.is_some()
        {
            return Ok(false);
        }
        self.runtime.needs_datagram(target)
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
        snapshot.require_ready_geoip()?;
        Ok(Self {
            runtime: std::sync::Arc::new(snapshot),
            debug: None,
        })
    }
    pub fn from_runtime(snapshot: std::sync::Arc<runtime::Snapshot>) -> Result<Self> {
        snapshot.require_ready_geoip()?;
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
    // 已发送数据后识别成另一出站策略时拒绝继续，禁止跨节点重放或静默改用原出站。
    quic_changes_route: bool,
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
        } else if quic && self.quic_changes_route {
            Err(Error::InvalidState)
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
#[path = "../tests/test_routing.rs"]
mod test_routing;
