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
#[path = "yaml.rs"]
mod yaml;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GenerationInput {
    pub format_version: u32,
    pub settings: Config,
    pub sources: Vec<Source>,
}
impl GenerationInput {
    pub fn from_yaml(input: &str) -> Result<Self> {
        if input.len() > yaml::MAX_BYTES {
            return Err(Error::ResourceLimit);
        }
        let _temporary =
            MEMORY.reserve(input.len().checked_mul(8).ok_or(Error::ResourceLimit)? + 4096)?;
        let value: Self = yaml::decode(input)?;
        if value.format_version != 1 || value.sources.len() > MAX_SOURCES {
            return Err(Error::InvalidInput);
        }
        validate_settings(&value.settings)?;
        charge(&value.sources)?;
        Ok(value)
    }
}
pub const MAX_RULES: usize = 100_000;
pub const MAX_SOURCES: usize = 64;
// 全局预算同时约束存量快照、新快照与编译临时对象。每次生成先预留最坏估算，
// 不把数量上限当作内存上限；高标签数输入可能在达到规则数量前被拒绝。
static MEMORY: ByteBudget = ByteBudget::new(512 * 1024 * 1024);
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Layer {
    Local,
    Remote,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub id: String,
    pub layer: Layer,
    #[serde(default = "enabled")]
    pub enabled: bool,
    pub rules: Vec<Rule>,
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
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EffectiveRule {
    pub origin: Origin,
    pub additional_origins: Vec<Origin>,
    pub rule: Rule,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
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
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Removal {
    pub origin: Origin,
    pub reason: RemovalReason,
    pub covered_by: Origin,
}
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Report {
    pub original: usize,
    pub duplicates: usize,
    pub conflicts: usize,
    pub pruned: usize,
    pub effective: usize,
    pub removals: Vec<Removal>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Document {
    pub format_version: u32,
    pub version: u64,
    pub settings: Config,
    pub sources: Vec<SourceIdentity>,
    pub rules: Vec<EffectiveRule>,
    pub report: Report,
}
pub(super) struct Entry {
    pub(super) matcher: Matcher,
    layer: Layer,
    policy: Policy,
    outbound: Option<String>,
}
impl Entry {
    // 索引槽位按来源存放；选择顺序另按类型优先，本地只在同类型内优先。
    pub(super) fn class(&self) -> usize {
        if self.policy == Policy::Reject {
            return 0;
        }
        if matches!(self.matcher, Matcher::Keyword(_)) {
            return if self.layer == Layer::Local { 9 } else { 10 };
        }
        let base = if self.layer == Layer::Local { 1 } else { 5 };
        base + match self.matcher {
            Matcher::Network(_) => 0,
            Matcher::Domain(_, false) => 1,
            Matcher::Domain(_, true) => 2,
            _ => 3,
        }
    }
}
pub struct Snapshot {
    instance: u64,
    document: Document,
    entries: Vec<Entry>,
    index: Index,
    _memory: ByteLease,
}
fn normalized(rule: &Rule, default: &Option<String>) -> Result<Rule> {
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
        Matcher::Transport(_) => rule.value.clone(),
    };
    Ok(Rule {
        r#type: rule.r#type,
        value,
        policy: rule.policy,
        outbound: policy_outbound(rule.policy, rule.outbound.clone(), default)?,
    })
}
fn key(rule: &Rule) -> (Match, String) {
    (rule.r#type, rule.value.clone())
}
fn charge(sources: &[Source]) -> Result<usize> {
    let mut count = 0usize;
    let mut bytes = 6 * 1024 * 1024usize;
    for source in sources {
        count = count
            .checked_add(source.rules.len())
            .ok_or(Error::ResourceLimit)?;
        if count > MAX_RULES {
            return Err(Error::ResourceLimit);
        }
        for rule in &source.rules {
            if rule.value.len() > 253 || rule.outbound.as_ref().is_some_and(|id| id.len() > 64) {
                return Err(Error::InvalidInput);
            }
            // 计入输入和输出字符串、哈希容量、来源、诊断、双树节点、排序事件和活动集合。
            // 标签最坏情况单独核算，防止大量单字符标签隐性放大 Trie。
            let labels = if matches!(rule.r#type, Match::DomainSuffix) {
                rule.value.split('.').count()
            } else {
                0
            };
            bytes = bytes
                .checked_add(
                    1024 + rule.value.len()
                        * if rule.r#type == Match::DomainKeyword {
                            64
                        } else {
                            4
                        }
                        + labels * 256,
                )
                .ok_or(Error::ResourceLimit)?;
        }
    }
    Ok(bytes)
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
        let mut exact = HashMap::<(Match, String, u8, Option<String>), usize>::new();
        let mut conditions = HashMap::<(u8, Match, String), usize>::new();
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
                    let rule = normalized(input, &document.settings.outbound)?;
                    let (kind, value) = key(&rule);
                    let action = match rule.policy {
                        Policy::Direct => 0,
                        Policy::Reject => 1,
                        Policy::Proxy => 2,
                    };
                    let full_key = (kind, value.clone(), action, rule.outbound.clone());
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
                        layer,
                        policy: rule.policy,
                        outbound: rule.outbound.clone(),
                    };
                    if rule.policy != Policy::Reject {
                        let level = u8::from(layer == Layer::Remote);
                        let condition = (level, kind, value.clone());
                        let same = conditions.get(&condition).copied();
                        let covered = coverage.coverage(&entry.matcher);
                        let winner = same.or_else(|| {
                            // 只裁剪同类包含关系；绝不由 DNS 的偶然结果裁剪跨 IP/域名规则。
                            let class = entry.class();
                            covered.get(class).or_else(|| {
                                if layer == Layer::Remote {
                                    covered.get(if class == 10 { 9 } else { class - 4 })
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
        document.report.effective = entries.len();
        let index = Index::compile(&entries)?;
        Ok(Self {
            instance: crate::lifecycle::identifier().ok_or(Error::ResourceLimit)?,
            document,
            entries,
            index,
            _memory: memory,
        })
    }
    /// 标准 YAML 生成产物；不是允许用户直接编辑的原始配置。
    pub fn to_yaml(&self) -> Result<String> {
        let _temporary = MEMORY.reserve(self.document.report.original * 1024 + 4096)?;
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
    pub(super) fn verify_yaml(&self, input: &str) -> Result<()> {
        let _temporary =
            MEMORY.reserve(input.len().checked_mul(8).ok_or(Error::ResourceLimit)? + 4096)?;
        let document: Document = yaml::decode(input)?;
        // 逐字段比较序列化流，避免同时创建两棵大型 JSON 值树。
        let original = serde_json::to_vec(&self.document).map_err(|_| Error::Internal)?;
        let restored = serde_json::to_vec(&document).map_err(|_| Error::Internal)?;
        if original != restored {
            return Err(Error::InvalidInput);
        }
        Ok(())
    }
    /// 读取文件只发生于加载或更新阶段，高频查询只访问已编译索引。
    pub fn from_yaml(input: &str) -> Result<Self> {
        if input.len() > yaml::MAX_BYTES {
            return Err(Error::ResourceLimit);
        }
        let temporary =
            MEMORY.reserve(input.len().checked_mul(8).ok_or(Error::ResourceLimit)? + 4096)?;
        let document: Document = yaml::decode(input)?;
        if document.format_version != 1
            || document.version == 0
            || document.sources.len() > MAX_SOURCES
            || document.rules.len() > MAX_RULES
            || document.report.original > MAX_RULES
            || document.report.removals.len() > MAX_RULES
            || document.report.duplicates > MAX_RULES
            || document.report.conflicts > MAX_RULES
            || document.report.pruned > MAX_RULES
        {
            return Err(Error::InvalidInput);
        }
        validate_settings(&document.settings)?;
        let mut sources: Vec<Source> = document
            .sources
            .iter()
            .map(|s| Source {
                id: s.id.clone(),
                layer: s.layer,
                enabled: true,
                rules: Vec::new(),
            })
            .collect();
        let mut ids = HashSet::new();
        for source in &sources {
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
            let source = sources
                .get_mut(usize::from(effective.origin.source))
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
            let normalized = normalized(&effective.rule, &document.settings.outbound)?;
            if normalized.value != effective.rule.value
                || normalized.outbound != effective.rule.outbound
            {
                return Err(Error::InvalidInput);
            }
            let source = &mut sources[usize::from(effective.origin.source)];
            entries.push(Entry {
                matcher: matcher(normalized.r#type, &normalized.value)?,
                layer: source.layer,
                policy: normalized.policy,
                outbound: normalized.outbound.clone(),
            });
            source.rules.push(normalized);
        }
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
        let duplicates: HashSet<_> = document
            .rules
            .iter()
            .flat_map(|row| row.additional_origins.iter())
            .map(|origin| (origin.source, origin.sequence))
            .collect();
        let mut removed = HashSet::new();
        let mut counts = [0usize; 3];
        for removal in &report.removals {
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
                    if !duplicates.contains(&origin) {
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
        let memory = MEMORY.reserve(charge(&sources)? + report.removals.len() * 128)?;
        let index = Index::compile(&entries)?;
        drop(temporary);
        Ok(Self {
            instance: crate::lifecycle::identifier().ok_or(Error::ResourceLimit)?,
            document,
            entries,
            index,
            _memory: memory,
        })
    }
    pub fn document(&self) -> &Document {
        &self.document
    }
    fn metadata(target: &Target) -> Result<(Option<String>, Option<IpAddr>)> {
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
    fn selected(values: Candidates) -> Option<usize> {
        if let Some(reject) = values.get(0) {
            return Some(reject);
        }
        // 索引已在域名命中时跳过 IP；各分支内部类型先于来源。
        let primary = [1, 5, 2, 6, 3, 7]
            .into_iter()
            .find_map(|class| values.get(class));
        // 保留端口/传输规则的来源和原序号语义，关键词仍在所有普通规则之后。
        let secondary = values.get(4).or_else(|| values.get(8));
        let winner = match (primary, secondary) {
            (Some(a), Some(b)) => {
                let a_local = [1, 2, 3]
                    .into_iter()
                    .any(|class| values.get(class) == Some(a));
                let b_local = values.get(4) == Some(b);
                Some(if a_local == b_local {
                    a.min(b)
                } else if b_local {
                    b
                } else {
                    a
                })
            }
            (a, b) => a.or(b),
        };
        winner.or_else(|| values.get(9)).or_else(|| values.get(10))
    }

    fn decide(&self, winner: Option<usize>) -> Decision {
        let settings = &self.document.settings;
        if let Some(id) = winner
            .filter(|id| self.entries[*id].policy == Policy::Reject || settings.mode == Mode::Rules)
        {
            let entry = &self.entries[id];
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
        Ok(self.decide(Self::selected(self.index.query(
            name.as_deref(),
            address,
            target.port,
            target.transport,
        ))))
    }
    /// 域名命中直接决定动作；未命中时等待真实 DNS 后查询 IP。
    pub fn evaluate_before_dns(&self, target: &Target) -> Result<Option<Decision>> {
        let (name, address) = Self::metadata(target)?;
        let values = self
            .index
            .query(name.as_deref(), address, target.port, target.transport);
        if let Some(id) = values.get(0) {
            return Ok(Some(self.decide(Some(id))));
        }
        if name
            .as_deref()
            .is_some_and(|name| self.index.domain_candidates(name).any())
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
        // 独立线性预扫描确定域名是否命中；命中时不评估任何 IP 规则。
        let domain_hit = self.entries.iter().any(|entry| match &entry.matcher {
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
        let mut classes: [Option<usize>; 10] = [None; 10];
        for (id, entry) in self.entries.iter().enumerate() {
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
            };
            if !matches {
                continue;
            }
            if entry.policy == Policy::Reject {
                reject.get_or_insert(id);
                continue;
            }
            if matches!(entry.matcher, Matcher::Keyword(_)) {
                classes[if entry.layer == Layer::Local { 8 } else { 9 }].get_or_insert(id);
                continue;
            }
            let source = if entry.layer == Layer::Local { 0 } else { 4 };
            let kind = match entry.matcher {
                Matcher::Network(_) => 0,
                Matcher::Domain(_, false) => 1,
                Matcher::Domain(_, true) => 2,
                _ => 3,
            };
            classes[source + kind].get_or_insert(id);
        }
        if let Some(reject) = reject {
            return Ok(self.decide(Some(reject)));
        }
        // 独立参考实现显式遍历类型及来源，不复用索引选择函数。
        let mut primary = None;
        for kind in 0..3 {
            for source in [0, 4] {
                if let Some(id) = classes[source + kind] {
                    primary = Some(id);
                    break;
                }
            }
            if primary.is_some() {
                break;
            }
        }
        let secondary = classes[3].or(classes[7]);
        let picked = match (primary, secondary) {
            (Some(a), Some(b)) => Some(if self.entries[a].layer == self.entries[b].layer {
                if a < b { a } else { b }
            } else if self.entries[b].layer == Layer::Local {
                b
            } else {
                a
            }),
            (a, b) => a.or(b),
        };
        if picked.is_some() {
            return Ok(self.decide(picked));
        }

        Ok(self.decide(classes[8].or(classes[9])))
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
    if values.len() >= 128 && !values.contains_key(&snapshot.instance) {
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
                    let rule = normalized(input, &document.settings.outbound)?;
                    entries.push(Entry {
                        matcher: matcher(rule.r#type, &rule.value)?,
                        layer,
                        policy: rule.policy,
                        outbound: rule.outbound.clone(),
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
        Ok(Self(Snapshot {
            instance: crate::lifecycle::identifier().ok_or(Error::ResourceLimit)?,
            document,
            entries,
            index: Index::default(),
            _memory: memory,
        }))
    }
    pub fn evaluate(&self, target: &Target) -> Result<Decision> {
        self.0.evaluate_reference(target)
    }
}
