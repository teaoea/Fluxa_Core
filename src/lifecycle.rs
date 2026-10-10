//! 所有权集中在注册表；进程内句柄统一分配，跨实例也不复用。
use std::{
    collections::BTreeMap,
    sync::atomic::{AtomicU64, Ordering},
};
const MAX_INSTANCES: usize = 64;
static IDENTIFIERS: AtomicU64 = AtomicU64::new(1);
fn allocate(counter: &AtomicU64) -> Option<u64> {
    counter
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
        .ok()
}
/// 仅供本进程使用；零无效，计数耗尽明确失败，不能持久化为跨进程身份。
pub(crate) fn identifier() -> Option<u64> {
    allocate(&IDENTIFIERS)
}

pub(crate) struct Registry<T> {
    instances: BTreeMap<u64, T>,
}
impl<T> Default for Registry<T> {
    fn default() -> Self {
        Self {
            instances: BTreeMap::new(),
        }
    }
}
impl<T> Registry<T> {
    pub(crate) fn create(&mut self, resource: T) -> Option<u64> {
        if self.instances.len() >= MAX_INSTANCES {
            return None;
        }
        let id = identifier()?;
        self.instances.insert(id, resource);
        Some(id)
    }
    pub(crate) fn get(&self, id: u64) -> Option<&T> {
        self.instances.get(&id)
    }
    pub(crate) fn len(&self) -> usize {
        self.instances.len()
    }
    pub(crate) fn iter(&self) -> impl Iterator<Item = (u64, &T)> {
        self.instances.iter().map(|(id, resource)| (*id, resource))
    }
    /// 批量移出所有权，在注册表锁外执行取消与析构。
    pub(crate) fn take_all(&mut self) -> Vec<T> {
        std::mem::take(&mut self.instances).into_values().collect()
    }
    pub(crate) fn take(&mut self, id: u64) -> Option<T> {
        self.instances.remove(&id)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
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
