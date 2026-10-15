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
mod tests {
    use super::*;
    use crate::outbound::{Family, RoutedTarget};
    use crate::routing::{Match, Policy, Rule, Target, Transport};
    use std::{
        sync::mpsc,
        time::{Duration, Instant},
    };
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };
    fn sources(policy: Policy) -> Vec<Source> {
        vec![Source {
            id: "local".into(),
            layer: super::super::runtime::Layer::Local,
            enabled: true,
            rules: vec![Rule {
                protocol: None,
                r#type: Match::IpCidr,
                value: "127.0.0.0/8".into(),
                policy,
                outbound: None,
            }],
        }]
    }
    fn path() -> PathBuf {
        // 生命周期编号只保证本进程唯一；测试目录增加进程与时间，不能读入旧测试产物。
        std::env::temp_dir().join(format!(
            "fluxa-background-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            crate::lifecycle::identifier().unwrap()
        ))
    }
    async fn finished(service: &Service, id: u64) -> Job {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let job = service.job(id).unwrap();
            if job.phase != Phase::Compiling {
                return job;
            }
            assert!(Instant::now() < deadline, "后台任务必须在测试期限内完成");
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    }
    #[test]
    fn queued_input_is_also_charged_to_shared_rule_pool() {
        let before = super::super::runtime::memory_usage().reserved_bytes;
        let mut parsed =
            GenerationInput::from_yaml("format_version: 1\nsettings: {}\nsources: []\n").unwrap();
        let prepared = PreparedInput {
            settings: Config::default(),
            sources: Vec::new(),
            _memory: parsed.take_memory(),
        };
        drop(parsed);
        assert!(
            super::super::runtime::memory_usage().reserved_bytes > before,
            "解析对象离开后，来源消费凭证仍持有配额"
        );
        drop(prepared);
        assert_eq!(super::super::runtime::memory_usage().reserved_bytes, before);
        let input = INPUTS.reserve(4096).unwrap();
        assert_eq!(
            super::super::runtime::memory_usage().reserved_bytes,
            before + 4096
        );
        drop(input);
        assert_eq!(super::super::runtime::memory_usage().reserved_bytes, before);
        let usage = super::super::runtime::memory_usage();
        let occupied = super::super::runtime::MEMORY
            .reserve(usage.limit_bytes - usage.reserved_bytes)
            .unwrap();
        assert!(matches!(
            INPUTS.reserve(1),
            Err(Error::ResourceBudgetExceeded { .. })
        ));
        drop(occupied);
        assert!(INPUTS.reserve(4096).is_ok(), "总池失败也归还输入子配额");
    }
    #[tokio::test]
    async fn async_generation_keeps_connections_old_until_commit_and_bounds_workers() {
        let directory = path();
        let manager = Arc::new(Manager::default());
        let store = Store::open(&directory).unwrap();
        store
            .update_network(Config::default(), sources(Policy::Direct), &manager)
            .unwrap();
        let service = Service::new(store, Arc::clone(&manager));
        let old = service.store.snapshot().unwrap();
        let (send, receive) = mpsc::channel();
        let job = service
            .spawn(
                move || {
                    receive.recv().unwrap();
                    Ok((Config::default(), sources(Policy::Reject)))
                },
                INPUTS.reserve(1).unwrap(),
            )
            .unwrap();
        assert_eq!(service.job(job.id).unwrap().phase, Phase::Compiling);
        assert!(matches!(
            service.start(Config::default(), sources(Policy::Reject)),
            Err(Error::InvalidState)
        ));
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let target = Target {
            domain: None,
            address: Some("127.0.0.1".into()),
            port: listener.local_addr().unwrap().port(),
            transport: Transport::Tcp,
        };
        let request = RoutedTarget {
            target: target.clone(),
            timeout_ms: 1000,
            family: Family::Ipv4,
            mapping_generation: None,
        };
        let opened = manager.connect_configured(request.clone()).await.unwrap();
        let (mut peer, _) = listener.accept().await.unwrap();
        assert_eq!(opened.decision.rule_version, Some(1));
        assert_eq!(service.store.snapshot().unwrap().document().version, 1);
        for _ in 0..100 {
            assert_eq!(
                manager
                    .network_snapshot()
                    .unwrap()
                    .router
                    .as_ref()
                    .unwrap()
                    .evaluate(target.clone())
                    .unwrap()
                    .policy,
                Policy::Direct
            );
        }
        send.send(()).unwrap();
        assert_eq!(finished(&service, job.id).await.version, Some(2));
        assert!(matches!(
            manager.connect_configured(request).await,
            Err(Error::Rejected)
        ));
        assert_eq!(old.evaluate(&target).unwrap().policy, Policy::Direct);
        // 独立验证新约定：连接只保留既定决策与活动资源；旧规则快照在连接仍工作时即可回收。
        let old_weak = Arc::downgrade(&old);
        drop(old);
        assert!(
            old_weak.upgrade().is_none(),
            "活动连接不得间接持有整份旧规则快照"
        );
        // 切换不能关闭已开始的连接；新请求已在上面断言使用版本 2 的拒绝规则。
        manager
            .write(opened.connection.id, b"x", 1000)
            .await
            .unwrap();
        let mut byte = [0];
        peer.read_exact(&mut byte).await.unwrap();
        peer.write_all(&byte).await.unwrap();
        assert_eq!(
            &*manager
                .read(opened.connection.id, 1, 1000)
                .await
                .unwrap()
                .bytes,
            b"x"
        );
        manager.close(opened.connection.id).await.unwrap();
        let before = std::fs::read(directory.join("runtime-rules.bin")).unwrap();
        let mut invalid = sources(Policy::Direct);
        invalid[0].rules[0].value = "invalid".into();
        let failed = service.start(Config::default(), invalid).unwrap();
        assert_eq!(finished(&service, failed.id).await.phase, Phase::Failed);
        assert_eq!(service.store.snapshot().unwrap().document().version, 2);
        let (send, receive) = mpsc::channel();
        let cancelled = service
            .spawn(
                move || {
                    receive.recv().unwrap();
                    Ok((Config::default(), sources(Policy::Direct)))
                },
                INPUTS.reserve(1).unwrap(),
            )
            .unwrap();
        service.cancel(cancelled.id).unwrap();
        assert!(matches!(
            service.start(Config::default(), sources(Policy::Direct)),
            Err(Error::InvalidState)
        ));
        send.send(()).unwrap();
        assert_eq!(
            finished(&service, cancelled.id).await.phase,
            Phase::Cancelled
        );
        assert_eq!(
            std::fs::read(directory.join("runtime-rules.bin")).unwrap(),
            before
        );
        let (send, receive) = mpsc::channel();
        let closing = service
            .spawn(
                move || {
                    receive.recv().unwrap();
                    Ok((Config::default(), sources(Policy::Direct)))
                },
                INPUTS.reserve(1).unwrap(),
            )
            .unwrap();
        service.close().unwrap();
        let reopened = Store::open(&directory).unwrap();
        send.send(()).unwrap();
        assert_eq!(finished(&service, closing.id).await.phase, Phase::Cancelled);
        assert_eq!(reopened.snapshot().unwrap().document().version, 2);
        reopened.close().unwrap();
        drop(reopened);
        drop(service);
        std::fs::remove_dir_all(directory).unwrap();
        // 阻塞四个加载器确定性耗尽全局槽位；第五个请求不能创建额外线程。
        let mut running = Vec::new();
        for _ in 0..4 {
            let directory = path();
            let service = Service::new(
                Store::open(&directory).unwrap(),
                Arc::new(Manager::default()),
            );
            let (send, receive) = mpsc::channel();
            let job = service
                .spawn(
                    move || {
                        receive.recv().unwrap();
                        Ok((Config::default(), sources(Policy::Direct)))
                    },
                    INPUTS.reserve(1).unwrap(),
                )
                .unwrap();
            running.push((directory, service, send, job.id));
        }
        let directory = path();
        let extra = Service::new(
            Store::open(&directory).unwrap(),
            Arc::new(Manager::default()),
        );
        assert!(matches!(
            extra.start(Config::default(), sources(Policy::Direct)),
            Err(Error::ResourceBudgetExceeded {
                actual: 1,
                limit: 4
            })
        ));
        for (directory, service, send, id) in running {
            send.send(()).unwrap();
            assert_eq!(finished(&service, id).await.phase, Phase::Succeeded);
            service.close().unwrap();
            drop(service);
            std::fs::remove_dir_all(directory).unwrap();
        }
        let restored = extra
            .start(Config::default(), sources(Policy::Direct))
            .unwrap();
        assert_eq!(finished(&extra, restored.id).await.phase, Phase::Succeeded);
        extra.close().unwrap();
        drop(extra);
        std::fs::remove_dir_all(directory).unwrap();
    }
}
