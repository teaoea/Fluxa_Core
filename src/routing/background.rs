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
static INPUTS: ByteBudget = ByteBudget::new(128 * 1024 * 1024);
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
}
pub struct Service {
    pub store: Arc<Store>,
    manager: Arc<Manager>,
    latest: Arc<Mutex<Option<Job>>>,
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
            if source.id.len() > 64 {
                return Err(Error::InvalidInput);
            }
            count = count
                .checked_add(source.rules.len())
                .ok_or(Error::ResourceLimit)?;
            for rule in &source.rules {
                if rule.value.len() > 253 || rule.outbound.as_ref().is_some_and(|id| id.len() > 64)
                {
                    return Err(Error::InvalidInput);
                }
                bytes = bytes
                    .checked_add(256 + rule.value.len() * 2)
                    .ok_or(Error::ResourceLimit)?;
            }
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
                let value = GenerationInput::from_yaml(&super::store::read(&path)?)?;
                Ok((
                    value.settings,
                    value
                        .sources
                        .into_iter()
                        .map(SourceInput::into_source)
                        .collect::<Result<_>>()?,
                ))
            },
            input,
        )
    }
    fn spawn<F>(&self, load: F, input: ByteLease) -> Result<Job>
    where
        F: FnOnce() -> Result<(Config, Vec<Source>)> + Send + 'static,
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
                    load().and_then(|(settings, sources)| {
                        store.update_job(settings, sources, &manager, id)
                    })
                }))
                .unwrap_or(Err(Error::Internal));
                let result = if result.is_err() && store.job_cancelled(id).unwrap_or(false) {
                    Err(Error::Cancelled)
                } else {
                    result
                };
                // 先持有状态锁再释放实例槽位，避免旧任务结果覆盖后提交的新任务。
                if let Ok(mut state) = state.lock() {
                    let _ = store.finish_job(id);
                    if state.as_ref().is_some_and(|job| job.id == id) {
                        *state = Some(Self::completed(id, result));
                    }
                } else {
                    let _ = store.finish_job(id);
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
        let _ = self.store.close();
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
                r#type: Match::IpCidr,
                value: "127.0.0.0/8".into(),
                policy,
                outbound: None,
            }],
        }]
    }
    fn path() -> PathBuf {
        std::env::temp_dir().join(format!(
            "fluxa-background-{}",
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
        // 已建立连接持有旧快照，规则切换不打断原连接的读写。
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
        manager.close(opened.connection.id).unwrap();
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
        drop(old);
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
            Err(Error::ResourceLimit)
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
