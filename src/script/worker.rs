//! 每个缓存脚本一个独立线程；手动清理中断并 join，TTL 到期只等待在途执行结束。
use super::{MAX_DATA, MAX_SOURCE, execute_cancellable};
use crate::{
    budget::{ByteBudget, ByteLease},
    error::{Error, Result},
};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
static WORKERS: ByteBudget = ByteBudget::new(4);
static SOURCES: ByteBudget = ByteBudget::new(4 * (2 * MAX_SOURCE + 8193));
static JOBS: ByteBudget = ByteBudget::new(64 * MAX_DATA);
/// 编译时仅绑定宿主授权的只读文件描述符及绝对路径，不读取或保存 JS 内容。
pub(crate) struct Binding {
    pub(crate) id: u64,
    pub(crate) path: PathBuf,
    pub(crate) ttl: u32,
    resource: Mutex<Option<Arc<crate::storage::ReadOnlyFile>>>,
    loaded: AtomicBool,
}
impl Binding {
    pub(crate) fn new(path: PathBuf, ttl: u32) -> Result<Arc<Self>> {
        if ttl == 0 || !path.is_absolute() {
            return Err(Error::InvalidInput);
        }
        let resource = crate::storage::ReadOnlyFile::open(&path)?;
        resource.check_size(MAX_SOURCE)?;
        Ok(Arc::new(Self {
            id: crate::lifecycle::identifier().ok_or(Error::ResourceLimit)?,
            path,
            ttl,
            resource: Mutex::new(Some(resource)),
            loaded: AtomicBool::new(false),
        }))
    }
}
struct Pending(Arc<AtomicUsize>);
impl Drop for Pending {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}
struct Job {
    input: Value,
    reply: tokio::sync::oneshot::Sender<Result<Value>>,
    _memory: ByteLease,
    _pending: Pending,
}
enum Command {
    Run(Job),
    Stop,
}
struct Worker {
    sender: mpsc::SyncSender<Command>,
    canceled: Arc<AtomicBool>,
    closed: Arc<AtomicBool>,
    expires: Instant,
    pending: Arc<AtomicUsize>,
    last_used: Instant,
    task: Option<JoinHandle<()>>,
}
impl Worker {
    fn start(binding: &Binding) -> Result<Self> {
        if !binding.path.is_absolute() {
            return Err(Error::InvalidInput);
        }
        let worker = WORKERS.reserve(1)?;
        let source_memory = SOURCES.reserve(2 * MAX_SOURCE + 8193)?;
        // 初次使用已固定描述符；显式清理或 TTL 后再次运行才重新打开宿主的同一授权路径。
        let file = if !binding.loaded.swap(true, Ordering::AcqRel) {
            binding
                .resource
                .lock()
                .map_err(|_| Error::Internal)?
                .as_ref()
                .cloned()
                .ok_or(Error::InvalidState)?
        } else {
            crate::storage::ReadOnlyFile::open(&binding.path)?
        };
        let source = file.read(MAX_SOURCE)?;
        *binding.resource.lock().map_err(|_| Error::Internal)? = Some(file);

        if source.is_empty() {
            return Err(Error::InvalidInput);
        }
        // 只用配置中的 js-ttl；不扫描、删除或解释 JS 头部注释。
        let expires = Instant::now()
            .checked_add(Duration::from_secs(binding.ttl.into()))
            .ok_or(Error::InvalidInput)?;
        let canceled = Arc::new(AtomicBool::new(false));
        let closed = Arc::new(AtomicBool::new(false));
        let stop = canceled.clone();
        let done = closed.clone();
        let (sender, receiver) = mpsc::sync_channel(4);
        let task = thread::Builder::new()
            .name("nodus-js".into())
            .stack_size(2 * 1024 * 1024)
            .spawn(move || {
                struct Finished(Arc<AtomicBool>);
                impl Drop for Finished {
                    fn drop(&mut self) {
                        self.0.store(true, Ordering::Release);
                    }
                }
                let _finished = Finished(done);
                let _worker = worker;
                let _source = source_memory;
                loop {
                    if stop.load(Ordering::Acquire) || Instant::now() >= expires {
                        break;
                    }
                    match receiver.recv_timeout(expires.saturating_duration_since(Instant::now())) {
                        Ok(Command::Run(job)) => {
                            if stop.load(Ordering::Acquire) || Instant::now() >= expires {
                                let _ = job.reply.send(Err(Error::Cancelled));
                                break;
                            }
                            // TTL 不打断本次执行；手动取消由 QuickJS interrupt handler 及时退出。
                            let result = execute_cancellable(&source, &job.input, stop.clone());
                            let _ = job.reply.send(result);
                        }
                        Ok(Command::Stop) | Err(_) => break,
                    }
                }
                // 关闭队列，已排队但尚未执行的调用明确失败，不能永久等候。
                for command in receiver.try_iter() {
                    if let Command::Run(job) = command {
                        let _ = job.reply.send(Err(Error::Cancelled));
                    }
                }
            })?;
        Ok(Self {
            sender,
            canceled,
            closed,
            expires,
            pending: Arc::new(AtomicUsize::new(0)),
            last_used: Instant::now(),
            task: Some(task),
        })
    }
    fn stop(&mut self) -> Result<()> {
        self.canceled.store(true, Ordering::Release);
        let _ = self.sender.try_send(Command::Stop);
        if let Some(task) = self.task.take() {
            task.join().map_err(|_| Error::Internal)?;
        }
        Ok(())
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        if let Err(e) = self.stop() {
            crate::error::diagnostics::record(
                e,
                crate::error::diagnostics::DiagnosticSite::Cleanup,
            );
        }
    }
}
#[derive(Default)]
pub(crate) struct Cache {
    workers: Mutex<BTreeMap<u64, Worker>>,
    closed: AtomicBool,
}
impl Cache {
    /// 只由内核流量管线调用；排队及源文件读取不阻塞 socket I/O 线程。
    pub(crate) async fn execute(
        self: &Arc<Self>,
        binding: Arc<Binding>,
        input: Value,
    ) -> Result<Value> {
        super::validate_input(&input)?;
        let mut encoded = super::LimitedJson(Vec::new());
        serde_json::to_writer(&mut encoded, &input).map_err(|_| Error::ResourceLimit)?;
        let memory = JOBS.reserve(8 * MAX_DATA)?;
        let me = self.clone();
        let reply = tokio::task::spawn_blocking(move || me.submit(&binding, input, memory))
            .await
            .map_err(|_| Error::Internal)??;
        reply.await.map_err(|_| Error::Cancelled)?
    }
    fn submit(
        &self,
        binding: &Binding,
        input: Value,
        memory: ByteLease,
    ) -> Result<tokio::sync::oneshot::Receiver<Result<Value>>> {
        let mut workers = self.workers.lock().map_err(|_| Error::Internal)?;
        if self.closed.load(Ordering::Acquire) {
            return Err(Error::InvalidState);
        }
        // 已退出线程先回收；保留旧快照不保留已到期缓存的执行权。
        workers.retain(|_, worker| !worker.closed.load(Ordering::Acquire));
        if let Some(worker) = workers.get(&binding.id)
            && Instant::now() >= worker.expires
        {
            // 在途线程仍可自然完成，不创建同一绑定的新线程绕过待移除状态。
            return Err(Error::Cancelled);
        }
        if !workers.contains_key(&binding.id) && WORKERS.usage().0 >= 4 {
            // 有界缓存按最近使用时间回收闲置线程；不能让重新编译产生的旧绑定耗尽四份全局配额。
            // pending 同时计入排队和在途执行，绝不为了容量淘汰打断正在执行的脚本。
            if let Some(id) = workers
                .iter()
                .filter(|(_, w)| w.pending.load(Ordering::Acquire) == 0)
                .min_by_key(|(_, w)| w.last_used)
                .map(|(id, _)| *id)
            {
                workers.remove(&id).ok_or(Error::Internal)?.stop()?;
            }
        }
        if let std::collections::btree_map::Entry::Vacant(entry) = workers.entry(binding.id) {
            entry.insert(Worker::start(binding)?);
        }
        let worker = workers.get_mut(&binding.id).ok_or(Error::Internal)?;
        worker.last_used = Instant::now();
        worker.pending.fetch_add(1, Ordering::AcqRel);
        let pending = Pending(worker.pending.clone());
        let (sender, receiver) = tokio::sync::oneshot::channel();
        worker
            .sender
            .try_send(Command::Run(Job {
                input,
                reply: sender,
                _memory: memory,
                _pending: pending,
            }))
            .map_err(|e| match e {
                mpsc::TrySendError::Full(_) => Error::ResourceLimit,
                mpsc::TrySendError::Disconnected(_) => Error::Cancelled,
            })?;
        Ok(receiver)
    }
    /// 清理完成意味着源缓存已释放且线程已经 join；不是只发送停止请求。
    pub(crate) fn clear(&self) -> Result<()> {
        let mut workers = self.workers.lock().map_err(|_| Error::Internal)?;
        for worker in workers.values() {
            worker.canceled.store(true, Ordering::Release);
            let _ = worker.sender.try_send(Command::Stop);
        }
        let mut error = None;
        for worker in workers.values_mut() {
            if let Err(e) = worker.stop() {
                error = Some(e);
            }
        }
        workers.clear();
        error.map_or(Ok(()), Err)
    }
    /// 先撤销准入；空缓存同步完成，避免没有任何异步资源的 FFI 销毁被虚构成 Pending。
    pub(crate) fn begin_close(&self) -> Result<bool> {
        self.closed.store(true, Ordering::Release);
        let workers = self.workers.lock().map_err(|_| Error::Internal)?;
        for worker in workers.values() {
            worker.canceled.store(true, Ordering::Release);
            let _ = worker.sender.try_send(Command::Stop);
        }
        Ok(workers.is_empty())
    }
    pub(crate) fn close(&self) -> Result<()> {
        self.closed.store(true, Ordering::Release);
        self.clear()
    }
}
impl Drop for Cache {
    fn drop(&mut self) {
        if let Err(e) = self.close() {
            crate::error::diagnostics::record(
                e,
                crate::error::diagnostics::DiagnosticSite::Cleanup,
            );
        }
    }
}
#[cfg(test)]
#[path = "../../tests/test_script_worker.rs"]
mod test_script_worker;

/// 一个已编译配置的脚本绑定；重新编译必须重新获取系统容器，禁止沿用全局根目录。
#[derive(Default)]
pub(crate) struct Bindings {
    entries: BTreeMap<(String, u32), Arc<Binding>>,
}
impl Bindings {
    fn diagnose(error: Error) {
        crate::error::diagnostics::record(
            error,
            crate::error::diagnostics::DiagnosticSite::BackgroundJob,
        );
    }
    pub(crate) fn from_file(
        declared: Option<&str>,
        file: Option<&crate::storage::NamedFile>,
        refs: Vec<(String, u32)>,
    ) -> Result<Arc<Self>> {
        if refs.len() > 1024 {
            return Err(Error::ResourceLimit);
        }
        if let Some(file) = file
            && Some(file.name.as_str()) != declared
        {
            return Err(Error::InvalidInput);
        }
        let mut requested = std::collections::BTreeSet::new();
        for (name, ttl) in refs {
            if Some(name.as_str()) != declared {
                Self::diagnose(Error::InvalidInput);
                continue;
            }
            requested.insert((name, ttl));
        }
        let mut entries = BTreeMap::new();
        if let Some(ttl) = requested.iter().map(|(_, ttl)| *ttl).min() {
            match file.map(|file| Binding::new(file.path.clone(), ttl)) {
                Some(Ok(binding)) => {
                    for key in requested {
                        entries.insert(key, binding.clone());
                    }
                }
                Some(Err(error)) => Self::diagnose(error),
                None => Self::diagnose(Error::InvalidInput),
            }
        }
        Ok(Arc::new(Self { entries }))
    }
    pub(crate) fn pin(&self) -> Result<()> {
        for binding in self.entries.values() {
            if binding
                .resource
                .lock()
                .map_err(|_| Error::Internal)?
                .is_none()
            {
                return Err(Error::InvalidState);
            }
        }
        Ok(())
    }
    pub(crate) fn contains(&self, name: &str, ttl: u32) -> bool {
        self.entries.contains_key(&(name.to_owned(), ttl))
    }
    pub(crate) fn get(&self, name: &str, ttl: u32) -> Result<Arc<Binding>> {
        self.entries
            .get(&(name.to_owned(), ttl))
            .cloned()
            .ok_or(Error::InvalidState)
    }
}
