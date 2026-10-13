//! 统一的按字节资源预算；配额随所有权释放，错误和取消无需手工回滚计数。
use crate::error::{Error, Result};
use std::sync::atomic::{AtomicUsize, Ordering};
pub(crate) struct ByteBudget {
    limit: usize,
    used: AtomicUsize,
}
pub(crate) struct ByteLease {
    budget: &'static ByteBudget,
    bytes: usize,
}
impl ByteBudget {
    pub const fn new(limit: usize) -> Self {
        Self {
            limit,
            used: AtomicUsize::new(0),
        }
    }
    pub fn reserve(&'static self, bytes: usize) -> Result<ByteLease> {
        self.used
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |used| {
                used.checked_add(bytes).filter(|total| *total <= self.limit)
            })
            .map_err(|_| Error::ResourceLimit)?;
        Ok(ByteLease {
            budget: self,
            bytes,
        })
    }
}
impl ByteLease {
    /// 仅缩减已预留预算；缓冲区完成后归还多余容量，不重新申请配额。
    pub(crate) fn shrink_to(&mut self, bytes: usize) {
        assert!(bytes <= self.bytes, "预算只能缩减");
        self.budget
            .used
            .fetch_sub(self.bytes - bytes, Ordering::AcqRel);
        self.bytes = bytes;
    }
}
impl Drop for ByteLease {
    fn drop(&mut self) {
        self.budget.used.fetch_sub(self.bytes, Ordering::AcqRel);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn quota_is_returned_once_and_overflow_is_rejected() {
        static BUDGET: ByteBudget = ByteBudget::new(10);
        let lease = BUDGET.reserve(8).unwrap();
        assert!(BUDGET.reserve(3).is_err());
        assert!(BUDGET.reserve(usize::MAX).is_err());
        let mut lease = lease;
        lease.shrink_to(2);
        let extra = BUDGET.reserve(8).unwrap();
        drop(extra);
        drop(lease);
        assert!(BUDGET.reserve(10).is_ok());
        assert_eq!(BUDGET.used.load(Ordering::Acquire), 0);
    }
}
