//! 所有宿主共用的有界后台生成任务，不在 UI 线程编译，不为查询创建线程。
use super::{
    Config,
    runtime::{GenerationInput, Source, SourceInput},
    store::{Commit, Store},
};
use crate::{
    budget::{ByteBudget, ByteLease},
    error::{Error, Result},
    outbound::Manager,
};
use serde::Serialize;
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};
// 一个实例最多一个在途编译，全进程最多四个工作线程；配额由所有权释放。
static WORKERS: ByteBudget = ByteBudget::new(4);
// 输入既受独立队列额度约束，也计入规则共享池；不能用输入子池绕过总准入。
struct InputBudget {
    quota: ByteBudget,
}
struct InputLease {
    _global: ByteLease,
    _quota: ByteLease,
}
impl InputBudget {
    fn reserve(&'static self, bytes: usize) -> Result<InputLease> {
        let quota = self.quota.reserve(bytes)?;
        let global = super::runtime::MEMORY.reserve(bytes)?;
        Ok(InputLease {
            _global: global,
            _quota: quota,
        })
    }
}
static INPUTS: InputBudget = InputBudget {
    quota: ByteBudget::new(128 * 1024 * 1024),
};
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Phase {
    Compiling,
    Succeeded,
    Failed,
    Cancelled,
}
#[derive(Clone, Debug, Serialize)]
pub struct Job {
    pub id: u64,
    pub phase: Phase,
    pub version: Option<u64>,
    pub directory_synced: Option<bool>,
    pub error: Option<String>,
    /// 机器可识别的实际失败；旧 error 文本仅供诊断。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_info: Option<crate::error::ErrorInfo>,
}
pub struct Service {
    pub store: Arc<Store>,
    manager: Arc<Manager>,
    latest: Arc<Mutex<Option<Job>>>,
}
// 解析配额跟随实际来源直到编译消费结束，避免文件解码结束后出现无预算驻留窗口。
struct PreparedInput {
    settings: Config,
    sources: Vec<Source>,
    _memory: Option<ByteLease>,
}
impl From<(Config, Vec<Source>)> for PreparedInput {
    fn from((settings, sources): (Config, Vec<Source>)) -> Self {
        Self {
            settings,
            sources,
            _memory: None,
        }
    }
}
impl Service {
    pub fn new(store: Store, manager: Arc<Manager>) -> Self {
        Self {
            store: Arc::new(store),
            manager,
            latest: Arc::new(Mutex::new(None)),
        }
    }
    pub fn start(&self, settings: Config, sources: Vec<Source>) -> Result<Job> {
        let mut bytes = 4096usize;
        let mut count = 0usize;
        if sources.len() > super::runtime::MAX_SOURCES {
            return Err(Error::ResourceLimit);
        }
        for source in &sources {
            count = count
                .checked_add(source.rules.len())
                .ok_or(Error::ResourceLimit)?;
            bytes = bytes
                .checked_add(super::runtime::source_input_charge(source)?)
                .ok_or(Error::ResourceLimit)?;
        }
        if count > super::runtime::MAX_RULES {
            return Err(Error::ResourceLimit);
        }
        let input = INPUTS.reserve(bytes)?;
        self.spawn(move || Ok((settings, sources)), input)
    }
    pub fn start_inputs(&self, settings: Config, sources: Vec<SourceInput>) -> Result<Job> {
        if sources.len() > super::runtime::MAX_SOURCES {
            return Err(Error::ResourceLimit);
        }
        let bytes = sources.iter().try_fold(4096usize, |sum, source| {
            sum.checked_add(source.input_bytes()?)
                .ok_or(Error::ResourceLimit)
        })?;
        let input = INPUTS.reserve(bytes)?;
        self.spawn(
            move || {
                Ok((
                    settings,
                    sources
                        .into_iter()
                        .map(SourceInput::into_source)
                        .collect::<Result<_>>()?,
                ))
            },
            input,
        )
    }
    /// 文件读取和严格 YAML 解码也由工作线程完成，提交接口仅接收有界路径。
    pub fn start_file(&self, path: PathBuf) -> Result<Job> {
        if !path.is_absolute() || path.as_os_str().len() > 4096 {
            return Err(Error::InvalidInput);
        }
        let input = INPUTS.reserve(32 * 1024 * 1024)?;
        self.spawn(
            move || {
                let mut value = GenerationInput::from_yaml(&super::store::read(&path)?)?;
                let memory = value.take_memory();
                Ok(PreparedInput {
                    settings: value.settings,
                    sources: value
                        .sources
                        .into_iter()
                        .map(SourceInput::into_source)
                        .collect::<Result<_>>()?,
                    _memory: memory,
                })
            },
            input,
        )
    }
    fn spawn<F, P>(&self, load: F, input: InputLease) -> Result<Job>
    where
        F: FnOnce() -> Result<P> + Send + 'static,
        P: Into<PreparedInput> + 'static,
    {
        let worker = WORKERS.reserve(1)?;
        // 锁顺序固定为 latest → Store；编译与文件 I/O 不持有 latest 锁。
        let mut latest = self.latest.lock().map_err(|_| Error::Internal)?;
        let id = self.store.begin_job()?;
        let job = Job {
            id,
            phase: Phase::Compiling,
            version: None,
            directory_synced: None,
            error: None,
            error_info: None,
        };
        let previous = latest.replace(job.clone());
        let state = Arc::clone(&self.latest);
        let store = Arc::clone(&self.store);
        let manager = Arc::clone(&self.manager);
        let result = std::thread::Builder::new()
            .name("fluxa-rule-compiler".into())
            .spawn(move || {
                let _worker = worker;
                let _input = input;
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    load().and_then(|prepared| {
                        let PreparedInput {
                            settings,
                            sources,
                            _memory,
                        } = prepared.into();
                        store.update_job(settings, sources, &manager, id)
                    })
                }))
                .unwrap_or(Err(Error::Internal));
                let result = if result.is_err()
                    && crate::error::diagnostics::observe(
                        store.job_cancelled(id),
                        crate::error::diagnostics::DiagnosticSite::BackgroundJob,
                    )
                    .unwrap_or(false)
                {
                    Err(Error::Cancelled)
                } else {
                    result
                };
                // 先持有状态锁再释放实例槽位，避免旧任务结果覆盖后提交的新任务。
                if let Ok(mut state) = state.lock() {
                    let _ = crate::error::diagnostics::observe(
                        store.finish_job(id),
                        crate::error::diagnostics::DiagnosticSite::BackgroundJob,
                    );
                    if state.as_ref().is_some_and(|job| job.id == id) {
                        *state = Some(Self::completed(id, result));
                    }
                } else {
                    crate::error::diagnostics::record(
                        Error::Internal,
                        crate::error::diagnostics::DiagnosticSite::BackgroundJob,
                    );
                    let _ = crate::error::diagnostics::observe(
                        store.finish_job(id),
                        crate::error::diagnostics::DiagnosticSite::BackgroundJob,
                    );
                }
            });
        if result.is_err() {
            self.store.finish_job(id)?;
            *latest = previous;
            return Err(Error::ResourceLimit);
        }
        Ok(job)
    }
    fn completed(id: u64, result: Result<Commit>) -> Job {
        match result {
            Ok(commit) => Job {
                id,
                phase: Phase::Succeeded,
                version: Some(commit.version),
                directory_synced: Some(commit.directory_synced),
                error: None,
                error_info: None,
            },
            Err(error) => Job {
                id,
                phase: if error == Error::Cancelled {
                    Phase::Cancelled
                } else {
                    Phase::Failed
                },
                version: None,
                directory_synced: None,
                error: Some(error.to_string()),
                error_info: Some(error.public_info()),
            },
        }
    }
    pub fn job(&self, id: u64) -> Result<Job> {
        self.latest
            .lock()
            .map_err(|_| Error::Internal)?
            .as_ref()
            .filter(|job| job.id == id)
            .cloned()
            .ok_or(Error::InvalidState)
    }
    pub fn cancel(&self, id: u64) -> Result<()> {
        let latest = self.latest.lock().map_err(|_| Error::Internal)?;
        let job = latest
            .as_ref()
            .filter(|job| job.id == id)
            .ok_or(Error::InvalidState)?;
        if job.phase != Phase::Compiling {
            return Ok(());
        }
        self.store.cancel_job(id)
    }
    pub fn close(&self) -> Result<()> {
        self.store.close()
    }
}
impl Drop for Service {
    fn drop(&mut self) {
        let _ = crate::error::diagnostics::observe(
            self.store.close(),
            crate::error::diagnostics::DiagnosticSite::Cleanup,
        );
    }
}

#[cfg(test)]
#[path = "../../tests/test_routing_background.rs"]
mod test_routing_background;
