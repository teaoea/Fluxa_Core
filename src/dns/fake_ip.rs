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
    sync::Mutex,
};
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
struct Ranges {
    ipv4: u32,
    ipv6: u128,
    available: u32,
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
struct Mapping {
    name: String,
    _lease: ByteLease,
}
#[derive(Default)]
struct State {
    by_name: BTreeMap<String, u32>,
    by_id: BTreeMap<u32, Mapping>,
    cursor: u32,
}
pub struct Table {
    config: Config,
    ranges: Ranges,
    state: Mutex<State>,
}
impl Table {
    pub fn new(config: Config) -> Result<Self> {
        let ranges = config.ranges()?;
        Ok(Self {
            config,
            ranges,
            state: Mutex::new(State::default()),
        })
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
            let id = state.cursor.checked_add(1).ok_or(Error::ResourceLimit)?;
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
