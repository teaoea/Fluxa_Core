//! 生成文件与内存快照的提交：编译在锁外，文件替换后只进行不会返回错误的指针切换。
use super::Config;
use super::runtime::{Snapshot, Source};
use crate::error::{Error, Result};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
const MAX_BYTES: u64 = 32 * 1024 * 1024;
pub struct Store {
    directory: PathBuf,
    current: Mutex<State>,
}
struct State {
    snapshot: Option<Arc<Snapshot>>,
    closed: bool,
    lock: Option<File>,
    job: Option<(u64, bool, bool)>,
}
struct Temporary(PathBuf);
impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Commit {
    pub version: u64,
    pub directory_synced: bool,
}
pub(crate) fn read(path: &Path) -> Result<String> {
    // 调用者提供自己的应用数据目录。拒绝符号链接，避免把配置源文件当作生成文件。
    if fs::symlink_metadata(path)?.file_type().is_symlink() {
        return Err(Error::InvalidInput);
    }
    let file = File::open(path)?;
    if !file.metadata()?.is_file() || file.metadata()?.len() > MAX_BYTES {
        return Err(Error::ResourceLimit);
    }
    let mut value = String::new();
    file.take(MAX_BYTES + 1).read_to_string(&mut value)?;
    if value.len() as u64 > MAX_BYTES {
        return Err(Error::ResourceLimit);
    }
    Ok(value)
}
impl Store {
    pub fn open(directory: impl AsRef<Path>) -> Result<Self> {
        let directory = directory.as_ref();
        if !directory.is_absolute() {
            return Err(Error::InvalidInput);
        }
        fs::create_dir_all(directory)?;
        let directory = fs::canonicalize(directory)?;
        let lock_path = directory.join(".runtime-rules.lock");
        if fs::symlink_metadata(&lock_path).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err(Error::InvalidInput);
        }
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let lock = options.open(lock_path)?;
        // 使用标准库的跨进程锁，不阻塞等待，也不使用 unsafe 或可能永久残留的占位锁。
        lock.try_lock().map_err(|error| match error {
            std::fs::TryLockError::WouldBlock => Error::InvalidState,
            std::fs::TryLockError::Error(_) => Error::Network,
        })?;
        let path = directory.join("runtime-rules.yaml");
        let snapshot = match fs::symlink_metadata(&path) {
            Ok(_) => Some(Arc::new(Snapshot::from_yaml(&read(&path)?)?)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(e.into()),
        };
        Ok(Self {
            directory,
            current: Mutex::new(State {
                snapshot,
                closed: false,
                lock: Some(lock),
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
        let snapshot = Arc::new(Snapshot::generate(settings, sources, version)?);
        let yaml = snapshot.to_yaml()?;
        // 完整严格解析校验生成文件；索引已经编译，不能在文件提交后再运行可能失败的编译。
        snapshot.verify_yaml(&yaml)?;
        let unique = crate::lifecycle::identifier().ok_or(Error::ResourceLimit)?;
        let temporary = Temporary(self.directory.join(format!(
            ".runtime-rules-{}-{unique}.tmp",
            std::process::id()
        )));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&temporary.0)?;
        file.write_all(yaml.as_bytes())?;
        file.sync_all()?;
        drop(file);
        let mut state = self.current.lock().map_err(|_| Error::Internal)?;
        if state.closed {
            return Err(Error::Cancelled);
        }
        Self::check_job(&state, job)?;
        if state.snapshot.as_ref().map_or(0, |s| s.document().version) != previous {
            return Err(Error::InvalidState);
        }
        let destination = self.directory.join("runtime-rules.yaml");
        if fs::symlink_metadata(&destination).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err(Error::InvalidInput);
        }
        if let Some(manager) = manager {
            let router = Arc::new(super::Router::from_runtime(Arc::clone(&snapshot))?);
            manager.commit_routing_file(router, || {
                fs::rename(&temporary.0, &destination)?;
                Ok(())
            })?;
        } else {
            fs::rename(&temporary.0, &destination)?;
        }
        // 文件已提交：余下只替换 Arc，不执行可失败的内核启动、分配或索引编译。
        // 旧查询持有旧 Arc；进程在此处退出时下次启动会读取完整的新文件。
        state.snapshot = Some(snapshot);
        if let Some((_, _, committed)) = state.job.as_mut() {
            *committed = true;
        }
        let directory_synced = File::open(&self.directory)
            .and_then(|file| file.sync_all())
            .is_ok();
        // 目录刷盘失败作为耐久性状态报告，不能伪造回滚或声称旧文件仍有效。
        Ok(Commit {
            version,
            directory_synced,
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
        state.lock.take();
        Ok(())
    }
}
