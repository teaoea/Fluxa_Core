//! 有界 Fake-IP 映射：分配后不自动复用地址，防止旧连接误指向新的域名。
use super::normalize;
use crate::lifecycle::session::{Authority, Operation, Phase};
use crate::{
    budget::{ByteBudget, ByteLease},
    error::{Error, Result},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
    sync::{Arc, LazyLock, Mutex},
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
    owner: Arc<Authority>,
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
    fn prune(&mut self) -> Result<()> {
        for pool in &self.active {
            pool.owner.phase()?;
        }
        self.active
            .retain(|pool| pool.owner.phase() != Ok(Phase::Closed));
        Ok(())
    }
    fn namespace(&mut self, ranges: &Ranges, owner: &Arc<Authority>) -> Result<Arc<Authority>> {
        self.prune()?;
        owner.check(owner.identity())?;
        if self
            .active
            .iter()
            .any(|pool| pool.ranges != *ranges && overlaps(ranges, &pool.ranges))
        {
            return Err(Error::InvalidState);
        }
        if self
            .active
            .iter()
            .any(|pool| pool.owner.identity() == owner.identity())
        {
            return Err(Error::InvalidState);
        }
        if self.active.len() >= MAX_ACTIVE_POOLS {
            return Err(Error::ResourceLimit);
        }
        self.active
            .try_reserve(1)
            .map_err(|_| Error::ResourceLimit)?;
        self.active.push(Namespace {
            ranges: ranges.clone(),
            owner: owner.clone(),
        });
        Ok(owner.clone())
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
    #[cfg(test)]
    fn allocate(&mut self, ranges: &Ranges, first: u32, tun: bool) -> Result<u32> {
        self.prune()?;
        self.allocate_active(ranges, first, tun)
    }
    fn allocate_active(&mut self, ranges: &Ranges, first: u32, tun: bool) -> Result<u32> {
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
    namespace: Option<Arc<Authority>>,
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
    pub(crate) fn bind_tun(&self, owner: &Arc<Authority>) -> Result<()> {
        let mut state = self.state.lock().map_err(|_| Error::Internal)?;
        if state.namespace.is_some() {
            return Err(Error::InvalidState);
        }
        if state.cursor != 0 {
            return Err(Error::InvalidState);
        }
        state.namespace = Some(
            TUN_POOLS
                .lock()
                .map_err(|_| Error::Internal)?
                .namespace(&self.ranges, owner)?,
        );
        Ok(())
    }
    /// 快照仅保留控制对象；使用权始终核验内核登记的身份、代次和状态。
    pub(crate) fn authority(&self) -> Result<Option<Arc<Authority>>> {
        Ok(self
            .state
            .lock()
            .map_err(|_| Error::Internal)?
            .namespace
            .clone())
    }
    fn operation(&self) -> Result<Option<Operation>> {
        self.authority()?
            .map(|owner| owner.enter(owner.identity()))
            .transpose()
    }
    pub(crate) fn ensure_active(&self) -> Result<()> {
        if let Some(owner) = self.authority()? {
            owner.check(owner.identity())?;
        }
        Ok(())
    }
    /// None 表示排除项，调用者应明确使用真实解析；排除域名按标签边界匹配。
    pub fn allocate(&self, name: &str, ipv6: bool) -> Result<Option<(String, bool)>> {
        let _operation = self.operation()?;
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
        // 租约核验及地址历史/映射提交处于同一内核操作范围。
        let mut pools = TUN_POOLS.lock().map_err(|_| Error::Internal)?;
        pools.prune()?;
        let owner = state.namespace.clone();
        let commit = || {
            let (id, cached) = if let Some(id) = state.by_name.get(&name) {
                (*id, true)
            } else {
                if state.by_id.len() >= self.config.capacity
                    || state.cursor >= self.ranges.available
                {
                    return Err(Error::ResourceLimit);
                }
                // 两份最多 253 字节的域名与两个树节点、配额值合计预留 1 KiB。
                let lease = BUDGET.reserve(1024)?;
                let first = state.cursor.checked_add(1).ok_or(Error::ResourceLimit)?;
                let id = pools.allocate_active(&self.ranges, first, state.namespace.is_some())?;
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
        };
        if let Some(owner) = owner {
            owner.with_active(owner.identity(), commit)
        } else {
            commit()
        }
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
        let _operation = self.operation()?;
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
        let _operation = self.operation()?;
        let mut state = self.state.lock().map_err(|_| Error::Internal)?;
        state.by_name.clear();
        state.by_id.clear();
        Ok(())
    }
}

#[cfg(test)]
#[path = "../../tests/test_dns_fake_ip.rs"]
mod test_dns_fake_ip;
