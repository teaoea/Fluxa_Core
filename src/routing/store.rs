//! 生成文件与内存快照的提交：编译在锁外，文件替换后只进行不会返回错误的指针切换。
use super::Config;
use super::runtime::{Snapshot, Source};
use crate::error::{Error, Result};
use std::{
    fs::{self, File, OpenOptions},
    io::Read,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
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
struct Temporary(PathBuf, File);
impl Temporary {
    fn create(path: PathBuf) -> Result<Self> {
        let mut options = OpenOptions::new();
        options.read(true).write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        // 创建成功才接管清理责任；创建失败不能删除原有目录项。
        let file = options.open(&path)?;
        Ok(Self(path, file))
    }
    fn validate(&self) -> Result<()> {
        let metadata = fs::symlink_metadata(&self.0)?;
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return Err(Error::InvalidInput);
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let owned = self.1.metadata()?;
            // 路径必须仍指向创建时的文件，不能提交被替换的普通文件。
            if metadata.dev() != owned.dev() || metadata.ino() != owned.ino() {
                return Err(Error::InvalidInput);
            }
        }
        Ok(())
    }
}
impl Drop for Temporary {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_file(&self.0)
            && error.kind() != std::io::ErrorKind::NotFound
        {
            crate::error::diagnostics::record(
                error.into(),
                crate::error::diagnostics::DiagnosticSite::Cleanup,
            );
        }
    }
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
        let path = directory.join("runtime-rules.bin");
        let snapshot = match fs::symlink_metadata(&path) {
            Ok(_) => Some(Arc::new(super::cache::load(&path)?)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                if directory.join("runtime-rules.sqlite").try_exists()? {
                    return Err(Error::Unsupported);
                }
                let legacy = directory.join("runtime-rules.yaml");
                if legacy.try_exists()? {
                    let snapshot = Arc::new(Snapshot::migrate_yaml(&read(&legacy)?)?.into_flat()?);
                    let temporary = Self::prepare(&directory, &snapshot)?;
                    temporary.validate()?;
                    fs::rename(&temporary.0, &path)?;
                    File::open(&directory)?.sync_all()?;
                    Some(snapshot)
                } else {
                    None
                }
            }
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
    fn prepare(directory: &Path, snapshot: &Snapshot) -> Result<Temporary> {
        let unique = crate::lifecycle::identifier().ok_or(Error::ResourceLimit)?;
        let mut temporary = Temporary::create(directory.join(format!(
            ".runtime-rules-{}-{unique}.tmp",
            std::process::id()
        )))?;
        super::cache::write(&mut temporary.1, snapshot)?;
        temporary.validate()?;
        Ok(temporary)
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
        let mut timings = UpdateTimings::default();
        let started = Instant::now();
        let snapshot = Arc::new(Snapshot::generate(settings, sources, version)?.into_flat()?);
        timings.compile = started.elapsed();
        let started = Instant::now();
        let temporary = Self::prepare(&self.directory, &snapshot)?;
        timings.write_sync = started.elapsed();
        let started = Instant::now();
        let mut state = self.current.lock().map_err(|_| Error::Internal)?;
        if state.closed {
            return Err(Error::Cancelled);
        }
        Self::check_job(&state, job)?;
        if state.snapshot.as_ref().map_or(0, |s| s.document().version) != previous {
            return Err(Error::InvalidState);
        }
        let destination = self.directory.join("runtime-rules.bin");
        if fs::symlink_metadata(&destination).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err(Error::InvalidInput);
        }
        let commit_file = || {
            temporary.validate()?;
            fs::rename(&temporary.0, &destination)?;
            Ok(())
        };
        if let Some(manager) = manager {
            let router = Arc::new(super::Router::from_runtime(Arc::clone(&snapshot))?);
            manager.commit_routing_file(router, commit_file)?;
        } else {
            commit_file()?;
        }
        // 文件已提交：余下只替换 Arc，不执行可失败的内核启动、分配或索引编译。
        // 旧查询持有旧 Arc；进程在此处退出时下次启动会读取完整的新文件。
        state.snapshot = Some(snapshot);
        if let Some((_, _, committed)) = state.job.as_mut() {
            *committed = true;
        }
        timings.switch = started.elapsed();
        let started = Instant::now();
        let directory_synced = crate::error::diagnostics::observe(
            File::open(&self.directory)
                .and_then(|file| file.sync_all())
                .map_err(Error::from),
            crate::error::diagnostics::DiagnosticSite::Cleanup,
        )
        .is_ok();
        timings.directory_sync = started.elapsed();
        // 目录刷盘失败作为耐久性状态报告，不能伪造回滚或声称旧文件仍有效。
        Ok(Commit {
            version,
            directory_synced,
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
        state.lock.take();
        Ok(())
    }
}

#[cfg(test)]
mod cache_path_tests {
    use super::*;
    use crate::routing::cache;

    struct Directory(PathBuf);
    impl Directory {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "fluxa-cache-path-{}-{}",
                std::process::id(),
                crate::lifecycle::identifier().unwrap()
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Directory {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }
    fn snapshot() -> Snapshot {
        Snapshot::generate(Config::default(), vec![], 1)
            .unwrap()
            .into_flat()
            .unwrap()
    }

    #[test]
    fn cache_owned_file_roundtrip_preserves_document() {
        let directory = Directory::new();
        let mut temporary = Temporary::create(directory.0.join("cache.tmp")).unwrap();
        let expected = snapshot();
        cache::write(&mut temporary.1, &expected).unwrap();
        temporary.validate().unwrap();
        assert_eq!(
            cache::load(&temporary.0).unwrap().document(),
            expected.document()
        );
    }

    #[test]
    fn cache_write_rejects_nonempty_handle_without_truncation() {
        let directory = Directory::new();
        let mut temporary = Temporary::create(directory.0.join("cache.tmp")).unwrap();
        use std::io::Write;
        temporary.1.write_all(b"original-content").unwrap();
        assert_eq!(
            cache::write(&mut temporary.1, &snapshot()).unwrap_err(),
            Error::InvalidInput
        );
        assert_eq!(fs::read(&temporary.0).unwrap(), b"original-content");
    }

    #[test]
    fn cache_failed_exclusive_creation_does_not_remove_existing_file() {
        let directory = Directory::new();
        let path = directory.0.join("existing.tmp");
        fs::write(&path, b"original-content").unwrap();
        assert!(Temporary::create(path.clone()).is_err());
        assert_eq!(fs::read(path).unwrap(), b"original-content");
    }

    #[cfg(unix)]
    #[test]
    fn cache_symlink_swap_never_truncates_or_overwrites_target() {
        use std::os::unix::fs::symlink;
        let directory = Directory::new();
        let victim = directory.0.join("victim.txt");
        fs::write(&victim, b"must-remain-unchanged").unwrap();
        let mut temporary = Temporary::create(directory.0.join("cache.tmp")).unwrap();
        // 确定性替换发生在安全创建之后、缓存写入之前，覆盖原漏洞的真实窗口。
        fs::remove_file(&temporary.0).unwrap();
        symlink(&victim, &temporary.0).unwrap();
        cache::write(&mut temporary.1, &snapshot()).unwrap();
        assert_eq!(fs::read(&victim).unwrap(), b"must-remain-unchanged");
        assert!(temporary.1.metadata().unwrap().len() > 0);
        assert_eq!(temporary.validate().unwrap_err(), Error::InvalidInput);
        assert!(cache::load(&temporary.0).is_err());
        drop(temporary);
        assert_eq!(fs::read(&victim).unwrap(), b"must-remain-unchanged");
    }

    #[cfg(unix)]
    #[test]
    fn cache_regular_file_swap_is_rejected_by_identity() {
        let directory = Directory::new();
        let mut temporary = Temporary::create(directory.0.join("cache.tmp")).unwrap();
        fs::remove_file(&temporary.0).unwrap();
        fs::write(&temporary.0, b"replacement").unwrap();
        cache::write(&mut temporary.1, &snapshot()).unwrap();
        assert_eq!(fs::read(&temporary.0).unwrap(), b"replacement");
        assert_eq!(temporary.validate().unwrap_err(), Error::InvalidInput);
    }

    #[cfg(unix)]
    #[test]
    fn cache_failed_exclusive_creation_does_not_remove_symlink() {
        use std::os::unix::fs::symlink;
        let directory = Directory::new();
        let victim = directory.0.join("victim.txt");
        let path = directory.0.join("existing.tmp");
        fs::write(&victim, b"original-content").unwrap();
        symlink(&victim, &path).unwrap();
        assert!(Temporary::create(path.clone()).is_err());
        assert!(fs::symlink_metadata(path).unwrap().file_type().is_symlink());
        assert_eq!(fs::read(victim).unwrap(), b"original-content");
    }

    #[cfg(unix)]
    #[test]
    fn cache_rejected_update_keeps_snapshot_old_file_and_symlink_target() {
        use std::os::unix::fs::symlink;
        let directory = Directory::new();
        let store = Store::open(&directory.0).unwrap();
        store.update(Config::default(), vec![]).unwrap();
        let before = store.snapshot().unwrap();
        let path = directory.0.join("runtime-rules.bin");
        let saved = directory.0.join("saved.bin");
        let bytes = fs::read(&path).unwrap();
        fs::rename(&path, &saved).unwrap();
        let victim = directory.0.join("victim.txt");
        fs::write(&victim, b"must-remain-unchanged").unwrap();
        symlink(&victim, &path).unwrap();
        assert_eq!(
            store.update(Config::default(), vec![]).unwrap_err(),
            Error::InvalidInput
        );
        assert_eq!(store.snapshot().unwrap().document(), before.document());
        assert_eq!(fs::read(&saved).unwrap(), bytes);
        assert_eq!(fs::read(&victim).unwrap(), b"must-remain-unchanged");
        assert!(!fs::read_dir(&directory.0).unwrap().any(|entry| {
            entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .ends_with(".tmp")
        }));
        fs::remove_file(&path).unwrap();
        fs::rename(saved, path).unwrap();
        store.close().unwrap();
        let reopened = Store::open(&directory.0).unwrap();
        assert_eq!(reopened.snapshot().unwrap().document(), before.document());
        reopened.close().unwrap();
    }
}
