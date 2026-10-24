//! 来源感知运行时规则：只修改生成产物，原始配置与订阅始终由宿主保留。
use super::{
    Config, Decision, Match, Matcher, Mode, Policy, Reason, Rule, Target, domain, identifier,
    matcher, policy_outbound,
};
use crate::{
    budget::{ByteBudget, ByteLease},
    error::{Error, Result},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    net::IpAddr,
};
#[path = "index.rs"]
mod index;
use index::{Candidates, Index};
#[path = "flat.rs"]
mod flat;
use flat::FlatIndex;
#[path = "yaml.rs"]
pub(crate) mod yaml;

/// 仅有界解析入口可构造，避免直接 Deserialize 绕过驻留预算。
pub struct GenerationInput {
    pub format_version: u32,
    pub settings: Config,
    pub sources: Vec<SourceInput>,
    _memory: Option<ByteLease>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawGenerationInput {
    format_version: u32,
    settings: Config,
    sources: Vec<SourceInput>,
}
impl GenerationInput {
    /// 后台任务接续拥有解析配额，直到来源真正消费完成；不暴露给公开调用方。
    pub(crate) fn take_memory(&mut self) -> Option<ByteLease> {
        self._memory.take()
    }

    pub fn from_yaml(input: &str) -> Result<Self> {
        if input.len() > yaml::MAX_BYTES {
            return Err(Error::ResourceLimit);
        }
        let _temporary = reserve_parse(input, yaml::MAX_BYTES)?;
        let raw: RawGenerationInput = yaml::decode(input)?;
        let mut value = Self {
            format_version: raw.format_version,
            settings: raw.settings,
            sources: raw.sources,
            _memory: None,
        };
        if value.format_version != 1 {
            return Err(Error::ConfigUnsupportedVersion(value.format_version));
        }
        if value.sources.len() > MAX_SOURCES {
            return Err(Error::InvalidInput);
        }
        validate_settings(&value.settings)?;
        if value.sources.iter().try_fold(0usize, |total, source| {
            total
                .checked_add(source.input_bytes()?)
                .ok_or(Error::ResourceLimit)
        })? > MAX_INPUT_MEMORY
        {
            return Err(Error::ResourceLimit);
        }
        // 输入对象返回宿主后仍占内存，预算随对象持有，不在解析成功时提前归还。
        value._memory = Some(_temporary);
        Ok(value)
    }
}
/// 规则子系统共享预算的只读观测，不提供重置或改变资源配额的外部入口。
#[derive(Clone, Copy, Debug, Serialize)]
pub struct MemoryUsage {
    pub reserved_bytes: usize,
    pub peak_reserved_bytes: usize,
    pub limit_bytes: usize,
}
pub fn memory_usage() -> MemoryUsage {
    let (reserved_bytes, peak_reserved_bytes) = MEMORY.usage();
    MemoryUsage {
        reserved_bytes,
        peak_reserved_bytes,
        limit_bytes: MAX_MEMORY,
    }
}
pub const MAX_RULES: usize = 100_000;
pub const MAX_SOURCES: usize = 64;
// 全局预算同时约束存量快照、新快照与编译临时对象。每次生成先预留最坏估算，
// 不把数量上限当作内存上限；高标签数输入可能在达到规则数量前被拒绝。
const MAX_MEMORY: usize = 512 * 1024 * 1024;
const MAX_INPUT_MEMORY: usize = 128 * 1024 * 1024;
pub(crate) const MAX_PAYLOAD_BYTES: usize = 8 * 1024 * 1024;
const PARSE_FACTOR: usize = 8;
const PARSE_BASE: usize = 4096;
const SNAPSHOT_BASE: usize = 6 * 1024 * 1024;
const RULE_BYTES: usize = 1536;
const ORIGIN_BYTES: usize = 128;
// Vec 增长可保留多于逻辑元素数的容量；恢复准入也必须覆盖这个余量。
const ORIGIN_CAPACITY_FACTOR: usize = 2;
const RESIDENT_RULE_BYTES: usize = 512;
const RESIDENT_STRING_FACTOR: usize = 2;
// 2026-10-12 唯一多标签后缀压力测量显示旧系数低估构建峰值，
// 2026-10-15 并发旧快照 + 35 层后缀实测 768 系数低估 RSS；提高为 1024，
// 包括扩充候选后的哈希节点、Vec 容量、临时索引与分配器余量。
const LABEL_BYTES: usize = 1024;
const KEYWORD_FACTOR: usize = 128;
const STRING_COPY_FACTOR: usize = 4;
const MAX_HANDLES: usize = 128;
pub(super) static MEMORY: ByteBudget = ByteBudget::new(MAX_MEMORY);

/// 所有解析入口先做字节检查和预算准入；配额由 RAII 在失败及返回时归还。
pub(crate) fn reserve_parse(input: &str, limit: usize) -> Result<ByteLease> {
    if input.len() > limit {
        return Err(Error::ResourceLimit);
    }
    MEMORY.reserve(scaled(input.len(), PARSE_FACTOR, PARSE_BASE)?)
}
fn scaled(count: usize, factor: usize, base: usize) -> Result<usize> {
    count
        .checked_mul(factor)
        .and_then(|v| v.checked_add(base))
        .ok_or(Error::ResourceLimit)
}

/// 无原始来源的生成文档只能验证结构，不能证明被删规则的覆盖或冲突理由。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ReportVerification {
    VerifiedFromSources,
    StructuralOnly,
}
/// 来源身份，不使用枚举编号排序；执行顺序由独立的 SourcePriority 明确规定。
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum Layer {
    Local,
    Plugin,
    Remote,
}
// 此枚举专门表达业务来源顺序，不是 RuleClass 索引类别编号。
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum SourcePriority {
    Local,
    Plugin,
    Remote,
}
impl Layer {
    const ORDER: [Self; 3] = [Self::Local, Self::Plugin, Self::Remote];
    fn priority(self) -> SourcePriority {
        match self {
            Self::Local => SourcePriority::Local,
            Self::Plugin => SourcePriority::Plugin,
            Self::Remote => SourcePriority::Remote,
        }
    }
}
// 类型执行优先级独立于索引槽位，不能按 RuleClass 数字大小选择。
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum TypePriority {
    Process,
    Protocol,
    Port,
    Domain,
    Suffix,
    Keyword,
    Ip,
    GeoIp,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub id: String,
    pub layer: Layer,
    #[serde(default = "enabled")]
    pub enabled: bool,
    pub rules: Vec<Rule>,
}
/// 原始 Clash payload 在工作线程中解析，Swift/CLI 只传递来源及订阅动作。
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum SourceInput {
    Rules(Source),
    Clash(ClashSource),
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ClashSource {
    pub id: String,
    pub layer: Layer,
    #[serde(default = "enabled")]
    pub enabled: bool,
    pub payload_yaml: String,
    pub policy: Policy,
    pub outbound: Option<String>,
}
impl SourceInput {
    pub(crate) fn input_bytes(&self) -> Result<usize> {
        match self {
            Self::Rules(source) => source_input_charge(source),
            Self::Clash(source) => {
                identifier(&source.id)?;
                if let Some(id) = &source.outbound {
                    super::strategy::parse(id)?;
                    if source.policy != Policy::Proxy {
                        return Err(Error::InvalidInput);
                    }
                }
                if source.payload_yaml.len() > MAX_PAYLOAD_BYTES {
                    return Err(Error::ResourceLimit);
                }
                scaled(
                    source
                        .payload_yaml
                        .capacity()
                        .checked_add(source.id.capacity())
                        .and_then(|n| {
                            n.checked_add(source.outbound.as_ref().map_or(0, String::capacity))
                        })
                        .ok_or(Error::ResourceLimit)?,
                    PARSE_FACTOR,
                    PARSE_BASE,
                )
            }
        }
    }
    pub(crate) fn into_source(self) -> Result<Source> {
        match self {
            Self::Rules(source) => Ok(source),
            Self::Clash(source) => Ok(Source {
                id: source.id,
                layer: source.layer,
                enabled: source.enabled,
                rules: crate::subscription::parse_rules(
                    &source.payload_yaml,
                    source.policy,
                    source.outbound,
                )?,
            }),
        }
    }
}
fn enabled() -> bool {
    true
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Origin {
    pub source: u16,
    pub sequence: u32,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct EffectiveRule {
    pub origin: Origin,
    pub additional_origins: Vec<Origin>,
    pub rule: Rule,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SourceIdentity {
    pub id: String,
    pub layer: Layer,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum RemovalReason {
    Duplicate,
    Conflict,
    Covered,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Removal {
    pub origin: Origin,
    pub reason: RemovalReason,
    pub covered_by: Origin,
}
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Report {
    pub original: usize,
    pub duplicates: usize,
    pub conflicts: usize,
    pub pruned: usize,
    /// 编译后保留条目数，不是每条都可达的证明；拦截覆盖裁剪保持保守。
    pub effective: usize,
    pub removals: Vec<Removal>,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
/// 内部生成产物，不包含被删除规则原文；加载后报告默认仅结构验证。
pub struct Document {
    pub format_version: u32,
    pub version: u64,
    pub settings: Config,
    pub sources: Vec<SourceIdentity>,
    pub rules: Vec<EffectiveRule>,
    pub report: Report,
}
/// 仅提供阶段及来源序号，不包含域名、出站正文、凭据或配置内容。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValidationDiagnostic {
    pub error: Error,
    pub stage: &'static str,
    pub source: Option<u16>,
    pub sequence: Option<u32>,
}
impl ValidationDiagnostic {
    fn new(stage: &'static str) -> Self {
        Self {
            error: Error::InvalidInput,
            stage,
            source: None,
            sequence: None,
        }
    }
}
/// 名称只描述索引存储槽位，不能用枚举数值代替规则合并裁剪，网络流量规则执行顺序。
#[repr(usize)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RuleClass {
    Reject = 0,
    LocalIp = 1,
    LocalDomain = 2,
    LocalDomainSuffix = 3,
    LocalPortTransport = 4,
    PluginRemoteIp = 5,
    PluginRemoteDomain = 6,
    PluginRemoteDomainSuffix = 7,
    PluginRemotePortTransport = 8,
    LocalKeyword = 9,
    PluginRemoteKeyword = 10,
    // 独立槽位避免不匹配协议遮蔽同域其他候选；编号仍不表示优先级。
    RejectTcp = 11,
    RejectUdp = 12,
    LocalDomainTcp = 13,
    PluginRemoteDomainTcp = 14,
    LocalDomainUdp = 15,
    PluginRemoteDomainUdp = 16,
    LocalSuffixTcp = 17,
    PluginRemoteSuffixTcp = 18,
    LocalSuffixUdp = 19,
    PluginRemoteSuffixUdp = 20,
}
impl RuleClass {
    pub(super) const COUNT: usize = Self::PluginRemoteSuffixUdp as usize + 1;
    fn local_equivalent(self) -> Self {
        match self {
            Self::PluginRemoteIp => Self::LocalIp,
            Self::PluginRemoteDomain => Self::LocalDomain,
            Self::PluginRemoteDomainSuffix => Self::LocalDomainSuffix,
            Self::PluginRemotePortTransport => Self::LocalPortTransport,
            Self::PluginRemoteKeyword => Self::LocalKeyword,
            Self::PluginRemoteDomainTcp => Self::LocalDomainTcp,
            Self::PluginRemoteDomainUdp => Self::LocalDomainUdp,
            Self::PluginRemoteSuffixTcp => Self::LocalSuffixTcp,
            Self::PluginRemoteSuffixUdp => Self::LocalSuffixUdp,
            other => other,
        }
    }
}
pub(super) struct Entry {
    pub(super) matcher: Matcher,
    protocol: Option<super::Transport>,
    layer: Layer,
    policy: Policy,
}
impl Entry {
    // 非本地槽位由 Plugin/Remote 共享，编译先插件后远程，故同类型首次命中不会丢掉更高来源。
    // 实际裁决读取每条规则的真实 Layer；槽位名及编号绝不等同来源或优先级。
    pub(super) fn class(&self) -> RuleClass {
        // QUIC 限定后缀复用 UDP 协议后缀槽：只有已识别时才查询独立森林根。
        // 同来源的 UDP/QUIC 协议条件按原始顺序裁决，避免给每个索引节点增加无用槽位。
        if matches!(self.matcher, Matcher::DomainQuic(_)) {
            return if self.policy == Policy::Reject {
                RuleClass::RejectUdp
            } else if self.layer == Layer::Local {
                RuleClass::LocalSuffixUdp
            } else {
                RuleClass::PluginRemoteSuffixUdp
            };
        }
        if self.special_reject() {
            return match self.protocol {
                None => RuleClass::Reject,
                Some(super::Transport::Tcp) => RuleClass::RejectTcp,
                Some(super::Transport::Udp) => RuleClass::RejectUdp,
            };
        }
        if let Some(protocol) = self.protocol {
            return match (&self.matcher, self.layer, protocol) {
                (Matcher::Domain(_, false), Layer::Local, super::Transport::Tcp) => {
                    RuleClass::LocalDomainTcp
                }
                (
                    Matcher::Domain(_, false),
                    Layer::Plugin | Layer::Remote,
                    super::Transport::Tcp,
                ) => RuleClass::PluginRemoteDomainTcp,
                (Matcher::Domain(_, false), Layer::Local, super::Transport::Udp) => {
                    RuleClass::LocalDomainUdp
                }
                (
                    Matcher::Domain(_, false),
                    Layer::Plugin | Layer::Remote,
                    super::Transport::Udp,
                ) => RuleClass::PluginRemoteDomainUdp,
                (Matcher::Domain(_, true), Layer::Local, super::Transport::Tcp) => {
                    RuleClass::LocalSuffixTcp
                }
                (
                    Matcher::Domain(_, true),
                    Layer::Plugin | Layer::Remote,
                    super::Transport::Tcp,
                ) => RuleClass::PluginRemoteSuffixTcp,
                (Matcher::Domain(_, true), Layer::Local, super::Transport::Udp) => {
                    RuleClass::LocalSuffixUdp
                }
                (
                    Matcher::Domain(_, true),
                    Layer::Plugin | Layer::Remote,
                    super::Transport::Udp,
                ) => RuleClass::PluginRemoteSuffixUdp,
                _ => unreachable!("组合规则已完成域名类型校验"),
            };
        }
        match (&self.matcher, self.layer) {
            (Matcher::Network(_) | Matcher::GeoIp(_), Layer::Local) => RuleClass::LocalIp,
            (Matcher::Network(_) | Matcher::GeoIp(_), Layer::Plugin | Layer::Remote) => {
                RuleClass::PluginRemoteIp
            }
            (Matcher::Domain(_, false), Layer::Local) => RuleClass::LocalDomain,
            (Matcher::Domain(_, false), Layer::Plugin | Layer::Remote) => {
                RuleClass::PluginRemoteDomain
            }
            (Matcher::Domain(_, true), Layer::Local) => RuleClass::LocalDomainSuffix,
            (Matcher::Domain(_, true), Layer::Plugin | Layer::Remote) => {
                RuleClass::PluginRemoteDomainSuffix
            }
            (Matcher::Keyword(_), Layer::Local) => RuleClass::LocalKeyword,
            (Matcher::Keyword(_), Layer::Plugin | Layer::Remote) => RuleClass::PluginRemoteKeyword,
            (_, Layer::Local) => RuleClass::LocalPortTransport,
            (_, Layer::Plugin | Layer::Remote) => RuleClass::PluginRemotePortTransport,
        }
    }
    fn special_reject(&self) -> bool {
        self.policy == Policy::Reject
            && (self.protocol.is_some()
                || matches!(
                    self.matcher,
                    Matcher::Domain(_, false)
                        | Matcher::Network(_)
                        | Matcher::GeoIp(_)
                        | Matcher::Port(..)
                        | Matcher::DomainPort(..)
                        | Matcher::DomainQuic(_)
                        | Matcher::Transport(_)
                        | Matcher::Quic
                ))
    }
}
/// 只读执行索引与资源所有者；报告信任程度不参与策略匹配。
// QUIC 属于 UDP 数据面识别，不扩充正式候选槽位；编译和缓存恢复共用此准入判断。
fn quic_guard(entries: &[Entry]) -> Option<usize> {
    entries
        .iter()
        .position(|entry| matches!(entry.matcher, Matcher::Quic) && entry.policy == Policy::Reject)
}
pub struct Snapshot {
    instance: u64,
    document: Document,
    entries: Vec<Entry>,
    index: Index,
    flat: Option<FlatIndex>,
    _flat_memory: Option<ByteLease>,
    _memory: ByteLease,
    quic_blocked: bool,
    quic_rule: Option<usize>,
    has_geoip: bool,
    has_process: bool,
    geoip: Option<std::sync::Arc<crate::geoip::Database>>,
    report_verification: ReportVerification,
}
pub(crate) fn normalize_condition(rule: &Rule) -> Result<String> {
    let value = match matcher(rule.r#type, &rule.value)? {
        Matcher::Domain(value, _)
        | Matcher::Keyword(value)
        | Matcher::DomainQuic(value)
        | Matcher::GeoIp(value)
        | Matcher::ProcessName(value) => value,
        Matcher::DomainPort(value, port) => format!("{port},{value}"),
        Matcher::Network(network) => network.canonical(),
        Matcher::Port(first, last) => {
            if first == last {
                first.to_string()
            } else {
                format!("{first}-{last}")
            }
        }
        Matcher::Transport(_) | Matcher::Quic => rule.value.clone(),
    };
    Ok(value)
}
fn validate_rule_action(rule: &Rule) -> Result<()> {
    if rule.protocol.is_some() && !matches!(rule.r#type, Match::Domain | Match::DomainSuffix) {
        return Err(Error::InvalidInput);
    }
    // 即使来源暂时禁用，全域传输直连/代理仍是非法规则，不能留到启用时才发现。
    if rule.r#type == Match::Transport && rule.policy != Policy::Reject {
        return Err(Error::InvalidInput);
    }
    Ok(())
}
pub(crate) fn normalize_rule(rule: &Rule, default: &Option<String>) -> Result<Rule> {
    validate_rule_action(rule)?;
    let value = normalize_condition(rule)?;
    Ok(Rule {
        protocol: rule.protocol,
        r#type: rule.r#type,
        value,
        policy: rule.policy,
        outbound: policy_outbound(rule.policy, rule.outbound.clone(), default)?,
    })
}
fn key(rule: &Rule) -> (Match, String) {
    (rule.r#type, rule.value.clone())
}
// 按输入扫描估算，不为估算复制规则；估算是准入策略，不是分配器硬上限。
fn rule_charge(rule: &Rule) -> Result<usize> {
    validate_rule_action(rule)?;
    if rule.value.len()
        > if rule.r#type == Match::DomainPort {
            super::MAX_DOMAIN_PORT_BYTES
        } else if rule.r#type == Match::ProcessName {
            crate::routing::MAX_PROCESS_NAME_BYTES
        } else {
            253
        }
        || rule
            .outbound
            .as_ref()
            .is_some_and(|id| id.len() > super::strategy::MAX_BYTES)
    {
        return Err(Error::InvalidInput);
    }
    let labels = if matches!(
        rule.r#type,
        Match::DomainSuffix | Match::DomainPort | Match::DomainQuic
    ) {
        rule.value.split('.').count()
    } else {
        0
    };
    let factor = if rule.r#type == Match::DomainKeyword {
        KEYWORD_FACTOR
    } else {
        STRING_COPY_FACTOR
    };
    scaled(rule.value.len(), factor, RULE_BYTES)?
        .checked_add(
            labels
                .checked_mul(LABEL_BYTES)
                .ok_or(Error::ResourceLimit)?,
        )
        .ok_or(Error::ResourceLimit)
}
// 队列只预留它实际拥有的输入对象，不把最终编译估算当作输入堆大小重复预留。
pub(super) fn source_input_charge(source: &Source) -> Result<usize> {
    identifier(&source.id)?;
    if source.rules.len() > MAX_RULES {
        return Err(Error::ResourceLimit);
    }
    let bytes = source
        .rules
        .capacity()
        .checked_mul(std::mem::size_of::<Rule>())
        .and_then(|n| n.checked_mul(STRING_COPY_FACTOR))
        .and_then(|n| n.checked_add(source.id.capacity().checked_mul(STRING_COPY_FACTOR)?))
        .and_then(|n| n.checked_add(PARSE_BASE))
        .ok_or(Error::ResourceLimit)?;
    source.rules.iter().try_fold(bytes, |sum, rule| {
        validate_rule_action(rule)?;
        if rule.value.len()
            > if rule.r#type == Match::DomainPort {
                super::MAX_DOMAIN_PORT_BYTES
            } else {
                253
            }
            || rule
                .outbound
                .as_ref()
                .is_some_and(|id| id.len() > super::strategy::MAX_BYTES)
        {
            return Err(Error::InvalidInput);
        }
        let strings = rule
            .value
            .capacity()
            .checked_add(rule.outbound.as_ref().map_or(0, String::capacity))
            .ok_or(Error::ResourceLimit)?;
        scaled(strings, STRING_COPY_FACTOR, sum)
    })
}
fn charge(sources: &[Source]) -> Result<usize> {
    let mut count = 0usize;
    let mut bytes = SNAPSHOT_BASE;
    for source in sources {
        identifier(&source.id)?;
        count = count
            .checked_add(source.rules.len())
            .ok_or(Error::ResourceLimit)?;
        if count > MAX_RULES {
            return Err(Error::ResourceLimit);
        }
        for rule in &source.rules {
            bytes = bytes
                .checked_add(rule_charge(rule)?)
                .ok_or(Error::ResourceLimit)?;
        }
    }
    Ok(bytes)
}
/// 文档及中间哈希集合、来源引用和最终索引在额外分配前统一准入。
fn document_charge(document: &Document) -> Result<usize> {
    let mut bytes = SNAPSHOT_BASE;
    let mut origins = document.rules.len();
    for source in &document.sources {
        identifier(&source.id)?;
    }
    for row in &document.rules {
        origins = origins
            .checked_add(row.additional_origins.len())
            .ok_or(Error::ResourceLimit)?;
        if origins > MAX_RULES {
            return Err(Error::ResourceLimit);
        }
        bytes = bytes
            .checked_add(rule_charge(&row.rule)?)
            .ok_or(Error::ResourceLimit)?;
    }
    origins = origins
        .checked_add(document.report.removals.len())
        .ok_or(Error::ResourceLimit)?;
    // 重复来源既在 additional_origins 也在 removals 中，因此最多两倍原始数量。
    if origins > MAX_RULES * 2 {
        return Err(Error::ResourceLimit);
    }
    scaled(
        origins,
        ORIGIN_BYTES
            .checked_mul(ORIGIN_CAPACITY_FACTOR)
            .ok_or(Error::ResourceLimit)?,
        bytes,
    )
}
/// 调试规则只含有限条目，不建立全套订阅索引；同样计入全局资源预算。
pub(super) fn reserve_debug<'a>(mut rules: impl Iterator<Item = &'a Rule>) -> Result<ByteLease> {
    let bytes = rules.try_fold(PARSE_BASE, |bytes, rule| {
        bytes
            .checked_add(rule_charge(rule)?)
            .ok_or(Error::ResourceLimit)
    })?;
    MEMORY.reserve(bytes)
}
fn validate_settings(settings: &Config) -> Result<()> {
    if !settings.rules.is_empty() || settings.runtime_handle.is_some() {
        return Err(Error::InvalidInput);
    }
    if let Some(id) = &settings.outbound {
        super::strategy::parse(id)?;
    }
    if settings.fallback == Policy::Proxy && settings.outbound.is_none() {
        return Err(Error::InvalidInput);
    }
    Ok(())
}
impl Snapshot {
    pub fn generate(settings: Config, sources: Vec<Source>, version: u64) -> Result<Self> {
        validate_settings(&settings)?;
        if sources.len() > MAX_SOURCES || version == 0 {
            return Err(Error::InvalidInput);
        }
        let memory = MEMORY.reserve(charge(&sources)?)?;
        let mut ids = HashSet::new();
        for source in &sources {
            identifier(&source.id)?;
            if !ids.insert(&source.id) {
                return Err(Error::InvalidInput);
            }
        }
        let mut document = Document {
            format_version: 1,
            version,
            settings,
            sources: sources
                .iter()
                .map(|s| SourceIdentity {
                    id: s.id.clone(),
                    layer: s.layer,
                })
                .collect(),
            rules: Vec::new(),
            report: Report::default(),
        };
        let mut entries = Vec::new();
        let mut exact =
            HashMap::<(Match, String, Option<super::Transport>, u8, Option<String>), usize>::new();
        let mut conditions =
            HashMap::<(Layer, Match, String, Option<super::Transport>), usize>::new();
        let mut coverage = Index::default();
        // 来源先按本地、插件、远程编译；同层级保留输入来源顺序及来源内规则序号。
        for layer in Layer::ORDER {
            for (source_id, source) in sources
                .iter()
                .enumerate()
                .filter(|(_, s)| s.layer == layer && s.enabled)
            {
                for (sequence, input) in source.rules.iter().enumerate() {
                    document.report.original += 1;
                    let origin = Origin {
                        source: source_id as u16,
                        sequence: sequence as u32,
                    };
                    let rule = normalize_rule(input, &document.settings.outbound)?;
                    let (kind, value) = key(&rule);
                    let action = match rule.policy {
                        Policy::Direct => 0,
                        Policy::Reject => 1,
                        Policy::Proxy => 2,
                    };
                    let full_key = (
                        kind,
                        value.clone(),
                        rule.protocol,
                        action,
                        rule.outbound.clone(),
                    );
                    if let Some(&existing) = exact.get(&full_key) {
                        document.rules[existing]
                            .additional_origins
                            .push(origin.clone());
                        document.report.duplicates += 1;
                        document.report.removals.push(Removal {
                            origin,
                            reason: RemovalReason::Duplicate,
                            covered_by: document.rules[existing].origin.clone(),
                        });
                        continue;
                    }
                    let entry = Entry {
                        matcher: matcher(kind, &value)?,
                        protocol: rule.protocol,
                        layer,
                        policy: rule.policy,
                    };
                    if rule.policy != Policy::Reject {
                        let level = layer;
                        let condition = (level, kind, value.clone(), rule.protocol);
                        let same = conditions.get(&condition).copied();
                        let covered = coverage.coverage(&entry.matcher);
                        let winner = same.or_else(|| {
                            // 只裁剪同类包含关系；绝不由 DNS 的偶然结果裁剪跨 IP/域名规则。
                            let class = entry.class();
                            covered.get(class as usize).or_else(|| {
                                if layer != Layer::Local {
                                    covered.get(class.local_equivalent() as usize)
                                } else {
                                    None
                                }
                            })
                        });
                        if let Some(existing) = winner {
                            let reason = if same.is_some() {
                                document.report.conflicts += 1;
                                RemovalReason::Conflict
                            } else {
                                document.report.pruned += 1;
                                RemovalReason::Covered
                            };
                            document.report.removals.push(Removal {
                                origin,
                                reason,
                                covered_by: document.rules[existing].origin.clone(),
                            });
                            continue;
                        }
                        conditions.insert(condition, entries.len());
                        coverage.insert(&entry, entries.len() as u32);
                    }
                    exact.insert(full_key, entries.len());
                    document.rules.push(EffectiveRule {
                        origin,
                        additional_origins: Vec::new(),
                        rule,
                    });
                    entries.push(entry);
                }
            }
        }
        // 去重与覆盖结构仅用于生成；先释放临时索引和原始输入，再构建最终索引。
        // 新旧活动快照仍由各自 Arc 保有，不能提前释放正在服务的旧版本。
        drop(ids);
        drop(sources);
        drop(exact);
        drop(conditions);
        drop(coverage);
        document.report.effective = entries.len();
        let index = Index::compile(&entries)?;
        let has_process = entries
            .iter()
            .any(|entry| matches!(entry.matcher, Matcher::ProcessName(_)));
        // global/direct 忽略普通国家规则，不查询其数据库或制造无关依赖故障。
        // 只有规则模式或仍有效的特殊 GeoIP REJECT 才要求 Ready。
        let has_geoip = entries.iter().any(|entry| {
            matches!(entry.matcher, Matcher::GeoIp(_))
                && (document.settings.mode == Mode::Rules || entry.policy == Policy::Reject)
        });
        let quic_rule = quic_guard(&entries);
        let quic_blocked = quic_rule.is_some();
        Ok(Self {
            instance: crate::lifecycle::identifier().ok_or(Error::ResourceLimit)?,
            document,
            entries,
            index,
            flat: None,
            _flat_memory: None,
            quic_blocked,
            quic_rule,
            has_geoip,
            has_process,
            geoip: None,
            _memory: memory,
            report_verification: ReportVerification::VerifiedFromSources,
        })
    }
    /// 标准 YAML 生成产物；不是允许用户直接编辑的原始配置。
    /// 编码为只读内存字节；由 App／CLI 决定是否持久化，不接收可写文件句柄。
    pub fn to_binary(&self) -> Result<super::Encoded> {
        super::cache::encode(self)
    }
    pub fn to_yaml(&self) -> Result<String> {
        let _temporary = MEMORY.reserve(scaled(
            self.document.report.original,
            RULE_BYTES,
            PARSE_BASE,
        )?)?;
        let document = &self.document;
        let mut writer = yaml::Writer::new();
        writer.field("format_version", &document.format_version, 0)?;
        writer.field("version", &document.version, 0)?;
        writer.field("settings", &document.settings, 0)?;
        writer.sequence("sources", &document.sources, 0)?;
        writer.sequence("rules", &document.rules, 0)?;
        writer.mapping("report")?;
        let report = &document.report;
        for (name, value) in [
            ("original", report.original),
            ("duplicates", report.duplicates),
            ("conflicts", report.conflicts),
            ("pruned", report.pruned),
            ("effective", report.effective),
        ] {
            writer.field(name, &value, 1)?;
        }
        writer.sequence("removals", &report.removals, 1)?;
        Ok(writer.finish())
    }
    /// 生成文件重新严格解析并比较全部值；复用已经编译的索引，不再创建第二份索引。
    #[cfg(test)]
    pub(super) fn verify_yaml(&self, input: &str) -> Result<()> {
        let _temporary = reserve_parse(input, yaml::MAX_BYTES)?;
        let document: Document = yaml::decode(input)?;
        // 直接比较完整类型值，包括来源与诊断，避免两份整文件 JSON 缓冲区。
        if self.document != document {
            return Err(Error::InvalidInput);
        }
        Ok(())
    }
    /// 读取文件只发生于加载或更新阶段，高频查询只访问已编译索引。
    pub fn from_yaml(input: &str) -> Result<Self> {
        if input.len() > yaml::MAX_BYTES {
            return Err(Error::ResourceLimit);
        }
        let temporary = reserve_parse(input, yaml::MAX_BYTES)?;
        let document: Document = yaml::decode(input)?;
        let result = Self::from_document(document);
        drop(temporary);
        result
    }
    /// Rust 诊断入口返回低敏感位置；常规入口与 C ABI 仍保留稳定错误类别。
    pub fn from_yaml_diagnosed(input: &str) -> std::result::Result<Self, ValidationDiagnostic> {
        let mut diagnostic = ValidationDiagnostic::new("parse");
        let parse = || -> Result<Document> { yaml::decode(input) };
        // 临时解析预算必须覆盖文档校验阶段，因此在该入口外层保留守卫。
        let _temporary = reserve_parse(input, yaml::MAX_BYTES).map_err(|error| {
            diagnostic.error = error;
            diagnostic.clone()
        })?;
        let document = parse().map_err(|error| {
            diagnostic.error = error;
            diagnostic
        })?;
        Self::from_document_diagnosed(document)
    }

    pub(crate) fn from_document(document: Document) -> Result<Self> {
        Self::from_document_diagnosed(document).map_err(|d| d.error)
    }
    fn from_document_diagnosed(
        document: Document,
    ) -> std::result::Result<Self, ValidationDiagnostic> {
        let mut diagnostic = ValidationDiagnostic::new("header");
        Self::validate_document(document, &mut diagnostic, true).map_err(|error| {
            diagnostic.error = error;
            diagnostic
        })
    }
    fn validate_document(
        document: Document,
        diagnostic: &mut ValidationDiagnostic,
        compile_index: bool,
    ) -> Result<Self> {
        if document.format_version != 1 || document.version == 0 {
            return Err(Error::InvalidInput);
        }
        if document.sources.len() > MAX_SOURCES
            || document.rules.len() > MAX_RULES
            || document.report.original > MAX_RULES
            || document.report.removals.len() > MAX_RULES
            || document.report.duplicates > MAX_RULES
            || document.report.conflicts > MAX_RULES
            || document.report.pruned > MAX_RULES
        {
            return Err(Error::ResourceLimit);
        }
        validate_settings(&document.settings)?;
        diagnostic.stage = "budget";
        let memory = MEMORY.reserve(document_charge(&document)?)?;
        diagnostic.stage = "sources";
        let sources = document.sources.as_slice();
        let mut ids = HashSet::new();
        for source in sources {
            identifier(&source.id)?;
            if !ids.insert(&source.id) {
                return Err(Error::InvalidInput);
            }
        }
        let mut origins = HashSet::new();
        let mut previous = None;
        let mut entries = Vec::with_capacity(document.rules.len());
        let mut additional = 0usize;
        for effective in &document.rules {
            diagnostic.stage = "rules";
            diagnostic.source = Some(effective.origin.source);
            diagnostic.sequence = Some(effective.origin.sequence);
            let source = sources
                .get(usize::from(effective.origin.source))
                .ok_or(Error::InvalidInput)?;
            let order = (
                source.layer.priority(),
                effective.origin.source,
                effective.origin.sequence,
            );
            if previous.is_some_and(|p| p >= order) {
                return Err(Error::InvalidInput);
            }
            previous = Some(order);
            for origin in std::iter::once(&effective.origin).chain(&effective.additional_origins) {
                if origin != &effective.origin {
                    let layer = sources
                        .get(usize::from(origin.source))
                        .ok_or(Error::InvalidInput)?
                        .layer;
                    let duplicate_order = (layer.priority(), origin.source, origin.sequence);
                    if duplicate_order <= order {
                        return Err(Error::InvalidInput);
                    }
                }
                if usize::from(origin.source) >= sources.len()
                    || origin.sequence as usize >= MAX_RULES
                    || !origins.insert((origin.source, origin.sequence))
                {
                    return Err(Error::InvalidInput);
                }
            }
            additional = additional
                .checked_add(effective.additional_origins.len())
                .ok_or(Error::ResourceLimit)?;
            if additional > MAX_RULES {
                return Err(Error::ResourceLimit);
            }
            let normalized = normalize_rule(&effective.rule, &document.settings.outbound)?;
            if normalized.value != effective.rule.value
                || normalized.outbound != effective.rule.outbound
            {
                return Err(Error::InvalidInput);
            }
            let source = &sources[usize::from(effective.origin.source)];
            entries.push(Entry {
                matcher: matcher(normalized.r#type, &normalized.value)?,
                protocol: normalized.protocol,
                layer: source.layer,
                policy: normalized.policy,
            });
        }
        diagnostic.stage = "report";
        diagnostic.source = None;
        diagnostic.sequence = None;
        let report = &document.report;
        if report.effective != entries.len()
            || report.duplicates != additional
            || report.removals.len() != report.duplicates + report.conflicts + report.pruned
            || report.original != report.effective + report.removals.len()
        {
            return Err(Error::InvalidInput);
        }
        let primaries: HashSet<_> = document
            .rules
            .iter()
            .map(|row| (row.origin.source, row.origin.sequence))
            .collect();
        let duplicates: HashMap<_, _> = document
            .rules
            .iter()
            .flat_map(|row| {
                row.additional_origins.iter().map(move |origin| {
                    (
                        (origin.source, origin.sequence),
                        (row.origin.source, row.origin.sequence),
                    )
                })
            })
            .collect();
        let mut removed = HashSet::new();
        let mut counts = [0usize; 3];
        for removal in &report.removals {
            diagnostic.stage = "removals";
            diagnostic.source = Some(removal.origin.source);
            diagnostic.sequence = Some(removal.origin.sequence);
            let origin = (removal.origin.source, removal.origin.sequence);
            if usize::from(removal.origin.source) >= sources.len()
                || removal.origin.sequence as usize >= MAX_RULES
                || !removed.insert(origin)
                || !primaries.contains(&(removal.covered_by.source, removal.covered_by.sequence))
            {
                return Err(Error::InvalidInput);
            }
            match removal.reason {
                RemovalReason::Duplicate => {
                    if duplicates.get(&origin)
                        != Some(&(removal.covered_by.source, removal.covered_by.sequence))
                    {
                        return Err(Error::InvalidInput);
                    }
                    counts[0] += 1;
                }
                RemovalReason::Conflict => {
                    if !origins.insert(origin) {
                        return Err(Error::InvalidInput);
                    }
                    counts[1] += 1;
                }
                RemovalReason::Covered => {
                    if !origins.insert(origin) {
                        return Err(Error::InvalidInput);
                    }
                    counts[2] += 1;
                }
            }
        }
        if counts != [report.duplicates, report.conflicts, report.pruned]
            || origins.len() != report.original
        {
            return Err(Error::InvalidInput);
        }
        // 每个启用来源的原始序号必须连续；引用数量一致不能接受伪造的序号空洞。
        let mut source_counts = [0usize; MAX_SOURCES];
        for &(source, _) in &origins {
            source_counts[usize::from(source)] += 1;
        }
        if origins
            .iter()
            .any(|&(source, sequence)| sequence as usize >= source_counts[usize::from(source)])
        {
            return Err(Error::InvalidInput);
        }
        let index = if compile_index {
            Index::compile(&entries)?
        } else {
            Index::default()
        };
        let has_process = entries
            .iter()
            .any(|entry| matches!(entry.matcher, Matcher::ProcessName(_)));
        // global/direct 忽略普通国家规则，不查询其数据库或制造无关依赖故障。
        // 只有规则模式或仍有效的特殊 GeoIP REJECT 才要求 Ready。
        let has_geoip = entries.iter().any(|entry| {
            matches!(entry.matcher, Matcher::GeoIp(_))
                && (document.settings.mode == Mode::Rules || entry.policy == Policy::Reject)
        });
        let quic_rule = quic_guard(&entries);
        let quic_blocked = quic_rule.is_some();
        Ok(Self {
            instance: crate::lifecycle::identifier().ok_or(Error::ResourceLimit)?,
            document,
            entries,
            index,
            flat: None,
            _flat_memory: None,
            quic_blocked,
            quic_rule,
            has_geoip,
            has_process,
            geoip: None,
            _memory: memory,
            report_verification: ReportVerification::StructuralOnly,
        })
    }
    /// 缓存加载只验证规则结构，直接采用已编译的平铺索引，不重建 Trie/自动机。
    pub(crate) fn from_flat_document(document: Document, bytes: Vec<u8>) -> Result<Self> {
        let mut diagnostic = ValidationDiagnostic::new("binary-cache");
        let mut snapshot = Self::validate_document(document, &mut diagnostic, false)?;
        snapshot._flat_memory = Some(MEMORY.reserve(bytes.len())?);
        snapshot.flat = Some(FlatIndex::parse(bytes, &snapshot.entries)?);
        // 平铺查询只需规则文档提供决策来源；校验用 Matcher/出口副本不再驻留。
        snapshot.entries.clear();
        snapshot.entries.shrink_to_fit();
        snapshot.release_build_reserve()?;
        Ok(snapshot)
    }
    /// 更新时在锁外平铺一次；在途决策可完成旧方案，已建连接不保留整份规则 Arc。
    pub(crate) fn into_flat(mut self) -> Result<Self> {
        let flat = self.index.flatten(&self.entries)?;
        // Builder 将临时租约收缩为实际字节容量并随 FlatIndex 交接，不能出现无预算窗口。
        self.flat = Some(flat);
        self.index = Index::default();
        self.entries.clear();
        self.entries.shrink_to_fit();
        self.release_build_reserve()?;
        Ok(self)
    }
    // 只有实际释放树和 Matcher 后缩减构建配额；旧快照的文档和字节租约继续计入共享预算。
    fn release_build_reserve(&mut self) -> Result<()> {
        let bytes = self
            .document
            .rules
            .iter()
            .try_fold(SNAPSHOT_BASE, |sum, row| {
                let strings = row
                    .rule
                    .value
                    .capacity()
                    .checked_mul(RESIDENT_STRING_FACTOR)
                    .and_then(|n| {
                        n.checked_add(row.rule.outbound.as_ref().map_or(0, String::capacity))
                    })
                    .ok_or(Error::ResourceLimit)?;
                sum.checked_add(RESIDENT_RULE_BYTES)
                    .and_then(|n| n.checked_add(strings))
                    .and_then(|n| {
                        n.checked_add(
                            row.additional_origins
                                .capacity()
                                .checked_mul(ORIGIN_BYTES)?,
                        )
                    })
                    .ok_or(Error::ResourceLimit)
            })?
            .checked_add(
                self.document
                    .report
                    .removals
                    .capacity()
                    .checked_mul(ORIGIN_BYTES)
                    .ok_or(Error::ResourceLimit)?,
            )
            .ok_or(Error::ResourceLimit)?;
        // 驻留估算必须小于已预留构建额度，异常时返回资源错误，不能触发断言或无预算成功。
        if bytes > self._memory.reserved_bytes() {
            return Err(Error::ResourceLimit);
        }
        self._memory.shrink_to(bytes);
        Ok(())
    }
    pub(crate) fn flat_bytes(&self) -> Result<&[u8]> {
        self.flat
            .as_ref()
            .map(FlatIndex::bytes)
            .ok_or(Error::InvalidState)
    }
    fn query(
        &self,
        name: Option<&str>,
        ip: Option<IpAddr>,
        port: u16,
        transport: super::Transport,
    ) -> Candidates {
        let query = |name, ip| match &self.flat {
            Some(index) => index.query(name, ip, port, transport),
            None => self.index.query(name, ip, port, transport),
        };
        if self.document.settings.mode == Mode::Rules {
            return query(name, ip);
        }
        let mut values = Candidates::default();
        let domain = name
            .map(|name| self.domain_candidates(name, port, transport))
            .unwrap_or_default();
        if Self::reject_candidate(domain).is_some() {
            values.merge(domain);
        } else {
            // 普通域名在 global/direct 中无效，不能抢先决定或阻止真实 IP 拦截查询。
            values.merge(query(None, ip));
        }
        let transport_values = query(None, None);
        for class in [
            RuleClass::Reject,
            RuleClass::RejectTcp,
            RuleClass::RejectUdp,
        ] {
            if let Some(id) = transport_values.get(class as usize) {
                values.add(class as usize, id as u32);
            }
        }
        let mut rejects = Candidates::default();
        for class in [
            RuleClass::Reject,
            RuleClass::RejectTcp,
            RuleClass::RejectUdp,
        ] {
            if let Some(id) = values.get(class as usize) {
                rejects.add(class as usize, id as u32);
            }
        }
        rejects
    }
    fn domain_candidates(&self, name: &str, port: u16, transport: super::Transport) -> Candidates {
        match &self.flat {
            Some(index) => index.domain_candidates(name, port).for_transport(transport),
            None => self
                .index
                .domain_candidates(name, port)
                .for_transport(transport),
        }
    }
    pub fn blocks_transport(&self, transport: super::Transport) -> bool {
        self.query(None, None, 0, transport)
            .get(RuleClass::Reject as usize)
            .is_some()
    }
    pub(crate) fn needs_process(&self) -> bool {
        self.has_process && self.document.settings.mode == Mode::Rules
    }
    fn merge_process(&self, process: Option<&str>, mut values: Candidates) -> Result<Candidates> {
        if !self.needs_process() {
            return Ok(values);
        }
        let Some(process) = process else {
            return Ok(values);
        }; // OS 无法确定时按用户约定跳过。
        if process.len() > crate::routing::MAX_PROCESS_NAME_BYTES {
            return Err(Error::ResourceLimit);
        }
        let matched = match &self.flat {
            Some(index) => index.process_candidates(process),
            None => self.index.process_candidates(process),
        };
        for (slot, id) in matched
            .0
            .into_iter()
            .enumerate()
            .filter(|(_, id)| *id != index::NONE)
        {
            if values.get(slot).is_none_or(|old| {
                self.candidate_priority(id as usize) < self.candidate_priority(old)
            }) {
                values.0[slot] = id;
            }
        }
        Ok(values)
    }
    pub(crate) fn require_ready_geoip(&self) -> Result<()> {
        if self.has_geoip && self.geoip.is_none() {
            Err(Error::GeoipInitializationFailed)
        } else {
            Ok(())
        }
    }
    /// Ready 服务对象由内核验证后绑定；数据库及其预算随真实快照引用存活。
    pub(crate) fn bind_geoip(mut self, database: std::sync::Arc<crate::geoip::Database>) -> Self {
        self.geoip = Some(database);
        self
    }
    fn geoip_candidates(&self, target: &Target) -> Result<Candidates> {
        if !self.has_geoip {
            return Ok(Candidates::default());
        }
        let database = self
            .geoip
            .as_ref()
            .ok_or(Error::GeoipInitializationFailed)?;
        let (name, address) = Self::metadata(target)?;
        let Some(address) = address else {
            return Ok(Candidates::default());
        };
        if name.as_deref().is_some_and(|name| {
            let domain = self.domain_candidates(name, target.port, target.transport);
            if self.document.settings.mode == Mode::Rules {
                domain.any()
            } else {
                Self::reject_candidate(domain).is_some()
            }
        }) {
            return Ok(Candidates::default());
        }
        let Some(code) = database.country_code(address)? else {
            return Ok(Candidates::default());
        };
        let mut values = match &self.flat {
            Some(index) => index.geoip_candidates(code),
            None => self.index.geoip_candidates(code),
        };
        if self.document.settings.mode != Mode::Rules {
            let reject = Self::reject_candidate(values);
            values = Candidates::default();
            if let Some(id) = reject {
                values.add(RuleClass::Reject as usize, id as u32);
            }
        }
        Ok(values)
    }
    fn merge_geoip(&self, target: &Target, mut values: Candidates) -> Result<Candidates> {
        let geoip = self.geoip_candidates(target)?;
        for (slot, id) in geoip
            .0
            .into_iter()
            .enumerate()
            .filter(|(_, id)| *id != index::NONE)
        {
            let selected = values.get(slot);
            // GeoIP 属于 IP 阶段；按用户最新确认，同来源显式 CIDR 优先 GeoIP。
            // 特殊 REJECT 不适用来源/类型排序，沿用该集合原有最早候选语义。
            let special = matches!(slot, s if s == RuleClass::Reject as usize
                || s == RuleClass::RejectTcp as usize || s == RuleClass::RejectUdp as usize);
            if selected.is_none_or(|old| {
                if special {
                    (id as usize) < old
                } else {
                    self.candidate_priority(id as usize) < self.candidate_priority(old)
                }
            }) {
                values.0[slot] = id;
            }
        }
        Ok(values)
    }
    fn quic_candidates(&self, name: &str) -> Candidates {
        match &self.flat {
            Some(index) => index.quic_candidates(name),
            None => self.index.quic_candidates(name),
        }
    }
    pub(crate) fn needs_datagram(&self, target: &Target) -> Result<bool> {
        let (name, _) = Self::metadata(target)?;
        Ok(target.transport == super::Transport::Udp
            && name.is_some_and(|name| {
                let values = self.quic_candidates(&name);
                if self.document.settings.mode == Mode::Rules {
                    values.any()
                } else {
                    Self::reject_candidate(values).is_some()
                }
            }))
    }
    fn datagram_query(&self, target: &Target, identified: bool) -> Result<Candidates> {
        self.require_ready_geoip()?;
        if identified && target.transport != super::Transport::Udp {
            return Err(Error::InvalidInput);
        }
        let (name, address) = Self::metadata(target)?;
        let mut scoped = if identified {
            name.as_deref()
                .map_or_else(Candidates::default, |name| self.quic_candidates(name))
        } else {
            Candidates::default()
        };
        if self.document.settings.mode != Mode::Rules {
            let reject = scoped.get(RuleClass::RejectUdp as usize);
            scoped = Candidates::default();
            if let Some(id) = reject {
                scoped.add(RuleClass::RejectUdp as usize, id as u32);
            }
        }
        // 实际命中的 QUIC+域名也是域名阶段命中，必须跳过 IP 规则。
        let mut values = self.query(
            name.as_deref(),
            if scoped.any() { None } else { address },
            target.port,
            target.transport,
        );
        values.merge(scoped);
        if identified && let Some(id) = self.quic_rule {
            values.add(RuleClass::Reject as usize, id as u32);
        }
        if scoped.any() {
            Ok(values)
        } else {
            self.merge_geoip(target, values)
        }
    }
    pub(crate) fn evaluate_identified(
        &self,
        target: &Target,
        identified: bool,
    ) -> Result<Decision> {
        self.evaluate_facts(target, identified, None)
    }
    pub(crate) fn evaluate_facts(
        &self,
        target: &Target,
        identified: bool,
        process: Option<&str>,
    ) -> Result<Decision> {
        Ok(self.decide(
            self.selected(self.merge_process(process, self.datagram_query(target, identified)?)?),
        ))
    }
    /// 根据实际首个 UDP 报文匹配 QUIC 条件；端口、域名或调用者布尔声明不能代替报文识别。
    /// 例如 protocol,quic,example.com,reject 仅命中该域及子域的已识别 QUIC。
    pub fn evaluate_datagram(&self, target: &Target, bytes: &[u8]) -> Result<Decision> {
        let identified = Self::identify_datagram(target, bytes)?;
        self.evaluate_identified(target, identified)
    }
    pub(crate) fn identify_datagram(target: &Target, bytes: &[u8]) -> Result<bool> {
        if target.transport != super::Transport::Udp {
            return Err(Error::InvalidInput);
        }
        if bytes.len() > crate::outbound::MAX_DATAGRAM {
            return Err(Error::ResourceLimit);
        }
        Ok(crate::protocol::quic::Flow::default().observe(bytes))
    }
    pub(crate) fn before_dns_facts(
        &self,
        target: &Target,
        identified: bool,
        process: Option<&str>,
    ) -> Result<Option<Decision>> {
        let (name, address) = Self::metadata(target)?;
        let values = self.merge_process(process, self.datagram_query(target, identified)?)?;
        if Self::reject_candidate(values).is_some() {
            return Ok(Some(self.decide(self.selected(values))));
        }
        let domain_hit = name.as_deref().is_some_and(|name| {
            self.domain_candidates(name, target.port, target.transport)
                .any()
                || (identified && self.quic_candidates(name).any())
        });
        if address.is_some() || (domain_hit && self.document.settings.mode == Mode::Rules) {
            Ok(Some(self.decide(self.selected(values))))
        } else {
            Ok(None)
        }
    }

    pub fn blocks_quic(&self) -> bool {
        self.quic_blocked
    }
    pub fn document(&self) -> &Document {
        &self.document
    }
    pub fn report_verification(&self) -> ReportVerification {
        self.report_verification
    }
    /// 宿主仍持有原始来源时重新生成并比较全部字段，证明裁剪报告和来源一致。
    /// 验证不会把不可信文档就地升级或影响正在使用的快照。
    pub fn verify_against_sources(&self, sources: Vec<Source>) -> Result<()> {
        let expected = Self::generate(
            self.document.settings.clone(),
            sources,
            self.document.version,
        )?;
        if expected.document != self.document {
            return Err(Error::InvalidInput);
        }
        Ok(())
    }
    pub(super) fn metadata(target: &Target) -> Result<(Option<String>, Option<IpAddr>)> {
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
        Ok((name, address))
    }
    fn reject_candidate(values: Candidates) -> Option<usize> {
        [
            RuleClass::Reject,
            RuleClass::RejectTcp,
            RuleClass::RejectUdp,
        ]
        .into_iter()
        .filter_map(|class| values.get(class as usize))
        .min()
    }
    fn selected(&self, values: Candidates) -> Option<usize> {
        if let Some(reject) = Self::reject_candidate(values) {
            return Some(reject);
        }
        // 索引只提供候选；按真实来源、独立类型优先级、原始先后裁决。
        // 插件先编译使共享候选槽始终保留插件/远程中更高来源的最早匹配项。
        values
            .0
            .into_iter()
            .filter(|id| *id != index::NONE)
            .map(|id| id as usize)
            .min_by_key(|id| self.candidate_priority(*id))
    }
    fn candidate_priority(&self, id: usize) -> (SourcePriority, TypePriority, usize) {
        let row = &self.document.rules[id];
        let layer = self.document.sources[usize::from(row.origin.source)].layer;
        let kind = if row.rule.protocol.is_some() {
            TypePriority::Protocol
        } else {
            match row.rule.r#type {
                Match::ProcessName => TypePriority::Process,
                Match::Transport | Match::DomainQuic => TypePriority::Protocol,
                Match::DestinationPort | Match::DomainPort => TypePriority::Port,
                Match::Domain => TypePriority::Domain,
                Match::DomainSuffix => TypePriority::Suffix,
                Match::DomainKeyword => TypePriority::Keyword,
                Match::IpCidr => TypePriority::Ip,
                Match::GeoIp => TypePriority::GeoIp,
            }
        };
        (layer.priority(), kind, id)
    }

    fn decide(&self, winner: Option<usize>) -> Decision {
        let settings = &self.document.settings;
        if let Some(id) = winner.filter(|id| {
            self.document.rules[*id].rule.policy == Policy::Reject || settings.mode == Mode::Rules
        }) {
            let entry = &self.document.rules[id].rule;
            return Decision {
                policy: entry.policy,
                outbound: entry.outbound.clone(),
                rule_index: Some(id),
                reason: Reason::Rule,
                rule_version: Some(self.document.version),
                origin: Some(self.document.rules[id].origin.clone()),
            };
        }
        let (policy, reason) = match settings.mode {
            Mode::Direct => (Policy::Direct, Reason::Mode),
            Mode::Global => (settings.fallback, Reason::Mode),
            Mode::Rules => (settings.fallback, Reason::Fallback),
        };
        Decision {
            policy,
            outbound: if policy == Policy::Proxy {
                settings.outbound.clone()
            } else {
                None
            },
            rule_index: None,
            reason,
            rule_version: Some(self.document.version),
            origin: None,
        }
    }
    /// 纯索引查询；域名命中跳过 IP，域名未命中的连接须解析并检查全部候选。
    pub fn evaluate(&self, target: &Target) -> Result<Decision> {
        self.evaluate_facts(target, false, None)
    }
    /// 域名命中直接决定动作；未命中时等待真实 DNS 后查询 IP。
    pub fn evaluate_before_dns(&self, target: &Target) -> Result<Option<Decision>> {
        let (name, address) = Self::metadata(target)?;
        let values = self.merge_geoip(
            target,
            self.query(name.as_deref(), address, target.port, target.transport),
        )?;
        if let Some(id) = Self::reject_candidate(values) {
            return Ok(Some(self.decide(Some(id))));
        }
        if self.document.settings.mode == Mode::Rules
            && name.as_deref().is_some_and(|name| {
                self.domain_candidates(name, target.port, target.transport)
                    .any()
            })
        {
            return Ok(Some(self.decide(self.selected(values))));
        }
        if address.is_none() {
            return Ok(None);
        }
        Ok(Some(self.decide(self.selected(values))))
    }
    /// 新语义的线性参考匹配器，仅用于验证和基准对照，不接入高频连接路径。
    pub fn evaluate_reference(&self, target: &Target) -> Result<Decision> {
        self.evaluate_reference_identified(target, false)
    }
    fn evaluate_reference_identified(&self, target: &Target, quic: bool) -> Result<Decision> {
        self.evaluate_reference_facts(target, quic, None)
    }
    fn evaluate_reference_facts(
        &self,
        target: &Target,
        quic: bool,
        process: Option<&str>,
    ) -> Result<Decision> {
        let (name, address) = Self::metadata(target)?;
        self.require_ready_geoip()?;
        // 平铺快照不常驻 Matcher 副本；参考实现仅在显式验证时临时恢复，绝不接入查询路径。
        // 并发显式参考验证也预留独立预算，不能用验证入口绕过临时对象配额。
        let _reference_memory = if self.flat.is_some() {
            Some(MEMORY.reserve(document_charge(&self.document)?)?)
        } else {
            None
        };
        let reconstructed;
        let entries = if self.flat.is_some() {
            reconstructed = self
                .document
                .rules
                .iter()
                .map(|effective| {
                    Ok(Entry {
                        matcher: matcher(effective.rule.r#type, &effective.rule.value)?,
                        protocol: effective.rule.protocol,
                        layer: self.document.sources[usize::from(effective.origin.source)].layer,
                        policy: effective.rule.policy,
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            &reconstructed
        } else {
            &self.entries
        };
        let active = |entry: &Entry| {
            self.document.settings.mode == Mode::Rules
                || (entry.policy == Policy::Reject
                    && (entry.protocol.is_some()
                        || matches!(
                            entry.matcher,
                            Matcher::Domain(_, false)
                                | Matcher::Network(_)
                                | Matcher::GeoIp(_)
                                | Matcher::Port(..)
                                | Matcher::DomainPort(..)
                                | Matcher::DomainQuic(_)
                                | Matcher::Transport(_)
                                | Matcher::Quic
                        )))
        };
        let domain_hit = entries
            .iter()
            .filter(|entry| active(entry) && entry.protocol.is_none_or(|p| p == target.transport))
            .any(|entry| match &entry.matcher {
                Matcher::Domain(value, suffix) => name.as_ref().is_some_and(|name| {
                    name == value
                        || (*suffix
                            && name
                                .strip_suffix(value)
                                .is_some_and(|prefix| prefix.ends_with('.')))
                }),
                Matcher::DomainQuic(value) => {
                    quic && name.as_deref().is_some_and(|name| {
                        name == value
                            || name
                                .strip_suffix(value)
                                .is_some_and(|prefix| prefix.ends_with('.'))
                    })
                }
                Matcher::DomainPort(value, port) => {
                    *port == target.port
                        && name.as_ref().is_some_and(|name| {
                            name == value
                                || name
                                    .strip_suffix(value)
                                    .is_some_and(|prefix| prefix.ends_with('.'))
                        })
                }
                Matcher::Keyword(value) => name
                    .as_deref()
                    .and_then(super::keyword_subject)
                    .is_some_and(|label| label.contains(value)),
                _ => false,
            });
        let country = if !domain_hit && self.has_geoip {
            let database = self
                .geoip
                .as_ref()
                .ok_or(Error::GeoipInitializationFailed)?;
            address
                .map(|ip| database.country_code(ip))
                .transpose()?
                .flatten()
        } else {
            None
        };
        let hit = |entry: &Entry| {
            if !active(entry) || entry.protocol.is_some_and(|p| p != target.transport) {
                return false;
            }
            match &entry.matcher {
                Matcher::Domain(value, suffix) => name.as_ref().is_some_and(|name| {
                    name == value
                        || (*suffix
                            && name
                                .strip_suffix(value)
                                .is_some_and(|prefix| prefix.ends_with('.')))
                }),
                Matcher::DomainQuic(value) => {
                    quic && name.as_deref().is_some_and(|name| {
                        name == value
                            || name
                                .strip_suffix(value)
                                .is_some_and(|prefix| prefix.ends_with('.'))
                    })
                }
                Matcher::DomainPort(value, port) => {
                    *port == target.port
                        && name.as_ref().is_some_and(|name| {
                            name == value
                                || name
                                    .strip_suffix(value)
                                    .is_some_and(|prefix| prefix.ends_with('.'))
                        })
                }
                Matcher::Keyword(value) => name
                    .as_deref()
                    .and_then(super::keyword_subject)
                    .is_some_and(|label| label.contains(value)),
                Matcher::ProcessName(value) => process == Some(value.as_str()),
                Matcher::GeoIp(code) => {
                    !domain_hit && country.is_some_and(|country| country == code)
                }
                Matcher::Network(network) => {
                    !domain_hit && address.is_some_and(|ip| network.includes(ip))
                }
                Matcher::Port(first, last) => (*first..=*last).contains(&target.port),
                Matcher::Transport(value) => *value == target.transport,
                Matcher::Quic => quic,
            }
        };
        for (id, entry) in entries.iter().enumerate() {
            let reject = entry.policy == Policy::Reject
                && (entry.protocol.is_some()
                    || matches!(
                        entry.matcher,
                        Matcher::Domain(_, false)
                            | Matcher::Network(_)
                            | Matcher::GeoIp(_)
                            | Matcher::Port(..)
                            | Matcher::DomainPort(..)
                            | Matcher::DomainQuic(_)
                            | Matcher::Transport(_)
                            | Matcher::Quic
                    ));
            if reject && hit(entry) {
                return Ok(self.decide(Some(id)));
            }
        }
        // 独立遍历真实来源与条件形态，不调用索引槽位、Entry::class 或 selected。
        for layer in [Layer::Local, Layer::Plugin, Layer::Remote] {
            for kind in [
                Match::ProcessName,
                Match::Transport,
                Match::DestinationPort,
                Match::Domain,
                Match::DomainSuffix,
                Match::DomainKeyword,
                Match::IpCidr,
                Match::GeoIp,
            ] {
                for (id, entry) in entries.iter().enumerate() {
                    let row = &self.document.rules[id].rule;
                    let shape = if row.r#type == Match::DomainQuic {
                        Match::Transport
                    } else if row.r#type == Match::DomainPort {
                        Match::DestinationPort
                    } else if row.protocol.is_some() {
                        Match::Transport
                    } else {
                        row.r#type
                    };
                    if entry.layer == layer && shape == kind && hit(entry) {
                        return Ok(self.decide(Some(id)));
                    }
                }
            }
        }
        Ok(self.decide(None))
    }
}

// 跨客户端只传递进程内不复用整数句柄，弱引用不会替宿主持有业务快照。
fn snapshots() -> &'static std::sync::Mutex<HashMap<u64, std::sync::Weak<Snapshot>>> {
    static SNAPSHOTS: std::sync::OnceLock<
        std::sync::Mutex<HashMap<u64, std::sync::Weak<Snapshot>>>,
    > = std::sync::OnceLock::new();
    SNAPSHOTS.get_or_init(|| std::sync::Mutex::new(HashMap::new()))
}
pub(crate) fn register(snapshot: &std::sync::Arc<Snapshot>) -> Result<()> {
    let mut values = snapshots().lock().map_err(|_| Error::Internal)?;
    values.retain(|_, value| value.strong_count() > 0);
    if values.len() >= MAX_HANDLES && !values.contains_key(&snapshot.instance) {
        return Err(Error::ResourceLimit);
    }
    values.insert(snapshot.instance, std::sync::Arc::downgrade(snapshot));
    Ok(())
}
pub(crate) fn lookup(id: u64) -> Result<std::sync::Arc<Snapshot>> {
    snapshots()
        .lock()
        .map_err(|_| Error::Internal)?
        .get(&id)
        .and_then(std::sync::Weak::upgrade)
        .ok_or(Error::InvalidState)
}
impl Snapshot {
    pub fn instance_handle(&self) -> u64 {
        self.instance
    }
}

/// 不做裁剪、不建立索引的参考实现，用于独立验证生成阶段没有改变规则含义。
pub struct Reference(Snapshot);
impl Reference {
    /// 为独立参考匹配绑定真实 Ready 服务；不接入正常连接快路径。
    pub fn compile_with_geoip(
        settings: Config,
        sources: Vec<Source>,
        version: u64,
        service: &crate::geoip::Service,
    ) -> Result<Self> {
        let database = service.database()?;
        let mut reference = Self::compile(settings, sources, version)?;
        reference.0 = reference.0.bind_geoip(database);
        Ok(reference)
    }
    pub fn compile(settings: Config, sources: Vec<Source>, version: u64) -> Result<Self> {
        validate_settings(&settings)?;
        if sources.len() > MAX_SOURCES || version == 0 {
            return Err(Error::InvalidInput);
        }
        let memory = MEMORY.reserve(charge(&sources)?)?;
        let mut ids = HashSet::new();
        for source in &sources {
            identifier(&source.id)?;
            if !ids.insert(&source.id) {
                return Err(Error::InvalidInput);
            }
        }
        let mut document = Document {
            format_version: 1,
            version,
            settings,
            sources: sources
                .iter()
                .map(|s| SourceIdentity {
                    id: s.id.clone(),
                    layer: s.layer,
                })
                .collect(),
            rules: Vec::new(),
            report: Report::default(),
        };
        let mut entries = Vec::new();
        for layer in Layer::ORDER {
            for (id, source) in sources
                .iter()
                .enumerate()
                .filter(|(_, source)| source.enabled && source.layer == layer)
            {
                for (sequence, input) in source.rules.iter().enumerate() {
                    let rule = normalize_rule(input, &document.settings.outbound)?;
                    entries.push(Entry {
                        matcher: matcher(rule.r#type, &rule.value)?,
                        protocol: rule.protocol,
                        layer,
                        policy: rule.policy,
                    });
                    document.rules.push(EffectiveRule {
                        origin: Origin {
                            source: id as u16,
                            sequence: sequence as u32,
                        },
                        additional_origins: Vec::new(),
                        rule,
                    });
                }
            }
        }
        document.report.original = entries.len();
        document.report.effective = entries.len();
        let has_process = entries
            .iter()
            .any(|entry| matches!(entry.matcher, Matcher::ProcessName(_)));
        // global/direct 忽略普通国家规则，不查询其数据库或制造无关依赖故障。
        // 只有规则模式或仍有效的特殊 GeoIP REJECT 才要求 Ready。
        let has_geoip = entries.iter().any(|entry| {
            matches!(entry.matcher, Matcher::GeoIp(_))
                && (document.settings.mode == Mode::Rules || entry.policy == Policy::Reject)
        });
        let quic_rule = quic_guard(&entries);
        let quic_blocked = quic_rule.is_some();
        Ok(Self(Snapshot {
            instance: crate::lifecycle::identifier().ok_or(Error::ResourceLimit)?,
            document,
            entries,
            index: Index::default(),
            flat: None,
            _flat_memory: None,
            quic_blocked,
            quic_rule,
            has_geoip,
            has_process,
            geoip: None,
            _memory: memory,
            report_verification: ReportVerification::VerifiedFromSources,
        }))
    }
    pub fn evaluate(&self, target: &Target) -> Result<Decision> {
        self.0.evaluate_reference(target)
    }
    /// 参考实现独立遍历条件和来源；报文识别复用同一协议解析，不调用索引裁决。
    pub fn evaluate_datagram(&self, target: &Target, bytes: &[u8]) -> Result<Decision> {
        let quic = Snapshot::identify_datagram(target, bytes)?;
        self.0.evaluate_reference_identified(target, quic)
    }
}

#[cfg(test)]
#[path = "../../tests/test_routing_runtime.rs"]
mod test_routing_runtime;
