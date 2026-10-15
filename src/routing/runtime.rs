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
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Layer {
    Local,
    Remote,
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
                    identifier(id)?;
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
    RemoteIp = 5,
    RemoteDomain = 6,
    RemoteDomainSuffix = 7,
    RemotePortTransport = 8,
    LocalKeyword = 9,
    RemoteKeyword = 10,
    // 独立槽位避免不匹配协议遮蔽同域其他候选；编号仍不表示优先级。
    RejectTcp = 11,
    RejectUdp = 12,
    LocalDomainTcp = 13,
    RemoteDomainTcp = 14,
    LocalDomainUdp = 15,
    RemoteDomainUdp = 16,
    LocalSuffixTcp = 17,
    RemoteSuffixTcp = 18,
    LocalSuffixUdp = 19,
    RemoteSuffixUdp = 20,
}
impl RuleClass {
    pub(super) const COUNT: usize = Self::RemoteSuffixUdp as usize + 1;
    fn local_equivalent(self) -> Self {
        match self {
            Self::RemoteIp => Self::LocalIp,
            Self::RemoteDomain => Self::LocalDomain,
            Self::RemoteDomainSuffix => Self::LocalDomainSuffix,
            Self::RemotePortTransport => Self::LocalPortTransport,
            Self::RemoteKeyword => Self::LocalKeyword,
            Self::RemoteDomainTcp => Self::LocalDomainTcp,
            Self::RemoteDomainUdp => Self::LocalDomainUdp,
            Self::RemoteSuffixTcp => Self::LocalSuffixTcp,
            Self::RemoteSuffixUdp => Self::LocalSuffixUdp,
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
    // 索引槽位按来源存放；选择顺序另按类型优先，本地只在同类型内优先。
    pub(super) fn class(&self) -> RuleClass {
        if self.policy == Policy::Reject {
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
                (Matcher::Domain(_, false), Layer::Remote, super::Transport::Tcp) => {
                    RuleClass::RemoteDomainTcp
                }
                (Matcher::Domain(_, false), Layer::Local, super::Transport::Udp) => {
                    RuleClass::LocalDomainUdp
                }
                (Matcher::Domain(_, false), Layer::Remote, super::Transport::Udp) => {
                    RuleClass::RemoteDomainUdp
                }
                (Matcher::Domain(_, true), Layer::Local, super::Transport::Tcp) => {
                    RuleClass::LocalSuffixTcp
                }
                (Matcher::Domain(_, true), Layer::Remote, super::Transport::Tcp) => {
                    RuleClass::RemoteSuffixTcp
                }
                (Matcher::Domain(_, true), Layer::Local, super::Transport::Udp) => {
                    RuleClass::LocalSuffixUdp
                }
                (Matcher::Domain(_, true), Layer::Remote, super::Transport::Udp) => {
                    RuleClass::RemoteSuffixUdp
                }
                _ => unreachable!("组合规则已完成域名类型校验"),
            };
        }
        match (&self.matcher, self.layer) {
            (Matcher::Network(_), Layer::Local) => RuleClass::LocalIp,
            (Matcher::Network(_), Layer::Remote) => RuleClass::RemoteIp,
            (Matcher::Domain(_, false), Layer::Local) => RuleClass::LocalDomain,
            (Matcher::Domain(_, false), Layer::Remote) => RuleClass::RemoteDomain,
            (Matcher::Domain(_, true), Layer::Local) => RuleClass::LocalDomainSuffix,
            (Matcher::Domain(_, true), Layer::Remote) => RuleClass::RemoteDomainSuffix,
            (Matcher::Keyword(_), Layer::Local) => RuleClass::LocalKeyword,
            (Matcher::Keyword(_), Layer::Remote) => RuleClass::RemoteKeyword,
            (_, Layer::Local) => RuleClass::LocalPortTransport,
            (_, Layer::Remote) => RuleClass::RemotePortTransport,
        }
    }
}
/// 只读执行索引与资源所有者；报告信任程度不参与策略匹配。
// QUIC 属于 UDP 数据面识别，不扩充正式候选槽位；编译和缓存恢复共用此准入判断。
fn quic_guard(entries: &[Entry]) -> bool {
    entries
        .iter()
        .any(|entry| matches!(entry.matcher, Matcher::Quic) && entry.policy == Policy::Reject)
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
    report_verification: ReportVerification,
}
pub(crate) fn normalize_condition(rule: &Rule) -> Result<String> {
    let value = match matcher(rule.r#type, &rule.value)? {
        Matcher::Domain(value, _) | Matcher::Keyword(value) => value,
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
    if rule.value.len() > 253 || rule.outbound.as_ref().is_some_and(|id| id.len() > 64) {
        return Err(Error::InvalidInput);
    }
    let labels = if rule.r#type == Match::DomainSuffix {
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
        if rule.value.len() > 253 || rule.outbound.as_ref().is_some_and(|id| id.len() > 64) {
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
        identifier(id)?;
    }
    if (settings.mode == Mode::Global || settings.fallback == Policy::Proxy)
        && settings.outbound.is_none()
    {
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
        let mut conditions = HashMap::<(u8, Match, String, Option<super::Transport>), usize>::new();
        let mut coverage = Index::default();
        // 本地总是在远程之前编译；同层级保持输入订阅顺序与各订阅内部序号。
        for layer in [Layer::Local, Layer::Remote] {
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
                        let level = u8::from(layer == Layer::Remote);
                        let condition = (level, kind, value.clone(), rule.protocol);
                        let same = conditions.get(&condition).copied();
                        let covered = coverage.coverage(&entry.matcher);
                        let winner = same.or_else(|| {
                            // 只裁剪同类包含关系；绝不由 DNS 的偶然结果裁剪跨 IP/域名规则。
                            let class = entry.class();
                            covered.get(class as usize).or_else(|| {
                                if layer == Layer::Remote {
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
        let quic_blocked = quic_guard(&entries);
        Ok(Self {
            instance: crate::lifecycle::identifier().ok_or(Error::ResourceLimit)?,
            document,
            entries,
            index,
            flat: None,
            _flat_memory: None,
            quic_blocked,
            _memory: memory,
            report_verification: ReportVerification::VerifiedFromSources,
        })
    }
    /// 标准 YAML 生成产物；不是允许用户直接编辑的原始配置。
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
    /// 二进制缓存与 YAML 导入共用严格文档校验，缓存直接使用平铺索引。
    /// 仅迁移旧内部生成文件；用户配置入口仍拒绝小写规则类型。
    pub(crate) fn migrate_yaml(input: &str) -> Result<Self> {
        let _temporary = reserve_parse(input, yaml::MAX_BYTES)?;
        let mut value: serde_json::Value = yaml::decode(input)?;
        let rules = value
            .get_mut("rules")
            .and_then(serde_json::Value::as_array_mut)
            .ok_or(Error::InvalidInput)?;
        if rules.len() > MAX_RULES {
            return Err(Error::ResourceLimit);
        }
        for row in rules {
            if let Some(kind) = row.get_mut("rule").and_then(|rule| rule.get_mut("type"))
                && let Some(name) = kind.as_str()
            {
                let renamed = match name {
                    "domain" => "DOMAIN",
                    "domain-suffix" => "DOMAIN-SUFFIX",
                    "domain-keyword" => "DOMAIN-KEYWORD",
                    "ip-cidr" => "IP-CIDR",
                    other => other,
                };
                *kind = renamed.into();
            }
        }
        Self::from_document(serde_json::from_value(value).map_err(|_| Error::InvalidInput)?)
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
                u8::from(source.layer == Layer::Remote),
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
                    let duplicate_order = (
                        u8::from(layer == Layer::Remote),
                        origin.source,
                        origin.sequence,
                    );
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
        let quic_blocked = quic_guard(&entries);
        Ok(Self {
            instance: crate::lifecycle::identifier().ok_or(Error::ResourceLimit)?,
            document,
            entries,
            index,
            flat: None,
            _flat_memory: None,
            quic_blocked,
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
        match &self.flat {
            Some(index) => index.query(name, ip, port, transport),
            None => self.index.query(name, ip, port, transport),
        }
    }
    fn domain_candidates(&self, name: &str, transport: super::Transport) -> Candidates {
        match &self.flat {
            Some(index) => index.domain_candidates(name).for_transport(transport),
            None => self.index.domain_candidates(name).for_transport(transport),
        }
    }
    pub fn blocks_transport(&self, transport: super::Transport) -> bool {
        self.query(None, None, 0, transport)
            .get(RuleClass::Reject as usize)
            .is_some()
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
    fn selected(values: Candidates) -> Option<usize> {
        if let Some(reject) = Self::reject_candidate(values) {
            return Some(reject);
        }
        // 域名命中时索引已跳过 IP。普通域名/IP 候选先于端口/全域传输候选，
        // 本地只在同类型内优先；关键词也是域名规则，不能被本地端口越级覆盖。
        [
            RuleClass::LocalIp,
            RuleClass::RemoteIp,
            RuleClass::LocalDomainTcp,
            RuleClass::RemoteDomainTcp,
            RuleClass::LocalDomainUdp,
            RuleClass::RemoteDomainUdp,
            RuleClass::LocalSuffixTcp,
            RuleClass::RemoteSuffixTcp,
            RuleClass::LocalSuffixUdp,
            RuleClass::RemoteSuffixUdp,
            RuleClass::LocalDomain,
            RuleClass::RemoteDomain,
            RuleClass::LocalDomainSuffix,
            RuleClass::RemoteDomainSuffix,
            RuleClass::LocalKeyword,
            RuleClass::RemoteKeyword,
            RuleClass::LocalPortTransport,
            RuleClass::RemotePortTransport,
        ]
        .into_iter()
        .find_map(|class| values.get(class as usize))
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
            Mode::Global => (Policy::Proxy, Reason::Mode),
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
        let (name, address) = Self::metadata(target)?;
        Ok(self.decide(Self::selected(self.query(
            name.as_deref(),
            address,
            target.port,
            target.transport,
        ))))
    }
    /// 域名命中直接决定动作；未命中时等待真实 DNS 后查询 IP。
    pub fn evaluate_before_dns(&self, target: &Target) -> Result<Option<Decision>> {
        let (name, address) = Self::metadata(target)?;
        let values = self.query(name.as_deref(), address, target.port, target.transport);
        if let Some(id) = Self::reject_candidate(values) {
            return Ok(Some(self.decide(Some(id))));
        }
        if name
            .as_deref()
            .is_some_and(|name| self.domain_candidates(name, target.transport).any())
        {
            return Ok(Some(self.decide(Self::selected(values))));
        }
        if address.is_none() {
            return Ok(None);
        }
        Ok(Some(self.decide(Self::selected(values))))
    }
    /// 新语义的线性参考匹配器，仅用于验证和基准对照，不接入高频连接路径。
    pub fn evaluate_reference(&self, target: &Target) -> Result<Decision> {
        let (name, address) = Self::metadata(target)?;
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
        // 独立线性预扫描确定域名是否命中；命中时不评估任何 IP 规则。
        let domain_hit = entries
            .iter()
            .filter(|entry| entry.protocol.is_none_or(|p| p == target.transport))
            .any(|entry| match &entry.matcher {
                Matcher::Domain(value, suffix) => name.as_ref().is_some_and(|name| {
                    name == value
                        || (*suffix
                            && name
                                .strip_suffix(value)
                                .is_some_and(|prefix| prefix.ends_with('.')))
                }),
                Matcher::Keyword(value) => name.as_ref().is_some_and(|name| name.contains(value)),
                _ => false,
            });
        let mut reject = None;
        let (
            mut scoped_exact_local,
            mut scoped_exact_remote,
            mut scoped_suffix_local,
            mut scoped_suffix_remote,
        ) = (None, None, None, None);
        let mut classes = [None; RuleClass::COUNT];
        for (id, entry) in entries.iter().enumerate() {
            if entry.protocol.is_some_and(|p| p != target.transport) {
                continue;
            }
            let matches = match &entry.matcher {
                Matcher::Domain(value, suffix) => name.as_ref().is_some_and(|name| {
                    name == value
                        || (*suffix
                            && name
                                .strip_suffix(value)
                                .is_some_and(|prefix| prefix.ends_with('.')))
                }),
                Matcher::Keyword(value) => name.as_ref().is_some_and(|name| name.contains(value)),
                Matcher::Network(network) => {
                    !domain_hit && address.is_some_and(|ip| network.includes(ip))
                }
                Matcher::Port(first, last) => (*first..=*last).contains(&target.port),
                Matcher::Transport(value) => *value == target.transport,
                Matcher::Quic => false,
            };
            if !matches {
                continue;
            }
            if entry.policy == Policy::Reject {
                reject.get_or_insert(id);
                continue;
            }
            if matches!(entry.matcher, Matcher::Keyword(_)) {
                classes[if entry.layer == Layer::Local {
                    RuleClass::LocalKeyword
                } else {
                    RuleClass::RemoteKeyword
                } as usize]
                    .get_or_insert(id);
                continue;
            }
            if entry.protocol.is_some() {
                // 独立按条件形态和来源选择，不调用 Entry::class 或生产候选裁决。
                let slot = match (&entry.matcher, entry.layer) {
                    (Matcher::Domain(_, false), Layer::Local) => &mut scoped_exact_local,
                    (Matcher::Domain(_, false), Layer::Remote) => &mut scoped_exact_remote,
                    (Matcher::Domain(_, true), Layer::Local) => &mut scoped_suffix_local,
                    (Matcher::Domain(_, true), Layer::Remote) => &mut scoped_suffix_remote,
                    _ => return Err(Error::InvalidInput),
                };
                slot.get_or_insert(id);
                continue;
            }
            let class = match (&entry.matcher, entry.layer) {
                (Matcher::Network(_), Layer::Local) => RuleClass::LocalIp,
                (Matcher::Network(_), Layer::Remote) => RuleClass::RemoteIp,
                (Matcher::Domain(_, false), Layer::Local) => RuleClass::LocalDomain,
                (Matcher::Domain(_, false), Layer::Remote) => RuleClass::RemoteDomain,
                (Matcher::Domain(_, true), Layer::Local) => RuleClass::LocalDomainSuffix,
                (Matcher::Domain(_, true), Layer::Remote) => RuleClass::RemoteDomainSuffix,
                (_, Layer::Local) => RuleClass::LocalPortTransport,
                (_, Layer::Remote) => RuleClass::RemotePortTransport,
            };
            classes[class as usize].get_or_insert(id);
        }
        if let Some(reject) = reject {
            return Ok(self.decide(Some(reject)));
        }
        if let Some(id) = scoped_exact_local
            .or(scoped_exact_remote)
            .or(scoped_suffix_local)
            .or(scoped_suffix_remote)
        {
            return Ok(self.decide(Some(id)));
        }
        // 独立参考实现显式遍历类型及来源，不复用索引选择函数。
        // 逐层显式检查，不借用索引的候选合并逻辑。
        for pair in [
            [RuleClass::LocalIp, RuleClass::RemoteIp],
            [RuleClass::LocalDomain, RuleClass::RemoteDomain],
            [RuleClass::LocalDomainSuffix, RuleClass::RemoteDomainSuffix],
            [RuleClass::LocalKeyword, RuleClass::RemoteKeyword],
            [
                RuleClass::LocalPortTransport,
                RuleClass::RemotePortTransport,
            ],
        ] {
            for class in pair {
                if let Some(id) = classes[class as usize] {
                    return Ok(self.decide(Some(id)));
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
        for layer in [Layer::Local, Layer::Remote] {
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
        let quic_blocked = quic_guard(&entries);
        Ok(Self(Snapshot {
            instance: crate::lifecycle::identifier().ok_or(Error::ResourceLimit)?,
            document,
            entries,
            index: Index::default(),
            flat: None,
            _flat_memory: None,
            quic_blocked,
            _memory: memory,
            report_verification: ReportVerification::VerifiedFromSources,
        }))
    }
    pub fn evaluate(&self, target: &Target) -> Result<Decision> {
        self.0.evaluate_reference(target)
    }
}

#[cfg(test)]
mod validation_tests {
    use super::*;
    #[test]
    fn full_value_validation_detects_provenance_and_report_changes() {
        let snapshot = Snapshot::generate(
            Config::default(),
            vec![Source {
                id: "local".into(),
                layer: Layer::Local,
                enabled: true,
                rules: vec![Rule {
                    protocol: None,
                    r#type: Match::Domain,
                    value: "example.com".into(),
                    policy: Policy::Reject,
                    outbound: None,
                }],
            }],
            7,
        )
        .unwrap();
        let valid = snapshot.to_yaml().unwrap();
        snapshot.verify_yaml(&valid).unwrap();
        for field in 0..5 {
            let mut document = snapshot.document().clone();
            match field {
                0 => document.rules[0].rule.value = "different.com".into(),
                1 => document.version += 1,
                2 => document.rules[0].origin.sequence += 1,
                3 => document.report.effective = 0,
                _ => document.sources[0].id = "other".into(),
            }
            let invalid = serde_json::to_string(&document).unwrap();
            assert!(snapshot.verify_yaml(&invalid).is_err());
        }
    }
    #[test]
    fn audit_migration_and_input_leases_are_bounded() {
        assert!(matches!(
            Snapshot::migrate_yaml(&"x".repeat(yaml::MAX_BYTES + 1)),
            Err(Error::ResourceLimit)
        ));
        assert!(matches!(
            scaled(usize::MAX, 8, PARSE_BASE),
            Err(Error::ResourceLimit)
        ));
        let input =
            GenerationInput::from_yaml(r#"{"format_version":1,"settings":{},"sources":[]}"#)
                .unwrap();
        assert!(input._memory.is_some());
        drop(input);
        assert!(GenerationInput::from_yaml("unknown: true").is_err());
        assert!(
            GenerationInput::from_yaml(r#"{"format_version":1,"settings":{},"sources":[]}"#)
                .is_ok()
        );
    }
    #[test]
    fn audit_registry_uses_weak_owners_and_releases_handles() {
        for _ in 0..256 {
            let snapshot =
                std::sync::Arc::new(Snapshot::generate(Config::default(), vec![], 1).unwrap());
            let handle = snapshot.instance_handle();
            register(&snapshot).unwrap();
            let retained = lookup(handle).unwrap();
            drop(snapshot);
            assert!(lookup(handle).is_ok());
            drop(retained);
            assert!(matches!(lookup(handle), Err(Error::InvalidState)));
        }
    }
    #[test]
    fn shared_pool_covers_compilation_flattening_cache_io_and_retained_snapshots() {
        let baseline = memory_usage().reserved_bytes;
        let snapshot = Snapshot::generate(Config::default(), vec![], 1)
            .unwrap()
            .into_flat()
            .unwrap();
        let retained = memory_usage().reserved_bytes;
        assert!(retained > baseline);
        let occupied = MEMORY.reserve(MAX_MEMORY - retained).unwrap();
        assert!(matches!(
            Snapshot::generate(Config::default(), vec![], 2),
            Err(Error::ResourceBudgetExceeded { .. })
        ));
        assert!(matches!(
            Index::default().flatten(&[]),
            Err(Error::ResourceBudgetExceeded { .. })
        ));
        // 安全创建的文件也必须先通过共享预算准入；超限时不能写入任何字节。
        let path = std::env::temp_dir().join(format!(
            "fluxa-cache-budget-{}-{}",
            std::process::id(),
            crate::lifecycle::identifier().unwrap()
        ));
        let mut file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap();
        assert!(matches!(
            crate::routing::cache::write(&mut file, &snapshot),
            Err(Error::ResourceBudgetExceeded { .. })
        ));
        assert_eq!(file.metadata().unwrap().len(), 0);
        drop(file);
        std::fs::remove_file(path).unwrap();
        drop(occupied);
        assert_eq!(memory_usage().reserved_bytes, retained);
        drop(snapshot);
        assert_eq!(memory_usage().reserved_bytes, baseline);
        assert!(memory_usage().peak_reserved_bytes <= MAX_MEMORY);
    }
    #[test]
    fn rule_class_slots_never_define_decision_priority() {
        let classes = [
            RuleClass::Reject,
            RuleClass::LocalIp,
            RuleClass::LocalDomain,
            RuleClass::LocalDomainSuffix,
            RuleClass::LocalPortTransport,
            RuleClass::RemoteIp,
            RuleClass::RemoteDomain,
            RuleClass::RemoteDomainSuffix,
            RuleClass::RemotePortTransport,
            RuleClass::LocalKeyword,
            RuleClass::RemoteKeyword,
            RuleClass::RejectTcp,
            RuleClass::RejectUdp,
            RuleClass::LocalDomainTcp,
            RuleClass::RemoteDomainTcp,
            RuleClass::LocalDomainUdp,
            RuleClass::RemoteDomainUdp,
            RuleClass::LocalSuffixTcp,
            RuleClass::RemoteSuffixTcp,
            RuleClass::LocalSuffixUdp,
            RuleClass::RemoteSuffixUdp,
        ];
        assert_eq!(classes.len(), RuleClass::COUNT);
        for (slot, class) in classes.into_iter().enumerate() {
            assert_eq!(slot, class as usize);
        }
        let mut values = Candidates::default();
        values.add(RuleClass::LocalDomain as usize, 10);
        values.add(RuleClass::RemoteIp as usize, 11);
        assert_eq!(
            Snapshot::selected(values),
            Some(11),
            "槽位2不能因为数值更小而覆盖槽位5的IP候选"
        );
        values.add(RuleClass::LocalPortTransport as usize, 1);
        assert_eq!(Snapshot::selected(values), Some(11), "端口规则不得越级");
    }
}
