//! 运行时规则只保存在内存；宿主管理持久化文件，Core 不创建目录、锁文件或缓存。
use super::Config;
use super::runtime::{Snapshot, Source};
use crate::error::{Error, Result};
use std::{
    path::Path,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
pub struct Store {
    current: Mutex<State>,
}
struct State {
    snapshot: Option<Arc<Snapshot>>,
    closed: bool,
    job: Option<(u64, bool, bool)>,
}
/// 更新阶段耗时随完成结果返回，不在匹配路径采样或写入日志。
#[derive(Clone, Copy, Debug, Default)]
pub struct UpdateTimings {
    pub compile: Duration,
    pub encode: Duration,
    pub validate: Duration,
    pub write_sync: Duration,
    pub switch: Duration,
    pub directory_sync: Duration,
}
#[derive(Clone, Copy, Debug)]
pub struct Commit {
    pub version: u64,
    pub directory_synced: bool,
    pub timings: UpdateTimings,
}
pub(crate) fn read(path: &Path) -> Result<zeroize::Zeroizing<String>> {
    crate::storage::ReadOnlyFile::open(path)?.read(32 * 1024 * 1024)
}
impl Store {
    /// 创建纯内存规则库，不访问文件系统。
    pub fn memory() -> Self {
        Self {
            current: Mutex::new(State {
                snapshot: None,
                closed: false,
                job: None,
            }),
        }
    }
    /// 只读打开宿主明确提供的二进制文件；不搜索目录、不迁移 YAML、不生成缓存。
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if !path.is_absolute() {
            return Err(Error::InvalidInput);
        }
        let snapshot = Arc::new(super::cache::load(path)?);
        Ok(Self {
            current: Mutex::new(State {
                snapshot: Some(snapshot),
                closed: false,
                job: None,
            }),
        })
    }
    pub fn snapshot(&self) -> Result<Arc<Snapshot>> {
        let state = self.current.lock().map_err(|_| Error::Internal)?;
        if state.closed {
            return Err(Error::InvalidState);
        }
        state.snapshot.clone().ok_or(Error::InvalidState)
    }
    pub fn update(&self, settings: Config, sources: Vec<Source>) -> Result<Commit> {
        self.update_inner(settings, sources, None, None)
    }
    pub fn update_network(
        &self,
        settings: Config,
        sources: Vec<Source>,
        manager: &crate::outbound::Manager,
    ) -> Result<Commit> {
        self.update_inner(settings, sources, Some(manager), None)
    }
    fn update_inner(
        &self,
        settings: Config,
        sources: Vec<Source>,
        manager: Option<&crate::outbound::Manager>,
        job: Option<u64>,
    ) -> Result<Commit> {
        let previous = {
            let state = self.current.lock().map_err(|_| Error::Internal)?;
            if state.closed {
                return Err(if job.is_some() {
                    Error::Cancelled
                } else {
                    Error::InvalidState
                });
            }
            Self::check_job(&state, job)?;
            state.snapshot.as_ref().map_or(0, |s| s.document().version)
        };
        let version = previous.checked_add(1).ok_or(Error::ResourceLimit)?;
        let started = Instant::now();
        let snapshot = Arc::new(Snapshot::generate(settings, sources, version)?.into_flat()?);
        let mut timings = UpdateTimings {
            compile: started.elapsed(),
            ..UpdateTimings::default()
        };
        let started = Instant::now();
        let mut state = self.current.lock().map_err(|_| Error::Internal)?;
        if state.closed {
            return Err(Error::Cancelled);
        }
        Self::check_job(&state, job)?;
        if state.snapshot.as_ref().map_or(0, |s| s.document().version) != previous {
            return Err(Error::InvalidState);
        }
        if let Some(manager) = manager {
            let router = Arc::new(super::Router::from_runtime(snapshot.clone())?);
            manager.commit_routing_file(router, || Ok(()))?;
        }
        state.snapshot = Some(snapshot);
        if let Some((_, _, committed)) = state.job.as_mut() {
            *committed = true;
        }
        timings.switch = started.elapsed();
        Ok(Commit {
            version,
            directory_synced: false,
            timings,
        })
    }
    // 任务身份与取消状态放在提交锁内，消除“检查取消后又提交”的竞态。
    fn check_job(state: &State, job: Option<u64>) -> Result<()> {
        match (state.job, job) {
            (None, None) => Ok(()),
            (Some((id, false, false)), Some(expected)) if id == expected => Ok(()),
            (Some((id, true, false)), Some(expected)) if id == expected => Err(Error::Cancelled),
            _ => Err(Error::InvalidState),
        }
    }
    pub(crate) fn begin_job(&self) -> Result<u64> {
        let mut state = self.current.lock().map_err(|_| Error::Internal)?;
        if state.closed || state.job.is_some() {
            return Err(Error::InvalidState);
        }
        let id = crate::lifecycle::identifier().ok_or(Error::ResourceLimit)?;
        state.job = Some((id, false, false));
        Ok(id)
    }
    pub(crate) fn cancel_job(&self, id: u64) -> Result<()> {
        let mut state = self.current.lock().map_err(|_| Error::Internal)?;
        match state.job.as_mut() {
            Some((active, cancelled, false)) if *active == id => {
                *cancelled = true;
                Ok(())
            }
            _ => Err(Error::InvalidState),
        }
    }
    pub(crate) fn job_cancelled(&self, id: u64) -> Result<bool> {
        let state = self.current.lock().map_err(|_| Error::Internal)?;
        Ok(state.closed
            || state.job.is_some_and(|(active, cancelled, committed)| {
                active == id && cancelled && !committed
            }))
    }
    pub(crate) fn finish_job(&self, id: u64) -> Result<()> {
        let mut state = self.current.lock().map_err(|_| Error::Internal)?;
        if state.job.is_some_and(|(active, _, _)| active == id) {
            state.job = None;
        }
        Ok(())
    }
    pub(crate) fn update_job(
        &self,
        settings: Config,
        sources: Vec<Source>,
        manager: &crate::outbound::Manager,
        id: u64,
    ) -> Result<Commit> {
        self.update_inner(settings, sources, Some(manager), Some(id))
    }
    pub fn close(&self) -> Result<()> {
        let mut state = self.current.lock().map_err(|_| Error::Internal)?;
        state.closed = true;
        state.snapshot.take();
        Ok(())
    }
}
#[cfg(test)]
#[path = "../../tests/test_routing_store.rs"]
mod test_routing_store;
