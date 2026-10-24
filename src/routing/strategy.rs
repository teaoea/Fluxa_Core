//! 有界有序策略表达式；只解析与校验，不触发拨号或运行状态变更。
use crate::error::{Error, Result};
pub(crate) const MAX_ITEMS: usize = 8;
pub(crate) const MAX_BYTES: usize = MAX_ITEMS * 128 + MAX_ITEMS - 1 + 2;
pub(crate) struct Plan<'a> {
    items: [&'a str; MAX_ITEMS],
    len: usize,
}
impl<'a> Plan<'a> {
    pub(crate) fn items(&self) -> &[&'a str] {
        &self.items[..self.len]
    }
    pub(crate) fn nodes(&self) -> impl Iterator<Item = &'a str> + '_ {
        self.items()
            .iter()
            .copied()
            .filter(|v| !matches!(*v, "direct" | "reject"))
    }
    pub(crate) fn canonical(&self) -> String {
        if self.len == 1 {
            self.items[0].to_owned()
        } else {
            format!("({})", self.items().join(","))
        }
    }
}
pub(crate) fn parse(value: &str) -> Result<Plan<'_>> {
    if value.len() > MAX_BYTES {
        return Err(Error::ResourceLimit);
    }
    let wrapped = value.starts_with('(');
    let body = if wrapped {
        value
            .strip_prefix('(')
            .and_then(|s| s.strip_suffix(')'))
            .ok_or(Error::InvalidInput)?
    } else {
        value
    };
    let mut plan = Plan {
        items: [""; MAX_ITEMS],
        len: 0,
    };
    for item in body.split(',') {
        if plan.len == MAX_ITEMS {
            return Err(Error::ResourceLimit);
        }
        if item.contains(['(', ')'])
            || plan.items().contains(&item)
            || plan.items().contains(&"reject")
        {
            return Err(Error::InvalidInput);
        }
        super::outbound_name(item)?;
        plan.items[plan.len] = item;
        plan.len += 1;
    }
    if wrapped && plan.len < 2 {
        return Err(Error::InvalidInput);
    }
    Ok(plan)
}
/// Proxy的单节点仍按节点名解释；仅显式组合表达式具有direct/reject候选语义。
pub(crate) fn members(value: &str) -> Result<Vec<&str>> {
    if value.contains(',') || value.starts_with('(') {
        Ok(parse(value)?.items().to_vec())
    } else {
        super::outbound_name(value)?;
        Ok(vec![value])
    }
}
#[cfg(test)]
#[path = "../../tests/test_routing_strategy.rs"]
mod test_routing_strategy;
