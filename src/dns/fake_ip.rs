//! 有界 Fake-IP 映射：分配后不自动复用地址，防止旧连接误指向新的域名。
use super::normalize;
use crate::{
    budget::{ByteBudget, ByteLease},
    error::{Error, Result},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
    sync::{Arc, LazyLock, Mutex, Weak},
};
// 代次仅在当前进程有效，耗尽后拒绝，不回绕复用旧令牌。

static BUDGET: ByteBudget = ByteBudget::new(16 * 1024 * 1024);
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub ipv4_range: String,
    pub ipv6_range: String,
    pub capacity: usize,
    #[serde(default)]
    pub exclude: Vec<String>,
}
#[derive(Clone, PartialEq, Eq)]
struct Ranges {
    ipv4: u32,
    ipv6: u128,
    available: u32,
    ipv4_end: u32,
    ipv6_end: u128,
}
impl Config {
    pub fn validate(&self) -> Result<()> {
        self.ranges().map(|_| ())
    }
    fn ranges(&self) -> Result<Ranges> {
        if !(1..=4096).contains(&self.capacity) || self.exclude.len() > 128 {
            return Err(Error::InvalidInput);
        }
        for suffix in &self.exclude {
            if normalize(suffix)? == "." {
                return Err(Error::InvalidInput);
            }
        }
        let (ipv4, prefix4) = range(&self.ipv4_range)?;
        let (ipv6, prefix6) = range(&self.ipv6_range)?;
        let IpAddr::V4(ipv4) = ipv4 else {
            return Err(Error::InvalidInput);
        };
        let IpAddr::V6(ipv6) = ipv6 else {
            return Err(Error::InvalidInput);
        };
        if !(15..=30).contains(&prefix4) || !(64..=120).contains(&prefix6) {
            return Err(Error::InvalidInput);
        }
        let ipv4 = u32::from(ipv4);
        let ipv6 = u128::from(ipv6);
        let mask4 = u32::MAX << (32 - prefix4);
        let mask6 = u128::MAX << (128 - prefix6);
        if ipv4 & mask4 != ipv4
            || ipv6 & mask6 != ipv6
            || ipv4 & 0xfffe0000 != 0xc6120000
            || ipv6 >> 120 != 0xfd
        {
            return Err(Error::InvalidInput);
        }
        let available = ((1u128 << (32 - prefix4)) - 2).min((1u128 << (128 - prefix6)) - 1) as u32;
        if self.capacity as u32 > available {
            return Err(Error::InvalidInput);
        }
        Ok(Ranges {
            ipv4,
            ipv6,
            available,
            ipv4_end: ipv4 | !mask4,
            ipv6_end: ipv6 | !mask6,
        })
    }
}
fn range(value: &str) -> Result<(IpAddr, u32)> {
    if value.len() > 64 {
        return Err(Error::InvalidInput);
    }
    let (address, prefix) = value.split_once('/').ok_or(Error::InvalidInput)?;
    Ok((
        address.parse().map_err(|_| Error::InvalidInput)?,
        prefix.parse().map_err(|_| Error::InvalidInput)?,
    ))
}
// 原始 TUN 包没有代次字段：存活池可回收，退役地址必须继续拒绝复用。
// IPv4 固定位图覆盖整个基准测试网段；IPv6 只保存合并后的已分配地址区间。
const IPV4_BASE: u32 = 0xc6120000;
const IPV4_WORDS: usize = 131072 / u64::BITS as usize;
const MAX_ACTIVE_POOLS: usize = 16;
const MAX_IPV6_INTERVALS: usize = 4096;
const HISTORY_BYTES: usize = MAX_IPV6_INTERVALS * 64;
static TUN_POOLS: LazyLock<Mutex<Pools>> = LazyLock::new(|| Mutex::new(Pools::default()));
struct Namespace {
    ranges: Ranges,
    owner: Weak<()>,
}
struct Pools {
    active: Vec<Namespace>,
    ipv4: [u64; IPV4_WORDS],
    ipv6: Vec<(u128, u128)>,
    history_lease: Option<ByteLease>,
}
impl Default for Pools {
    fn default() -> Self {
        Self {
            active: Vec::new(),
            ipv4: [0; IPV4_WORDS],
            ipv6: Vec::new(),
            history_lease: None,
        }
    }
}
fn overlaps(left: &Ranges, right: &Ranges) -> bool {
    (left.ipv4 <= right.ipv4_end && right.ipv4 <= left.ipv4_end)
        || (left.ipv6 <= right.ipv6_end && right.ipv6 <= left.ipv6_end)
}
impl Pools {
    fn prune(&mut self) {
        self.active.retain(|pool| pool.owner.strong_count() != 0);
    }
    fn namespace(&mut self, ranges: &Ranges) -> Result<Arc<()>> {
        self.prune();
        if self
            .active
            .iter()
            .any(|pool| pool.ranges != *ranges && overlaps(ranges, &pool.ranges))
        {
            return Err(Error::InvalidState);
        }
        if let Some(owner) = self
            .active
            .iter()
            .find(|pool| pool.ranges == *ranges)
            .and_then(|pool| pool.owner.upgrade())
        {
            return Ok(owner);
        }
        if self.active.len() >= MAX_ACTIVE_POOLS {
            return Err(Error::ResourceLimit);
        }
        self.active
            .try_reserve(1)
            .map_err(|_| Error::ResourceLimit)?;
        let owner = Arc::new(());
        self.active.push(Namespace {
            ranges: ranges.clone(),
            owner: Arc::downgrade(&owner),
        });
        Ok(owner)
    }
    fn used(&self, ranges: &Ranges, id: u32) -> bool {
        let offset = (ranges.ipv4 + id - IPV4_BASE) as usize;
        let ip6 = ranges.ipv6 + u128::from(id);
        self.ipv4[offset / u64::BITS as usize] & (1 << (offset % u64::BITS as usize)) != 0
            || self
                .ipv6
                .get(self.ipv6.partition_point(|&(_, end)| end < ip6))
                .is_some_and(|&(start, _)| start <= ip6)
    }
    // 先检查并预留历史记录容量，再原子登记两个地址；失败不占用地址。
    fn remember(&mut self, ranges: &Ranges, id: u32) -> Result<()> {
        let ip6 = ranges.ipv6 + u128::from(id);
        let first = self
            .ipv6
            .partition_point(|&(_, end)| end.saturating_add(1) < ip6);
        let mut last = first;
        let mut start = ip6;
        let mut end = ip6;
        while let Some(&(left, right)) = self.ipv6.get(last) {
            if left > end.saturating_add(1) {
                break;
            }
            start = start.min(left);
            end = end.max(right);
            last += 1;
        }
        if first == last && self.ipv6.len() >= MAX_IPV6_INTERVALS {
            return Err(Error::ResourceLimit);
        }
        if self.history_lease.is_none() {
            let lease = BUDGET.reserve(HISTORY_BYTES)?;
            self.ipv6
                .try_reserve_exact(MAX_IPV6_INTERVALS)
                .map_err(|_| Error::ResourceLimit)?;
            self.history_lease = Some(lease);
        }
        if first == last {
            self.ipv6.insert(first, (start, end));
        } else {
            self.ipv6[first] = (start, end);
            self.ipv6.drain(first + 1..last);
        }
        let offset = (ranges.ipv4 + id - IPV4_BASE) as usize;
        self.ipv4[offset / u64::BITS as usize] |= 1 << (offset % u64::BITS as usize);
        Ok(())
    }
    fn allocate(&mut self, ranges: &Ranges, first: u32, tun: bool) -> Result<u32> {
        self.prune();
        if !tun
            && self
                .active
                .iter()
                .any(|pool| pool.ranges != *ranges && overlaps(ranges, &pool.ranges))
        {
            return Err(Error::InvalidState);
        }
        let protected = tun || self.active.iter().any(|pool| pool.ranges == *ranges);
        let mut id = first;
        while protected && id <= ranges.available && self.used(ranges, id) {
            id = id.checked_add(1).ok_or(Error::ResourceLimit)?;
        }
        if id > ranges.available {
            return Err(Error::ResourceLimit);
        }
        self.remember(ranges, id)?;
        Ok(id)
    }
}
struct Mapping {
    name: String,
    _lease: ByteLease,
}
#[derive(Default)]
struct State {
    by_name: BTreeMap<String, u32>,
    by_id: BTreeMap<u32, Mapping>,
    cursor: u32,
    namespace: Option<Arc<()>>,
}
pub struct Table {
    generation: u64,
    config: Config,
    ranges: Ranges,
    state: Mutex<State>,
}
impl Table {
    pub fn new(config: Config) -> Result<Self> {
        let ranges = config.ranges()?;
        let generation = crate::lifecycle::identifier().ok_or(Error::ResourceLimit)?;
        Ok(Self {
            generation,
            config,
            ranges,
            state: Mutex::new(State::default()),
        })
    }
    /// 只在 TUN 会话显式启用；存活重叠池拒绝绑定，最后一个解析器释放后允许变更范围。
    /// 退役地址仍受历史记录保护；已有非 TUN 映射不能转换。
    pub(crate) fn bind_tun(&self) -> Result<()> {
        let mut state = self.state.lock().map_err(|_| Error::Internal)?;
        if state.namespace.is_some() {
            return Ok(());
        }
        if state.cursor != 0 {
            return Err(Error::InvalidState);
        }
        state.namespace = Some(
            TUN_POOLS
                .lock()
                .map_err(|_| Error::Internal)?
                .namespace(&self.ranges)?,
        );
        Ok(())
    }
    /// None 表示排除项，调用者应明确使用真实解析；排除域名按标签边界匹配。
    pub fn allocate(&self, name: &str, ipv6: bool) -> Result<Option<(String, bool)>> {
        let name = normalize(name)?;
        if name == "."
            || name.parse::<IpAddr>().is_ok()
            || self.config.exclude.iter().any(|suffix| {
                let suffix = suffix.trim_end_matches('.').to_ascii_lowercase();
                name == suffix || name.ends_with(&format!(".{suffix}"))
            })
        {
            return Ok(None);
        }
        let mut state = self.state.lock().map_err(|_| Error::Internal)?;
        let (id, cached) = if let Some(id) = state.by_name.get(&name) {
            (*id, true)
        } else {
            if state.by_id.len() >= self.config.capacity || state.cursor >= self.ranges.available {
                return Err(Error::ResourceLimit);
            }
            // 两份最多 253 字节的域名与两个树节点、配额值合计预留 1 KiB。
            let lease = BUDGET.reserve(1024)?;
            let first = state.cursor.checked_add(1).ok_or(Error::ResourceLimit)?;
            let id = TUN_POOLS.lock().map_err(|_| Error::Internal)?.allocate(
                &self.ranges,
                first,
                state.namespace.is_some(),
            )?;
            state.by_name.insert(name.clone(), id);
            state.by_id.insert(
                id,
                Mapping {
                    name,
                    _lease: lease,
                },
            );
            state.cursor = id;
            (id, false)
        };
        Ok(Some((
            if ipv6 {
                Ipv6Addr::from(self.ranges.ipv6 + u128::from(id)).to_string()
            } else {
                Ipv4Addr::from(self.ranges.ipv4 + id).to_string()
            },
            cached,
        )))
    }
    pub fn generation(&self) -> u64 {
        self.generation
    }
    pub fn contains(&self, address: IpAddr) -> bool {
        match address {
            IpAddr::V4(value) => {
                (self.ranges.ipv4..=self.ranges.ipv4_end).contains(&u32::from(value))
            }
            IpAddr::V6(value) => {
                (self.ranges.ipv6..=self.ranges.ipv6_end).contains(&u128::from(value))
            }
        }
    }
    pub fn reverse(&self, address: &str) -> Result<Option<String>> {
        if address.len() > 64 {
            return Err(Error::InvalidInput);
        }
        let address: IpAddr = address.parse().map_err(|_| Error::InvalidInput)?;
        let id = match address {
            IpAddr::V4(ip) => u32::from(ip).checked_sub(self.ranges.ipv4),
            IpAddr::V6(ip) => u128::from(ip)
                .checked_sub(self.ranges.ipv6)
                .and_then(|x| u32::try_from(x).ok()),
        };
        let state = self.state.lock().map_err(|_| Error::Internal)?;
        Ok(id
            .and_then(|id| state.by_id.get(&id))
            .map(|entry| entry.name.clone()))
    }
    /// 清除映射但不回退游标，旧地址不会在同一实例中分配给其他域名。
    pub fn reset(&self) -> Result<()> {
        let mut state = self.state.lock().map_err(|_| Error::Internal)?;
        state.by_name.clear();
        state.by_id.clear();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn config(v4: &str, v6: &str) -> Ranges {
        Config {
            ipv4_range: v4.into(),
            ipv6_range: v6.into(),
            capacity: 2,
            exclude: vec![],
        }
        .ranges()
        .unwrap()
    }
    #[test]
    fn retired_pool_can_expand_without_reusing_either_address_family() {
        let old = config("198.18.0.0/16", "fd00:1::/64");
        let new = config("198.18.0.0/15", "fd00:1::/64");
        let mut pools = Pools::default();
        let owner = pools.namespace(&old).unwrap();
        assert_eq!(pools.allocate(&old, 1, true).unwrap(), 1);
        assert!(matches!(pools.namespace(&new), Err(Error::InvalidState)));
        drop(owner);
        let _new_owner = pools.namespace(&new).unwrap();
        assert_eq!(pools.allocate(&new, 1, true).unwrap(), 2);
        assert_eq!(pools.active.len(), 1);
    }
    #[test]
    fn either_family_history_prevents_reinterpretation_after_reconfiguration() {
        let old = config("198.18.0.0/16", "fd00:2::/64");
        let changed_v4 = config("198.19.0.0/16", "fd00:2::/64");
        let changed_v6 = config("198.18.0.0/16", "fd00:3::/64");
        let mut pools = Pools::default();
        pools.allocate(&old, 1, true).unwrap();
        assert_eq!(pools.allocate(&changed_v4, 1, true).unwrap(), 2);
        assert_eq!(pools.allocate(&changed_v6, 1, true).unwrap(), 2);
    }
    #[test]
    fn unused_retired_namespaces_do_not_consume_lifetime_quota() {
        let mut pools = Pools::default();
        for index in 0..40 {
            let ranges = config("198.18.0.0/16", &format!("fd00:{index:x}::/64"));
            let owner = pools.namespace(&ranges).unwrap();
            drop(owner);
        }
        pools.prune();
        assert!(pools.active.is_empty());
        assert!(pools.ipv6.is_empty());
        assert!(pools.history_lease.is_none());
    }
    #[test]
    fn shared_pool_remains_active_until_last_owner_and_allocations_are_unique() {
        let ranges = config("198.18.0.0/16", "fd00:4::/64");
        let larger = config("198.18.0.0/15", "fd00:4::/64");
        let mut pools = Pools::default();
        let first = pools.namespace(&ranges).unwrap();
        let second = pools.namespace(&ranges).unwrap();
        assert!(Arc::ptr_eq(&first, &second));
        drop(first);
        assert!(matches!(pools.namespace(&larger), Err(Error::InvalidState)));
        assert_eq!(pools.allocate(&ranges, 1, true).unwrap(), 1);
        assert_eq!(pools.allocate(&ranges, 1, true).unwrap(), 2);
        drop(second);
        assert!(pools.namespace(&larger).is_ok());
    }
    #[test]
    fn active_limit_is_reclaimed_and_concurrent_shared_allocations_are_unique() {
        let mut pools = Pools::default();
        let mut owners = Vec::new();
        for index in 0..MAX_ACTIVE_POOLS {
            let ranges = config(
                &format!("198.18.0.{}/30", index * 4),
                &format!("fd00:{index:x}::/64"),
            );
            owners.push(pools.namespace(&ranges).unwrap());
        }
        let extra = config("198.18.0.64/30", "fd00:100::/64");
        assert!(matches!(pools.namespace(&extra), Err(Error::ResourceLimit)));
        owners.pop();
        assert!(pools.namespace(&extra).is_ok());
        drop(owners);
        let pools = Arc::new(Mutex::new(Pools::default()));
        let ranges = config("198.18.0.0/16", "fd00:6::/64");
        let threads: Vec<_> = (0..8)
            .map(|_| {
                let pools = pools.clone();
                let ranges = ranges.clone();
                std::thread::spawn(move || {
                    let owner = pools.lock().unwrap().namespace(&ranges).unwrap();
                    let id = pools.lock().unwrap().allocate(&ranges, 1, true).unwrap();
                    drop(owner);
                    id
                })
            })
            .collect();
        let mut ids: Vec<_> = threads
            .into_iter()
            .map(|thread| thread.join().unwrap())
            .collect();
        ids.sort_unstable();
        assert_eq!(ids, (1..=8).collect::<Vec<_>>());
        let mut pools = pools.lock().unwrap();
        pools.prune();
        assert!(pools.active.is_empty());
    }
    #[test]
    fn fragmented_history_limit_is_atomic_and_adjacent_history_merges() {
        let ranges = config("198.18.0.0/16", "fd00:5::/64");
        let mut pools = Pools::default();
        for index in 0..MAX_IPV6_INTERVALS {
            pools.remember(&ranges, (index * 2 + 1) as u32).unwrap();
        }
        let rejected = (MAX_IPV6_INTERVALS * 2 + 1) as u32;
        assert!(matches!(
            pools.remember(&ranges, rejected),
            Err(Error::ResourceLimit)
        ));
        assert!(!pools.used(&ranges, rejected));
        pools.remember(&ranges, 2).unwrap();
        assert_eq!(pools.ipv6.len(), MAX_IPV6_INTERVALS - 1);
        pools.remember(&ranges, rejected).unwrap();
    }
}
