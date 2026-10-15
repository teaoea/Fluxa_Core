//! 测试文件：tests/test_lifecycle.rs
//! 测试类型：单元测试、迁移回归
//! 测试对象：src/lifecycle.rs
//! 测试内容：
//! - owned_resources_are_released_exactly_once：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - stale_handles_never_refer_to_new_instances：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - instance_limit_is_released_after_destroy：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - overflow_does_not_reuse_zero_or_old_handles：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//!
//! 测试依据：保留原测试的业务/格式/授权/资源约束；《测试文件与报告统一管理原则》，独立语义审查不以实现输出反推预期。
//!
//! 前置条件：本地测试需文件/回环套接字权限；平台限定及忽略项以实际登记为准。
//!
//! 执行状态：由同名 Markdown 报告记录，不在源码中固化通过结论。

#[cfg(test)]
mod tests {
    use super::super::*;
    #[test]
    fn owned_resources_are_released_exactly_once() {
        use std::sync::{Arc, atomic::AtomicUsize};
        struct Resource(Arc<AtomicUsize>);
        impl Drop for Resource {
            fn drop(&mut self) {
                self.0.fetch_add(1, Ordering::SeqCst);
            }
        }
        let releases = Arc::new(AtomicUsize::new(0));
        let mut registry = Registry::default();
        let handle = registry.create(Resource(Arc::clone(&releases))).unwrap();
        assert!(registry.take(handle).is_some());
        assert!(registry.take(handle).is_none());
        assert_eq!(releases.load(Ordering::SeqCst), 1);
        registry.create(Resource(Arc::clone(&releases))).unwrap();
        drop(registry);
        assert_eq!(releases.load(Ordering::SeqCst), 2);
    }
    #[test]
    fn stale_handles_never_refer_to_new_instances() {
        let mut first = Registry::default();
        let old = first.create(()).unwrap();
        first.take(old).unwrap();
        let mut second = Registry::default();
        let new = second.create(()).unwrap();
        assert_ne!(old, new);
        assert!(second.get(old).is_none());
        assert!(second.take(old).is_none());
        assert!(second.get(new).is_some());
    }
    #[test]
    fn instance_limit_is_released_after_destroy() {
        let mut registry = Registry::default();
        let handles: Vec<_> = (0..MAX_INSTANCES)
            .map(|_| registry.create(()).unwrap())
            .collect();
        assert_eq!(registry.create(()), None);
        registry.take(handles[0]).unwrap();
        assert!(registry.create(()).is_some());
    }
    #[test]
    fn overflow_does_not_reuse_zero_or_old_handles() {
        let counter = AtomicU64::new(u64::MAX - 1);
        assert_eq!(allocate(&counter), Some(u64::MAX - 1));
        assert_eq!(allocate(&counter), None);
        assert_eq!(allocate(&counter), None);
    }
}
