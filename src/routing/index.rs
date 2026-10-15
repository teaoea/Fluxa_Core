//! 不可变索引返回各优先级最早候选，不采用最长前缀或最具体后缀替代原始顺序。
use super::super::{Matcher, Transport};
use super::{Entry, RuleClass};
use crate::error::{Error, Result};
use aho_corasick::{AhoCorasick, AhoCorasickKind};
use std::collections::{BTreeSet, HashMap};
use std::net::IpAddr;

pub(super) const CLASSES: usize = RuleClass::COUNT;
pub(super) const NONE: u32 = u32::MAX;
#[derive(Clone, Copy)]
pub(super) struct Candidates(pub [u32; CLASSES]);
impl Default for Candidates {
    fn default() -> Self {
        Self([NONE; CLASSES])
    }
}
impl Candidates {
    /// 先排除另一个协议的所有组合候选，再判断域名是否命中、是否需要 DNS。
    pub fn for_transport(mut self, transport: Transport) -> Self {
        let excluded = match transport {
            Transport::Tcp => [
                RuleClass::RejectUdp,
                RuleClass::LocalDomainUdp,
                RuleClass::RemoteDomainUdp,
                RuleClass::LocalSuffixUdp,
                RuleClass::RemoteSuffixUdp,
            ],
            Transport::Udp => [
                RuleClass::RejectTcp,
                RuleClass::LocalDomainTcp,
                RuleClass::RemoteDomainTcp,
                RuleClass::LocalSuffixTcp,
                RuleClass::RemoteSuffixTcp,
            ],
        };
        for class in excluded {
            self.0[class as usize] = NONE;
        }
        self
    }
    pub fn add(&mut self, class: usize, id: u32) {
        self.0[class] = self.0[class].min(id);
    }
    pub fn any(self) -> bool {
        self.0.iter().any(|value| *value != NONE)
    }
    pub fn merge(&mut self, other: Self) {
        for (value, candidate) in self.0.iter_mut().zip(other.0) {
            *value = (*value).min(candidate);
        }
    }
    pub fn get(self, class: usize) -> Option<usize> {
        (self.0[class] != NONE).then_some(self.0[class] as usize)
    }
}
#[derive(Default)]
pub(super) struct DomainNode {
    pub(super) children: HashMap<String, u32>,
    pub(super) values: Candidates,
}
#[derive(Default)]
pub(super) struct Domains {
    pub(super) nodes: Vec<DomainNode>,
}
impl Domains {
    fn insert(&mut self, name: &str, class: usize, id: u32) {
        if self.nodes.is_empty() {
            self.nodes.push(DomainNode::default());
        }
        let mut node = 0;
        for label in name.rsplit('.') {
            let child = match self.nodes[node].children.get(label) {
                Some(child) => *child,
                None => {
                    let child = self.nodes.len() as u32;
                    self.nodes.push(DomainNode::default());
                    self.nodes[node].children.insert(label.to_owned(), child);
                    child
                }
            };
            node = child as usize;
        }
        self.nodes[node].values.add(class, id);
    }
    fn query(&self, name: &str) -> Candidates {
        let mut result = Candidates::default();
        if self.nodes.is_empty() {
            return result;
        }
        let mut node = 0;
        for label in name.rsplit('.') {
            let Some(child) = self.nodes[node].children.get(label) else {
                break;
            };
            node = *child as usize;
            result.merge(self.nodes[node].values);
        }
        result
    }
}
pub(super) struct PrefixNode {
    pub(super) key: u128,
    pub(super) length: u8,
    pub(super) values: Candidates,
    pub(super) children: [u32; 2],
}
pub(super) struct Prefixes {
    pub(super) nodes: Vec<PrefixNode>,
    pub(super) root: u32,
    pub(super) width: u8,
}
fn mask(length: u8, width: u8) -> u128 {
    if length == 0 {
        0
    } else {
        u128::MAX << (width - length)
    }
}
fn branch(key: u128, length: u8, width: u8) -> usize {
    ((key >> (width - length - 1)) & 1) as usize
}
impl Prefixes {
    fn new(width: u8) -> Self {
        Self {
            nodes: Vec::new(),
            root: NONE,
            width,
        }
    }
    fn node(&mut self, key: u128, length: u8, values: Candidates) -> u32 {
        let id = self.nodes.len() as u32;
        self.nodes.push(PrefixNode {
            key,
            length,
            values,
            children: [NONE; 2],
        });
        id
    }
    fn insert_at(&mut self, current: u32, key: u128, length: u8, values: Candidates) -> u32 {
        if current == NONE {
            return self.node(key, length, values);
        }
        let old = &self.nodes[current as usize];
        let differing = key ^ old.key;
        let common = (differing.leading_zeros() as u8)
            .saturating_sub(128 - self.width)
            .min(length)
            .min(old.length);
        if common < old.length {
            let parent = self.node(
                key & mask(common, self.width),
                common,
                Candidates::default(),
            );
            let old_branch = branch(self.nodes[current as usize].key, common, self.width);
            self.nodes[parent as usize].children[old_branch] = current;
            if common == length {
                self.nodes[parent as usize].values.merge(values);
            } else {
                let child = self.node(key, length, values);
                self.nodes[parent as usize].children[1 - old_branch] = child;
            }
            return parent;
        }
        if length == old.length {
            self.nodes[current as usize].values.merge(values);
            return current;
        }
        let side = branch(key, old.length, self.width);
        let child = self.nodes[current as usize].children[side];
        let child = self.insert_at(child, key, length, values);
        self.nodes[current as usize].children[side] = child;
        current
    }
    fn insert(&mut self, key: u128, length: u8, class: usize, id: u32) {
        let mut values = Candidates::default();
        values.add(class, id);
        self.root = self.insert_at(self.root, key, length, values);
    }
    fn query(&self, key: u128, max_length: u8) -> Candidates {
        let mut result = Candidates::default();
        let mut current = self.root;
        while current != NONE {
            let node = &self.nodes[current as usize];
            if node.length > max_length || (key & mask(node.length, self.width)) != node.key {
                break;
            }
            result.merge(node.values);
            if node.length == max_length {
                break;
            }
            current = node.children[branch(key, node.length, self.width)];
        }
        result
    }
}
#[derive(Default)]
pub(super) struct Ports {
    pub(super) starts: Vec<u32>,
    pub(super) values: Vec<Candidates>,
}
impl Ports {
    fn compile(entries: &[Entry]) -> Self {
        let mut events = Vec::new();
        for (id, entry) in entries.iter().enumerate() {
            if let Matcher::Port(first, last) = entry.matcher {
                events.push((u32::from(first), true, entry.class() as usize, id as u32));
                events.push((
                    u32::from(last) + 1,
                    false,
                    entry.class() as usize,
                    id as u32,
                ));
            }
        }
        // 仅按端口边界排序；类别编号只是集合槽位，不参与优先级比较。
        events.sort_unstable_by_key(|event| event.0);
        let mut active: [BTreeSet<u32>; CLASSES] = std::array::from_fn(|_| BTreeSet::new());
        let mut result = Self::default();
        let mut offset = 0;
        while offset < events.len() {
            let start = events[offset].0;
            while offset < events.len() && events[offset].0 == start {
                let (_, add, class, id) = events[offset];
                if add {
                    active[class].insert(id);
                } else {
                    active[class].remove(&id);
                }
                offset += 1;
            }
            let values = Candidates(std::array::from_fn(|class| {
                active[class].first().copied().unwrap_or(NONE)
            }));
            result.starts.push(start);
            result.values.push(values);
        }
        result
    }
    fn query(&self, port: u16) -> Candidates {
        let end = self
            .starts
            .partition_point(|start| *start <= u32::from(port));
        if end == 0 {
            Candidates::default()
        } else {
            self.values[end - 1]
        }
    }
}
#[derive(Clone, Copy, Default)]
struct Cover {
    end: u16,
    id: u32,
}
struct PortCoverage {
    tree: Vec<[Cover; CLASSES]>,
}
impl PortCoverage {
    fn new() -> Self {
        Self {
            tree: vec![[Cover::default(); CLASSES]; 65536],
        }
    }
    fn insert(&mut self, first: u16, last: u16, class: usize, id: u32) {
        let mut position = usize::from(first);
        while position < self.tree.len() {
            let current = &mut self.tree[position][class];
            if last > current.end || (last == current.end && id < current.id) {
                *current = Cover { end: last, id };
            }
            position += position & position.wrapping_neg();
        }
    }
    fn query(&self, first: u16, last: u16) -> Candidates {
        let mut result = Candidates::default();
        let mut position = usize::from(first);
        while position > 0 {
            for class in 0..CLASSES {
                let value = self.tree[position][class];
                if value.end >= last {
                    result.add(class, value.id);
                }
            }
            position &= position - 1;
        }
        result
    }
}
pub(super) struct Index {
    keyword_exact: HashMap<String, Candidates>,
    keywords: Option<(AhoCorasick, Vec<Candidates>)>,
    port_coverage: Option<PortCoverage>,
    pub(super) exact: HashMap<String, Candidates>,
    pub(super) suffix: Domains,
    pub(super) v4: Prefixes,
    pub(super) v6: Prefixes,
    pub(super) ports: Ports,
    pub(super) transports: [Candidates; 2],
}
impl Default for Index {
    fn default() -> Self {
        Self {
            keyword_exact: HashMap::new(),
            keywords: None,
            port_coverage: None,
            exact: HashMap::new(),
            suffix: Domains::default(),
            v4: Prefixes::new(32),
            v6: Prefixes::new(128),
            ports: Ports::default(),
            transports: [Candidates::default(); 2],
        }
    }
}
impl Index {
    pub fn insert(&mut self, entry: &Entry, id: u32) {
        let class = entry.class() as usize;
        match &entry.matcher {
            Matcher::Quic => {}
            Matcher::Keyword(value) => self
                .keyword_exact
                .entry(value.clone())
                .or_default()
                .add(class, id),
            Matcher::Domain(value, false) => {
                self.exact.entry(value.clone()).or_default().add(class, id)
            }
            Matcher::Domain(value, true) => self.suffix.insert(value, class, id),
            Matcher::Network(network) => {
                let (key, prefix, v4) = network.key();
                if v4 {
                    self.v4.insert(key, prefix, class, id);
                } else {
                    self.v6.insert(key, prefix, class, id);
                }
            }
            Matcher::Transport(value) => {
                self.transports[usize::from(*value == Transport::Udp)].add(class, id)
            }
            Matcher::Port(first, last) => self
                .port_coverage
                .get_or_insert_with(PortCoverage::new)
                .insert(*first, *last, class, id),
        }
    }
    pub fn compile(entries: &[Entry]) -> Result<Self> {
        let mut result = Self::default();
        for (id, entry) in entries.iter().enumerate() {
            result.insert(entry, id as u32);
        }
        result.ports = Ports::compile(entries);
        result.port_coverage = None;
        if !result.keyword_exact.is_empty() {
            let patterns: Vec<_> = result.keyword_exact.keys().map(String::as_str).collect();
            let values = patterns
                .iter()
                .map(|pattern| result.keyword_exact[*pattern])
                .collect();
            let engine = AhoCorasick::builder()
                .kind(Some(AhoCorasickKind::ContiguousNFA))
                .build(patterns)
                .map_err(|_| Error::ResourceLimit)?;
            result.keywords = Some((engine, values));
            result.keyword_exact.clear();
            result.keyword_exact.shrink_to_fit();
        }
        Ok(result)
    }
    /// 覆盖查询只接受整个条件被包含；后缀条件不能由精确域名覆盖。
    pub fn coverage(&self, matcher: &Matcher) -> Candidates {
        match matcher {
            Matcher::Quic => Candidates::default(),
            Matcher::Keyword(value) => self.keyword_exact.get(value).copied().unwrap_or_default(),
            Matcher::Domain(value, suffix) => {
                let mut values = self.suffix.query(value);
                if !suffix {
                    values.merge(self.exact.get(value).copied().unwrap_or_default());
                }
                values
            }
            Matcher::Network(network) => {
                let (key, prefix, v4) = network.key();
                if v4 {
                    self.v4.query(key, prefix)
                } else {
                    self.v6.query(key, prefix)
                }
            }
            Matcher::Transport(value) => self.transports[usize::from(*value == Transport::Udp)],
            Matcher::Port(first, last) => self
                .port_coverage
                .as_ref()
                .map_or_else(Candidates::default, |index| index.query(*first, *last)),
        }
    }
    pub fn domain_candidates(&self, name: &str) -> Candidates {
        let mut values = self.suffix.query(name);
        values.merge(self.exact.get(name).copied().unwrap_or_default());
        if let Some((engine, candidates)) = &self.keywords {
            for found in engine.find_overlapping_iter(name) {
                values.merge(candidates[found.pattern().as_usize()]);
            }
        }
        values
    }
    pub fn query(
        &self,
        name: Option<&str>,
        ip: Option<IpAddr>,
        port: u16,
        transport: Transport,
    ) -> Candidates {
        let mut values = self.ports.query(port);
        values.merge(self.transports[usize::from(transport == Transport::Udp)]);
        if let Some(name) = name {
            let domains = self.domain_candidates(name).for_transport(transport);
            let hit = domains.0.iter().any(|value| *value != NONE);
            values.merge(domains);
            // 用户确定域名命中后跳过所有 IP 规则，包括 IP 拦截。
            if hit {
                return values;
            }
        }
        if let Some(ip) = ip {
            values.merge(match ip {
                IpAddr::V4(ip) => self.v4.query(u128::from(u32::from(ip)), 32),
                IpAddr::V6(ip) => self.v6.query(u128::from(ip), 128),
            });
        }
        values
    }
}

// 二进制索引编码留在索引模块，私有树节点不向业务调用者暴露。
impl Index {
    pub(super) fn flatten(&self, entries: &[Entry]) -> Result<super::flat::FlatIndex> {
        super::flat::Builder::compile(self, entries)
    }
}
