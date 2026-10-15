//! 所有权集中在注册表；进程内句柄统一分配，跨实例也不复用。
pub(crate) mod session;

/// 内核自己的有界运行时；清理任务不依赖前端线程或外部对象释放。
pub(crate) fn runtime() -> crate::error::Result<&'static tokio::runtime::Runtime> {
    static RUNTIME: std::sync::OnceLock<crate::error::Result<tokio::runtime::Runtime>> =
        std::sync::OnceLock::new();
    RUNTIME
        .get_or_init(|| {
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .map_err(|_| crate::error::Error::Internal)
        })
        .as_ref()
        .map_err(|error| *error)
}
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
    pub(crate) fn get_mut(&mut self, id: u64) -> Option<&mut T> {
        self.instances.get_mut(&id)
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
#[path = "../tests/test_lifecycle.rs"]
mod test_lifecycle;
