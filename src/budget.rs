//! 统一的按字节资源预算；配额随所有权释放，错误和取消无需手工回滚计数。
use crate::error::{Error, Result};
use std::sync::atomic::{AtomicUsize, Ordering};
pub(crate) struct ByteBudget {
    limit: usize,
    used: AtomicUsize,
    peak: AtomicUsize,
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
            peak: AtomicUsize::new(0),
        }
    }
    /// 只读观测准入计数；峰值是成功预留的高水位，不是整个进程 RSS。
    pub(crate) fn usage(&self) -> (usize, usize) {
        (
            self.used.load(Ordering::Acquire),
            self.peak.load(Ordering::Acquire),
        )
    }
    pub fn reserve(&'static self, bytes: usize) -> Result<ByteLease> {
        let previous = self
            .used
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |used| {
                used.checked_add(bytes).filter(|total| *total <= self.limit)
            })
            .map_err(|_| Error::ResourceBudgetExceeded {
                actual: bytes as u64,
                limit: self.limit as u64,
            })?;
        self.peak.fetch_max(previous + bytes, Ordering::Relaxed);
        Ok(ByteLease {
            budget: self,
            bytes,
        })
    }
}
impl ByteLease {
    /// 查询当前租约真实预留额，用于实际缓冲释放后的收缩检查。
    pub(crate) fn reserved_bytes(&self) -> usize {
        self.bytes
    }

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
#[path = "../tests/test_budget.rs"]
mod test_budget;
