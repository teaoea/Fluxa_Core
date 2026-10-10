//! 所有权集中在注册表；外部只能使用不可复用的整数句柄。

use std::collections::BTreeMap;

const MAX_INSTANCES: usize = 64;

pub(crate) struct Registry<T> {
    next_id: u64,
    instances: BTreeMap<u64, T>,
}

impl<T> Default for Registry<T> {
    fn default() -> Self {
        Self {
            next_id: 0,
            instances: BTreeMap::new(),
        }
    }
}

impl<T> Registry<T> {
    pub(crate) fn create(&mut self, resource: T) -> Option<u64> {
        if self.instances.len() >= MAX_INSTANCES {
            return None;
        }
        // 句柄永不复用，避免已释放句柄误操作后来的实例；溢出明确拒绝。
        let id = self.next_id.checked_add(1)?;
        self.instances.insert(id, resource);
        self.next_id = id;
        Some(id)
    }

    pub(crate) fn get(&self, id: u64) -> Option<&T> {
        self.instances.get(&id)
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
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };
        struct Resource(Arc<AtomicUsize>);
        impl Drop for Resource {
            fn drop(&mut self) {
                self.0.fetch_add(1, Ordering::SeqCst);
            }
        }
        let releases = Arc::new(AtomicUsize::new(0));
        let mut registry = Registry {
            next_id: 0,
            instances: BTreeMap::new(),
        };
        let handle = registry.create(Resource(Arc::clone(&releases))).unwrap();
        assert!(registry.take(handle).is_some());
        assert!(!registry.take(handle).is_some());
        assert_eq!(releases.load(Ordering::SeqCst), 1);
        registry.create(Resource(Arc::clone(&releases))).unwrap();
        drop(registry);
        assert_eq!(releases.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn stale_handles_never_refer_to_new_instances() {
        let mut registry = Registry::default();
        let old = registry.create(()).unwrap();
        assert!(registry.take(old).is_some());
        let new = registry.create(()).unwrap();
        assert_ne!(old, new);
        assert!(!registry.get(old).is_some());
        assert!(!registry.take(old).is_some());
        assert!(registry.get(new).is_some());
    }

    #[test]
    fn instance_limit_is_released_after_destroy() {
        let mut registry = Registry::default();
        let handles: Vec<_> = (0..MAX_INSTANCES)
            .map(|_| registry.create(()).unwrap())
            .collect();
        assert_eq!(registry.create(()), None);
        assert!(registry.take(handles[0]).is_some());
        assert!(registry.create(()).is_some());
    }

    #[test]
    fn overflow_does_not_reuse_zero_or_old_handles() {
        let mut registry = Registry {
            next_id: u64::MAX,
            ..Registry::default()
        };
        assert_eq!(registry.create(()), None);
        assert!(!registry.get(0).is_some());
    }
}
