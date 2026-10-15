//! 编译索引的平铺表示：固定小端记录、相对偏移和字符串池；查询直接借用字节切片。
//! 不把文件字节转换为 Rust 裸指针或本机结构体，不依赖对齐、主机字节序或 unsafe。
use super::index::{Candidates, Index, NONE};
use super::{Entry, MAX_RULES, RuleClass};
use crate::{
    budget::ByteLease,
    error::{Error, Result},
    routing::{Matcher, Transport},
};
use std::{
    collections::{BTreeMap, VecDeque},
    net::IpAddr,
};

// 在临时树/队列/序列化区段分配之前预留；持久快照缓冲区由 runtime 单独持有预算。
use super::MEMORY as BUILD_MEMORY;
const MAX_BUILD_BYTES: usize = 256 * 1024 * 1024;
const BUILD_BASE_BYTES: usize = 4096;
const BUILD_RULE_BYTES: usize = 768;
const BUILD_KEYWORD_BYTE_FACTOR: usize = 384;
const BUILD_SUFFIX_LABEL_BYTES: usize = 256;
const WORD: usize = 4;
const PREFIX_KEY_BYTES: usize = 16;
const PREFIX_LENGTH_OFFSET: usize = PREFIX_KEY_BYTES;
const PREFIX_CHILDREN_OFFSET: usize = PREFIX_LENGTH_OFFSET + WORD;
const PREFIX_VALUES_OFFSET: usize = PREFIX_CHILDREN_OFFSET + 2 * WORD;
const FNV_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
const FNV_PRIME: u64 = 0x100000001b3;
// 固定哈希只用于内部索引，限制探测链以拒绝碰撞堆积，不让查询退化到遍历十万槽。
const MAX_EXACT_PROBES: usize = 64;
const BYTE_MASK_WORDS: usize = 4;
const MASK_WORD_BITS: usize = 64;
const CANDIDATE_BYTES: usize = RuleClass::COUNT * WORD;
const SECTION_COUNT: usize = 10;
const DIRECTORY_BYTES: usize = SECTION_COUNT * 2 * WORD;
const HEADER_BYTES: usize = DIRECTORY_BYTES + 2 * WORD;
const EXACT_BYTES: usize = 2 * WORD + CANDIDATE_BYTES;
const NODE_BYTES: usize = 2 * WORD + CANDIDATE_BYTES;
const EDGE_BYTES: usize = 3 * WORD;
const PREFIX_BYTES: usize = PREFIX_VALUES_OFFSET + CANDIDATE_BYTES;
const PORT_BYTES: usize = WORD + CANDIDATE_BYTES;
const KEYWORD_NODE_BYTES: usize = 4 * WORD + CANDIDATE_BYTES;
const KEYWORD_EDGE_BYTES: usize = 2 * WORD;
const HAS_OUTPUT: u32 = 1 << 31;
const HASH_EDGES: u32 = 1 << 31;
const SUFFIX_HAS_OUTPUT: u32 = 1 << 30;
const EDGE_COUNT_MASK: u32 = SUFFIX_HAS_OUTPUT - 1;
const HASH_EDGE_THRESHOLD: usize = 8;
const DEPTH_MASK: u32 = HAS_OUTPUT - 1;
// 节编号仅描述文件区段位置，不参与规则优先级。
#[repr(usize)]
#[derive(Clone, Copy)]
enum Section {
    Exact,
    SuffixNodes,
    SuffixEdges,
    V4,
    V6,
    Ports,
    KeywordNodes,
    KeywordEdges,
    Transports,
    Strings,
}
const SECTIONS: [Section; SECTION_COUNT] = [
    Section::Exact,
    Section::SuffixNodes,
    Section::SuffixEdges,
    Section::V4,
    Section::V6,
    Section::Ports,
    Section::KeywordNodes,
    Section::KeywordEdges,
    Section::Transports,
    Section::Strings,
];
const WIDTHS: [usize; SECTION_COUNT] = [
    EXACT_BYTES,
    NODE_BYTES,
    EDGE_BYTES,
    PREFIX_BYTES,
    PREFIX_BYTES,
    PORT_BYTES,
    KEYWORD_NODE_BYTES,
    KEYWORD_EDGE_BYTES,
    CANDIDATE_BYTES,
    1,
];
fn invalid() -> Error {
    Error::InvalidInput
}
fn word(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(
        bytes[offset..offset + WORD]
            .try_into()
            .expect("已校验的固定宽度记录"),
    )
}
fn push(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}
fn count(value: usize) -> Result<u32> {
    value.try_into().map_err(|_| Error::ResourceLimit)
}
fn candidates(bytes: &[u8], offset: usize) -> Candidates {
    Candidates(std::array::from_fn(|slot| {
        word(bytes, offset + slot * WORD)
    }))
}
fn put_candidates(out: &mut Vec<u8>, values: Candidates) {
    for value in values.0 {
        push(out, value);
    }
}
fn hash(value: &str) -> usize {
    value.bytes().fold(FNV_OFFSET_BASIS, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(FNV_PRIME)
    }) as usize
}

/// 一次文件读取后直接查询该缓冲区，不解码为哈希表、Trie 节点或自动机对象。
/// 这里的零拷贝指索引解析；文件到用户缓冲区仍有一次系统读取复制。
pub(super) struct FlatIndex {
    bytes: Vec<u8>,
    _memory: Option<ByteLease>,
    ranges: [(usize, usize); SECTION_COUNT],
    roots: [u32; 2],
    keyword_first_bytes: [u64; BYTE_MASK_WORDS],
    keyword_single_first_byte: Option<u8>,
    keyword_has_patterns: bool,
}
impl FlatIndex {
    fn section(&self, section: Section) -> &[u8] {
        let (start, end) = self.ranges[section as usize];
        &self.bytes[start..end]
    }
    fn string_bytes(&self, record: &[u8], offset: usize) -> &[u8] {
        let start = word(record, offset) as usize;
        let length = word(record, offset + WORD) as usize;
        &self.section(Section::Strings)[start..start + length]
    }
    // 只在加载验证时解码 UTF-8；高频比较借用字节，不重复校验已验证的不可变字符串池。
    fn string(&self, record: &[u8], offset: usize) -> &str {
        std::str::from_utf8(self.string_bytes(record, offset)).expect("字符串池已经验证 UTF-8")
    }
    pub(super) fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub(super) fn parse(bytes: Vec<u8>, entries: &[Entry]) -> Result<Self> {
        Self::parse_owned(bytes, entries, None)
    }
    fn parse_owned(bytes: Vec<u8>, entries: &[Entry], memory: Option<ByteLease>) -> Result<Self> {
        if bytes.len() < HEADER_BYTES {
            return Err(invalid());
        }
        let mut ranges = [(0, 0); SECTION_COUNT];
        let mut end = HEADER_BYTES;
        for (slot, width) in WIDTHS.into_iter().enumerate() {
            let start = word(&bytes, slot * 2 * WORD) as usize;
            let amount = word(&bytes, slot * 2 * WORD + WORD) as usize;
            let next = start
                .checked_add(amount.checked_mul(width).ok_or_else(invalid)?)
                .ok_or_else(invalid)?;
            if start != end || next > bytes.len() {
                return Err(invalid());
            }
            ranges[slot] = (start, next);
            end = next;
        }
        if end != bytes.len() {
            return Err(invalid());
        }
        let roots = [
            word(&bytes, DIRECTORY_BYTES),
            word(&bytes, DIRECTORY_BYTES + WORD),
        ];
        let mut result = Self {
            bytes,
            _memory: memory,
            ranges,
            roots,
            keyword_first_bytes: [0; BYTE_MASK_WORDS],
            keyword_single_first_byte: None,
            keyword_has_patterns: false,
        };
        result.validate(entries)?;
        let nodes = result.section(Section::KeywordNodes);
        let start = word(nodes, 0) as usize;
        let amount = word(nodes, WORD) as usize;
        let edges = result.section(Section::KeywordEdges);
        let mut mask = [0u64; BYTE_MASK_WORDS];
        for edge in edges[start * KEYWORD_EDGE_BYTES..(start + amount) * KEYWORD_EDGE_BYTES]
            .as_chunks::<KEYWORD_EDGE_BYTES>()
            .0
        {
            let byte = word(edge, 0) as usize;
            mask[byte / MASK_WORD_BITS] |= 1 << (byte % MASK_WORD_BITS);
        }
        if mask.iter().map(|word| word.count_ones()).sum::<u32>() == 1 {
            result.keyword_single_first_byte = mask.iter().enumerate().find_map(|(slot, bits)| {
                (*bits != 0).then(|| (slot * MASK_WORD_BITS + bits.trailing_zeros() as usize) as u8)
            });
        }
        result.keyword_has_patterns = mask.iter().any(|bits| *bits != 0);
        result.keyword_first_bytes = mask;
        Ok(result)
    }
    fn validate_candidates(&self, bytes: &[u8], offset: usize, entries: &[Entry]) -> Result<()> {
        for (slot, id) in candidates(bytes, offset).0.into_iter().enumerate() {
            if id != NONE
                && entries
                    .get(id as usize)
                    .is_none_or(|entry| entry.class() as usize != slot)
            {
                return Err(invalid());
            }
        }
        Ok(())
    }
    fn validate_string(&self, bytes: &[u8], offset: usize) -> Result<()> {
        let start = word(bytes, offset) as usize;
        let length = word(bytes, offset + WORD) as usize;
        let pool = self.section(Section::Strings);
        let value = pool
            .get(start..start.checked_add(length).ok_or_else(invalid)?)
            .ok_or_else(invalid)?;
        if length > 253 || std::str::from_utf8(value).is_err() {
            return Err(invalid());
        }
        Ok(())
    }
    fn validate(&self, entries: &[Entry]) -> Result<()> {
        if entries.len() > MAX_RULES {
            return Err(Error::ResourceLimit);
        }
        let exact = self.section(Section::Exact);
        let slots = exact.len() / EXACT_BYTES;
        if slots == 0 || !slots.is_power_of_two() || slots > (MAX_RULES * 2).next_power_of_two() {
            return Err(invalid());
        }
        let mut empty = 0;
        for row in exact.as_chunks::<EXACT_BYTES>().0.iter() {
            if word(row, WORD) == 0 {
                empty += 1;
                if candidates(row, 2 * WORD).any() {
                    return Err(invalid());
                }
            }
            self.validate_string(row, 0)?;
            self.validate_candidates(row, 2 * WORD, entries)?;
        }
        if empty == 0 {
            return Err(invalid());
        }
        for (ordinal, row) in exact.as_chunks::<EXACT_BYTES>().0.iter().enumerate() {
            if word(row, WORD) == 0 {
                continue;
            }
            let name = self.string(row, 0);
            let mut slot = hash(name) & (slots - 1);
            let mut found = false;
            for _ in 0..MAX_EXACT_PROBES.min(slots) {
                let candidate = &exact[slot * EXACT_BYTES..];
                if word(candidate, WORD) == 0 {
                    break;
                }
                if self.string(candidate, 0) == name {
                    found = slot == ordinal;
                    break;
                }
                slot = (slot + 1) & (slots - 1);
            }
            if !found {
                return Err(invalid());
            }
        }
        let nodes = self.section(Section::SuffixNodes);
        let edges = self.section(Section::SuffixEdges);
        let mut expected_edge = 0usize;
        for (id, node) in nodes.as_chunks::<NODE_BYTES>().0.iter().enumerate() {
            self.validate_candidates(node, 2 * WORD, entries)?;
            if (word(node, WORD) & SUFFIX_HAS_OUTPUT != 0) != candidates(node, 2 * WORD).any() {
                return Err(invalid());
            }
            let start = word(node, 0) as usize;
            let hashed = word(node, WORD) & HASH_EDGES != 0;
            let length = (word(node, WORD) & EDGE_COUNT_MASK) as usize;
            if hashed && (length == 0 || !length.is_power_of_two()) {
                return Err(invalid());
            }
            if start != expected_edge {
                return Err(invalid());
            }
            let end = start.checked_add(length).ok_or_else(invalid)?;
            let slice = edges
                .get(
                    start.checked_mul(EDGE_BYTES).ok_or_else(invalid)?
                        ..end.checked_mul(EDGE_BYTES).ok_or_else(invalid)?,
                )
                .ok_or_else(invalid)?;
            // 哈希探测可能先访问尚未轮到的槽；先校验整段字符串引用，再比较任何槽。
            for edge in slice.as_chunks::<EDGE_BYTES>().0 {
                self.validate_string(edge, 0)?;
            }
            let mut previous: Option<&str> = None;
            let mut empty = 0;
            for (ordinal, edge) in slice.as_chunks::<EDGE_BYTES>().0.iter().enumerate() {
                let label = self.string(edge, 0);
                let child = word(edge, 2 * WORD) as usize;
                if hashed && child == NONE as usize {
                    if !label.is_empty() {
                        return Err(invalid());
                    }
                    empty += 1;
                    continue;
                }
                if label.is_empty()
                    || (!hashed && previous.is_some_and(|value| value >= label))
                    || child <= id
                    || child >= nodes.len() / NODE_BYTES
                {
                    return Err(invalid());
                }
                if hashed {
                    let mut slot = hash(label) & (length - 1);
                    let mut found = false;
                    for _ in 0..MAX_EXACT_PROBES.min(length) {
                        let candidate = &slice[slot * EDGE_BYTES..];
                        if word(candidate, 2 * WORD) == NONE {
                            break;
                        }
                        if self.string(candidate, 0) == label {
                            found = slot == ordinal;
                            break;
                        }
                        slot = (slot + 1) & (length - 1);
                    }
                    if !found {
                        return Err(invalid());
                    }
                }
                previous = Some(label);
            }
            if hashed && empty == 0 {
                return Err(invalid());
            }
            expected_edge = end;
        }
        if expected_edge * EDGE_BYTES != edges.len() {
            return Err(invalid());
        }
        for (slot, section) in [Section::V4, Section::V6].into_iter().enumerate() {
            let prefixes = self.section(section);
            let width = if slot == 0 { 32 } else { 128 };
            if self.roots[slot] != NONE
                && self.roots[slot] as usize >= prefixes.len() / PREFIX_BYTES
            {
                return Err(invalid());
            }
            for row in prefixes.as_chunks::<PREFIX_BYTES>().0.iter() {
                let length = word(row, PREFIX_LENGTH_OFFSET);
                if length > width {
                    return Err(invalid());
                }
                self.validate_candidates(row, PREFIX_VALUES_OFFSET, entries)?;
                for side in 0..2 {
                    let child = word(row, PREFIX_CHILDREN_OFFSET + side * WORD);
                    if child != NONE {
                        if child as usize >= prefixes.len() / PREFIX_BYTES {
                            return Err(invalid());
                        }
                        let target = prefixes
                            .get(child as usize * PREFIX_BYTES..(child as usize + 1) * PREFIX_BYTES)
                            .ok_or_else(invalid)?;
                        if word(target, PREFIX_LENGTH_OFFSET) <= length {
                            return Err(invalid());
                        }
                    }
                }
            }
        }
        let mut previous = None;
        for row in self
            .section(Section::Ports)
            .as_chunks::<PORT_BYTES>()
            .0
            .iter()
        {
            let start = word(row, 0);
            if start > 65536 || previous.is_some_and(|p| p >= start) {
                return Err(invalid());
            }
            previous = Some(start);
            self.validate_candidates(row, WORD, entries)?;
        }
        let nodes = self.section(Section::KeywordNodes);
        let edges = self.section(Section::KeywordEdges);
        let mut expected = 0usize;
        for (id, node) in nodes.as_chunks::<KEYWORD_NODE_BYTES>().0.iter().enumerate() {
            let fail = word(node, 2 * WORD) as usize;
            let depth = word(node, 3 * WORD) & DEPTH_MASK;
            if depth > 253 || (id == 0 && (depth != 0 || fail != 0)) {
                return Err(invalid());
            }
            if id != 0
                && (fail >= nodes.len() / KEYWORD_NODE_BYTES
                    || word(&nodes[fail * KEYWORD_NODE_BYTES..], 3 * WORD) & DEPTH_MASK >= depth)
            {
                return Err(invalid());
            }
            self.validate_candidates(node, 4 * WORD, entries)?;
            if (word(node, 3 * WORD) & HAS_OUTPUT != 0) != candidates(node, 4 * WORD).any() {
                return Err(invalid());
            }
            let start = word(node, 0) as usize;
            let length = word(node, WORD) as usize;
            if start != expected {
                return Err(invalid());
            }
            let end = start.checked_add(length).ok_or_else(invalid)?;
            let slice = edges
                .get(
                    start.checked_mul(KEYWORD_EDGE_BYTES).ok_or_else(invalid)?
                        ..end.checked_mul(KEYWORD_EDGE_BYTES).ok_or_else(invalid)?,
                )
                .ok_or_else(invalid)?;
            let mut previous = None;
            for edge in slice.as_chunks::<KEYWORD_EDGE_BYTES>().0.iter() {
                let byte = word(edge, 0);
                let child = word(edge, WORD) as usize;
                if byte > 255
                    || previous.is_some_and(|p| p >= byte)
                    || child <= id
                    || child >= nodes.len() / KEYWORD_NODE_BYTES
                    || word(&nodes[child * KEYWORD_NODE_BYTES..], 3 * WORD) & DEPTH_MASK
                        != depth + 1
                {
                    return Err(invalid());
                }
                previous = Some(byte);
            }
            expected = end;
        }
        if expected * KEYWORD_EDGE_BYTES != edges.len() || nodes.is_empty() {
            return Err(invalid());
        }
        if self.section(Section::Transports).len() != 2 * CANDIDATE_BYTES {
            return Err(invalid());
        }
        for row in self
            .section(Section::Transports)
            .as_chunks::<CANDIDATE_BYTES>()
            .0
            .iter()
        {
            self.validate_candidates(row, 0, entries)?;
        }
        Ok(())
    }
    fn exact(&self, name: &str) -> Candidates {
        let bytes = self.section(Section::Exact);
        // 空精确域名表只有一个空槽，不必计算输入哈希。
        if bytes.len() == EXACT_BYTES {
            return Candidates::default();
        }
        let mask = bytes.len() / EXACT_BYTES - 1;
        let mut slot = hash(name) & mask;
        // 至少一个空槽已在加载时验证；探测数额外有界，避免损坏文件导致死循环。
        for _ in 0..MAX_EXACT_PROBES.min(mask + 1) {
            let row = &bytes[slot * EXACT_BYTES..(slot + 1) * EXACT_BYTES];
            if word(row, WORD) == 0 {
                break;
            }
            if self.string_bytes(row, 0) == name.as_bytes() {
                return candidates(row, 2 * WORD);
            }
            slot = (slot + 1) & mask;
        }
        Candidates::default()
    }
    fn suffix(&self, name: &str) -> Candidates {
        let nodes = self.section(Section::SuffixNodes);
        if nodes.is_empty() {
            return Candidates::default();
        }
        let edges = self.section(Section::SuffixEdges);
        let mut id = 0usize;
        let mut values = Candidates::default();
        for label in name.rsplit('.') {
            let node = &nodes[id * NODE_BYTES..];
            let start = word(node, 0) as usize;
            let amount = (word(node, WORD) & EDGE_COUNT_MASK) as usize;
            let edge = if word(node, WORD) & HASH_EDGES != 0 {
                let mut slot = hash(label) & (amount - 1);
                let mut found = None;
                for _ in 0..MAX_EXACT_PROBES.min(amount) {
                    let edge = &edges[(start + slot) * EDGE_BYTES..];
                    if word(edge, 2 * WORD) == NONE {
                        break;
                    }
                    if self.string_bytes(edge, 0) == label.as_bytes() {
                        found = Some(edge);
                        break;
                    }
                    slot = (slot + 1) & (amount - 1);
                }
                let Some(edge) = found else {
                    break;
                };
                edge
            } else {
                let mut low = start;
                let mut high = start + amount;
                while low < high {
                    let middle = (low + high) / 2;
                    let row = &edges[middle * EDGE_BYTES..];
                    if self.string_bytes(row, 0) < label.as_bytes() {
                        low = middle + 1;
                    } else {
                        high = middle;
                    }
                }
                if low == start + amount {
                    break;
                }
                let edge = &edges[low * EDGE_BYTES..];
                if self.string_bytes(edge, 0) != label.as_bytes() {
                    break;
                }
                edge
            };
            id = word(edge, 2 * WORD) as usize;
            let node = &nodes[id * NODE_BYTES..];
            if word(node, WORD) & SUFFIX_HAS_OUTPUT != 0 {
                values.merge(candidates(node, 2 * WORD));
            }
        }
        values
    }
    fn transition(&self, id: usize, byte: u8) -> Option<usize> {
        let nodes = self.section(Section::KeywordNodes);
        let edges = self.section(Section::KeywordEdges);
        let node = &nodes[id * KEYWORD_NODE_BYTES..];
        let start = word(node, 0) as usize;
        let amount = word(node, WORD) as usize;
        let mut low = start;
        let mut high = start + amount;
        while low < high {
            let middle = (low + high) / 2;
            if word(edges, middle * KEYWORD_EDGE_BYTES) < u32::from(byte) {
                low = middle + 1;
            } else {
                high = middle;
            }
        }
        (low < start + amount && word(edges, low * KEYWORD_EDGE_BYTES) == u32::from(byte))
            .then(|| word(edges, low * KEYWORD_EDGE_BYTES + WORD) as usize)
    }
    pub(super) fn domain_candidates(&self, name: &str) -> Candidates {
        let mut values = self.exact(name);
        values.merge(self.suffix(name));
        // 单首字节场景让标准库字节查找快速排除；不会遗漏重叠匹配或关键词拦截。
        if !self.keyword_has_patterns
            || self
                .keyword_single_first_byte
                .is_some_and(|first| !name.as_bytes().contains(&first))
        {
            return values;
        }
        let mut id = 0usize;
        let nodes = self.section(Section::KeywordNodes);
        for byte in name.bytes() {
            if id == 0
                && self.keyword_first_bytes[usize::from(byte) / MASK_WORD_BITS]
                    & (1 << (usize::from(byte) % MASK_WORD_BITS))
                    == 0
            {
                continue;
            }
            loop {
                if let Some(next) = self.transition(id, byte) {
                    id = next;
                    break;
                }
                if id == 0 {
                    break;
                }
                id = word(nodes, id * KEYWORD_NODE_BYTES + 2 * WORD) as usize;
            }
            if word(nodes, id * KEYWORD_NODE_BYTES + 3 * WORD) & HAS_OUTPUT != 0 {
                values.merge(candidates(nodes, id * KEYWORD_NODE_BYTES + 4 * WORD));
            }
        }
        values
    }
    fn prefix(&self, key: u128, slot: usize) -> Candidates {
        let width = if slot == 0 { 32 } else { 128 };
        let nodes = self.section(if slot == 0 { Section::V4 } else { Section::V6 });
        let mut id = self.roots[slot];
        let mut values = Candidates::default();
        while id != NONE {
            let node = &nodes[id as usize * PREFIX_BYTES..];
            let length = word(node, PREFIX_LENGTH_OFFSET);
            let stored =
                u128::from_le_bytes(node[..PREFIX_KEY_BYTES].try_into().expect("已验证固定记录"));
            let mask = if length == 0 {
                0
            } else {
                u128::MAX << (width - length)
            };
            if key & mask != stored {
                break;
            }
            values.merge(candidates(node, PREFIX_VALUES_OFFSET));
            if length == width {
                break;
            }
            let side = ((key >> (width - length - 1)) & 1) as usize;
            id = word(node, PREFIX_CHILDREN_OFFSET + side * WORD);
        }
        values
    }
    pub(super) fn query(
        &self,
        name: Option<&str>,
        ip: Option<IpAddr>,
        port: u16,
        transport: Transport,
    ) -> Candidates {
        let mut values = candidates(
            self.section(Section::Transports),
            usize::from(transport == Transport::Udp) * CANDIDATE_BYTES,
        );
        let ports = self.section(Section::Ports);
        if !ports.is_empty() && word(ports, 0) <= u32::from(port) {
            let mut low = 0;
            let mut high = ports.len() / PORT_BYTES;
            while low < high {
                let mid = (low + high) / 2;
                if word(ports, mid * PORT_BYTES) <= u32::from(port) {
                    low = mid + 1;
                } else {
                    high = mid;
                }
            }
            values.merge(candidates(ports, (low - 1) * PORT_BYTES + WORD));
        }
        if let Some(name) = name {
            let domains = self.domain_candidates(name).for_transport(transport);
            values.merge(domains);
            if domains.any() {
                return values;
            }
        }
        if let Some(ip) = ip {
            values.merge(match ip {
                IpAddr::V4(ip) => self.prefix(u128::from(u32::from(ip)), 0),
                IpAddr::V6(ip) => self.prefix(u128::from(ip), 1),
            });
        }
        values
    }
}
#[derive(Default)]
struct KeywordNode {
    children: BTreeMap<u8, u32>,
    fail: u32,
    depth: u32,
    values: Candidates,
}
pub(super) struct Builder;
impl Builder {
    pub(super) fn compile(index: &Index, entries: &[Entry]) -> Result<FlatIndex> {
        let estimate = entries.iter().try_fold(BUILD_BASE_BYTES, |sum, entry| {
            let labels = match &entry.matcher {
                Matcher::Domain(value, true) => value.split('.').count(),
                _ => 0,
            };
            let keyword = match &entry.matcher {
                Matcher::Keyword(value) => value.len(),
                _ => 0,
            };
            sum.checked_add(BUILD_RULE_BYTES)
                .and_then(|n| {
                    labels
                        .checked_mul(BUILD_SUFFIX_LABEL_BYTES)
                        .and_then(|extra| n.checked_add(extra))
                })
                .and_then(|n| {
                    keyword
                        .checked_mul(BUILD_KEYWORD_BYTE_FACTOR)
                        .and_then(|k| n.checked_add(k))
                })
                .ok_or(Error::ResourceLimit)
        })?;
        if estimate > MAX_BUILD_BYTES {
            return Err(Error::ResourceLimit);
        }
        let mut memory = BUILD_MEMORY.reserve(estimate)?;
        let mut sections: [Vec<u8>; SECTION_COUNT] = std::array::from_fn(|_| Vec::new());
        let mut strings = Vec::new();
        let mut string = |value: &str| -> Result<[u32; 2]> {
            let pair = [count(strings.len())?, count(value.len())?];
            strings.extend_from_slice(value.as_bytes());
            Ok(pair)
        };
        let slots = (index
            .exact
            .len()
            .checked_mul(2)
            .ok_or(Error::ResourceLimit)?
            .max(1))
        .next_power_of_two();
        let exact = &mut sections[Section::Exact as usize];
        exact.resize(slots * EXACT_BYTES, 0);
        for row in exact.as_chunks_mut::<EXACT_BYTES>().0.iter_mut() {
            for cell in row[2 * WORD..].as_chunks_mut::<WORD>().0.iter_mut() {
                cell.copy_from_slice(&NONE.to_le_bytes());
            }
        }
        for (name, values) in &index.exact {
            let mut slot = hash(name) & (slots - 1);
            let mut probes = 1;
            while word(exact, slot * EXACT_BYTES + WORD) != 0 {
                if probes >= MAX_EXACT_PROBES {
                    return Err(Error::ResourceLimit);
                }
                slot = (slot + 1) & (slots - 1);
                probes += 1;
            }
            let mut row = Vec::with_capacity(EXACT_BYTES);
            for part in string(name)? {
                push(&mut row, part);
            }
            put_candidates(&mut row, *values);
            exact[slot * EXACT_BYTES..(slot + 1) * EXACT_BYTES].copy_from_slice(&row);
        }
        for node in &index.suffix.nodes {
            let mut children: Vec<_> = node.children.iter().collect();
            children.sort_unstable_by(|a, b| a.0.cmp(b.0));
            let start = count(sections[Section::SuffixEdges as usize].len() / EDGE_BYTES)?;
            let nodes = &mut sections[Section::SuffixNodes as usize];
            push(nodes, start);
            let hashed = children.len() >= HASH_EDGE_THRESHOLD;
            let slots = if hashed {
                (children.len() * 2).next_power_of_two()
            } else {
                children.len()
            };
            push(
                nodes,
                count(slots)?
                    | if hashed { HASH_EDGES } else { 0 }
                    | if node.values.any() {
                        SUFFIX_HAS_OUTPUT
                    } else {
                        0
                    },
            );
            put_candidates(nodes, node.values);
            let edges = &mut sections[Section::SuffixEdges as usize];
            if hashed {
                let begin = edges.len();
                edges.resize(begin + slots * EDGE_BYTES, 0);
                for edge in edges[begin..].as_chunks_mut::<EDGE_BYTES>().0 {
                    edge[2 * WORD..3 * WORD].copy_from_slice(&NONE.to_le_bytes());
                }
                for (label, child) in children {
                    let mut slot = hash(label) & (slots - 1);
                    let mut probes = 1;
                    while word(edges, begin + slot * EDGE_BYTES + 2 * WORD) != NONE {
                        if probes >= MAX_EXACT_PROBES {
                            return Err(Error::ResourceLimit);
                        }
                        slot = (slot + 1) & (slots - 1);
                        probes += 1;
                    }
                    let [offset, length] = string(label)?;
                    let row =
                        &mut edges[begin + slot * EDGE_BYTES..begin + (slot + 1) * EDGE_BYTES];
                    row[..WORD].copy_from_slice(&offset.to_le_bytes());
                    row[WORD..2 * WORD].copy_from_slice(&length.to_le_bytes());
                    row[2 * WORD..].copy_from_slice(&child.to_le_bytes());
                }
            } else {
                for (label, child) in children {
                    for part in string(label)? {
                        push(edges, part);
                    }
                    push(edges, *child);
                }
            }
        }
        for (section, prefixes) in [(Section::V4, &index.v4), (Section::V6, &index.v6)] {
            let out = &mut sections[section as usize];
            for node in &prefixes.nodes {
                out.extend_from_slice(&node.key.to_le_bytes());
                push(out, u32::from(node.length));
                for child in node.children {
                    push(out, child);
                }
                put_candidates(out, node.values);
            }
        }
        let out = &mut sections[Section::Ports as usize];
        for (start, values) in index.ports.starts.iter().zip(&index.ports.values) {
            push(out, *start);
            put_candidates(out, *values);
        }
        // 稀疏 Aho-Corasick 自动机：失败边与合并候选写入文件，加载不重建自动机。
        let mut keywords = vec![KeywordNode::default()];
        for (id, entry) in entries.iter().enumerate() {
            if let Matcher::Keyword(pattern) = &entry.matcher {
                let mut node = 0usize;
                for byte in pattern.bytes() {
                    let child = match keywords[node].children.get(&byte) {
                        Some(child) => *child,
                        None => {
                            let child = count(keywords.len())?;
                            let depth = keywords[node].depth + 1;
                            keywords.push(KeywordNode {
                                depth,
                                ..KeywordNode::default()
                            });
                            keywords[node].children.insert(byte, child);
                            child
                        }
                    };
                    node = child as usize;
                }
                keywords[node]
                    .values
                    .add(entry.class() as usize, count(id)?);
            }
        }
        let mut queue: VecDeque<usize> = keywords[0]
            .children
            .values()
            .map(|id| *id as usize)
            .collect();
        while let Some(parent) = queue.pop_front() {
            let children: Vec<_> = keywords[parent]
                .children
                .iter()
                .map(|(b, id)| (*b, *id as usize))
                .collect();
            for (byte, child) in children {
                let mut fail = keywords[parent].fail as usize;
                while fail != 0 && !keywords[fail].children.contains_key(&byte) {
                    fail = keywords[fail].fail as usize;
                }
                let target = keywords[fail].children.get(&byte).copied().unwrap_or(0);
                keywords[child].fail = target;
                let inherited = keywords[target as usize].values;
                keywords[child].values.merge(inherited);
                queue.push_back(child);
            }
        }
        for node in &keywords {
            let start = count(sections[Section::KeywordEdges as usize].len() / KEYWORD_EDGE_BYTES)?;
            let out = &mut sections[Section::KeywordNodes as usize];
            push(out, start);
            push(out, count(node.children.len())?);
            push(out, node.fail);
            push(
                out,
                node.depth | if node.values.any() { HAS_OUTPUT } else { 0 },
            );
            put_candidates(out, node.values);
            for (byte, child) in &node.children {
                let out = &mut sections[Section::KeywordEdges as usize];
                push(out, u32::from(*byte));
                push(out, *child);
            }
        }
        // 已写入平铺区段，立即释放临时关键词树，避免与最终缓冲区同时驻留。
        drop(keywords);
        let out = &mut sections[Section::Transports as usize];
        for values in index.transports {
            put_candidates(out, values);
        }
        sections[Section::Strings as usize] = strings;
        let total = sections.iter().try_fold(HEADER_BYTES, |sum, part| {
            sum.checked_add(part.len()).ok_or(Error::ResourceLimit)
        })?;
        let mut bytes = Vec::with_capacity(total);
        let mut offset = HEADER_BYTES;
        for section in SECTIONS {
            let slot = section as usize;
            push(&mut bytes, count(offset)?);
            push(&mut bytes, count(sections[slot].len() / WIDTHS[slot])?);
            offset += sections[slot].len();
        }
        push(&mut bytes, index.v4.root);
        push(&mut bytes, index.v6.root);
        for section in sections {
            bytes.extend_from_slice(&section);
        }
        if bytes.capacity() > estimate {
            return Err(Error::ResourceLimit);
        }
        memory.shrink_to(bytes.capacity());
        FlatIndex::parse_owned(bytes, entries, Some(memory))
    }
}
