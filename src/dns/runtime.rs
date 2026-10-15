//! DNS 驱动任务由内核登记、撤销并等待；旧解析器 Arc 不拥有继续运行的权限。
use crate::{
    budget::ByteBudget,
    error::{Error, Result},
    lifecycle::session::Authority,
};
use hickory_resolver::net::runtime::{RuntimeProvider, Spawn, TokioRuntimeProvider};
use std::{
    future::{Future, poll_fn},
    io,
    net::SocketAddr,
    pin::Pin,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::task::JoinSet;
// 八个上游及 UDP/TCP/加密驱动有界预留；任务内协议对象保守按 128 KiB 准入。
const MAX_BACKGROUND_TASKS: usize = 64;
const TASK_BYTES: usize = 128 * 1024;
static TASK_BUDGET: ByteBudget = ByteBudget::new(16 * 1024 * 1024);
struct Tasks {
    authority: Arc<Authority>,
    workers: Mutex<JoinSet<()>>,
    // 只保留最近事件与 watch 版本；不累积错误，也不表示永久故障。
    fault: tokio::sync::watch::Sender<Option<Error>>,
}
#[derive(Clone)]
pub(crate) struct ManagedRuntime {
    delegate: TokioRuntimeProvider,
    tasks: Arc<Tasks>,
}
#[derive(Clone)]
pub(crate) struct ManagedHandle(Arc<Tasks>);
fn record_join(result: std::result::Result<(), tokio::task::JoinError>) {
    crate::error::diagnostics::observe_join(
        result,
        crate::error::diagnostics::DiagnosticSite::DnsService,
    );
}
impl Tasks {
    fn failed(&self, error: Error) {
        crate::error::diagnostics::record(
            error,
            crate::error::diagnostics::DiagnosticSite::DnsService,
        );
        self.fault.send_replace(Some(error));
    }
}
impl ManagedRuntime {
    pub(crate) fn new(authority: Arc<Authority>) -> Self {
        Self {
            delegate: TokioRuntimeProvider::default(),
            tasks: Arc::new(Tasks {
                authority,
                workers: Mutex::new(JoinSet::new()),
                fault: tokio::sync::watch::channel(None).0,
            }),
        }
    }
    /// 准入故障是事件而非解析器健康状态。每次查询只观察开始后的事件；
    /// 同期查询保守失败并保留真实原因，历史故障不能阻止后续重新准入。
    pub(crate) async fn guard<T>(&self, future: impl Future<Output = Result<T>>) -> Result<T> {
        self.tasks
            .authority
            .check(self.tasks.authority.identity())?;
        let mut fault = self.tasks.fault.subscribe();
        // 订阅建立即以当前版本为边界，不清空共享状态，避免另一查询漏报。
        fault.borrow_and_update();
        let result = tokio::select! {
            biased;
            changed = fault.changed() => {
                changed.map_err(|_| Error::Internal)?;
                Err((*fault.borrow()).unwrap_or(Error::Internal))
            }
            result = future => result,
        };
        // spawn_bg 没有 Result。若同一次 poll 发布故障后 future 随即返回，
        // select 的等待分支尚未再轮询，仍必须将真实准入失败返回调用方。
        if fault.has_changed().map_err(|_| Error::Internal)? {
            return Err((*fault.borrow()).unwrap_or(Error::Internal));
        }
        self.tasks
            .authority
            .check(self.tasks.authority.identity())?;
        result
    }
    /// 只停止此解析器自己的驱动；排空 join 后才允许释放活动解析器配额。
    pub(crate) async fn stop(&self) -> Result<()> {
        self.tasks
            .workers
            .lock()
            .map_err(|_| Error::Internal)?
            .abort_all();
        loop {
            let result = poll_fn(|context| match self.tasks.workers.lock() {
                Ok(mut workers) => workers.poll_join_next(context).map(Ok),
                Err(_) => std::task::Poll::Ready(Err(Error::Internal)),
            })
            .await?;
            match result {
                Some(result) => record_join(result),
                None => return Ok(()),
            }
        }
    }
}
impl Spawn for ManagedHandle {
    fn spawn_bg(&mut self, future: impl Future<Output = ()> + Send + 'static) {
        self.spawn_with_budget(future, &TASK_BUDGET);
    }
}
impl ManagedHandle {
    /// 所有驱动共用同一准入流程；预算参数仅用于内部隔离测试，不对外暴露。
    fn spawn_with_budget(
        &mut self,
        future: impl Future<Output = ()> + Send + 'static,
        budget: &'static ByteBudget,
    ) {
        let tasks = self.0.clone();
        let result = tasks.authority.with_active(tasks.authority.identity(), || {
            let mut workers = tasks.workers.lock().map_err(|_| Error::Internal)?;
            while let Some(result) = workers.try_join_next() {
                record_join(result);
            }
            if workers.len() >= MAX_BACKGROUND_TASKS {
                return Err(Error::ResourceLimit);
            }
            let lease = budget.reserve(TASK_BYTES)?;
            workers.spawn_on(
                async move {
                    let _lease = lease;
                    future.await;
                },
                crate::lifecycle::runtime()?.handle(),
            );
            Ok(())
        });
        if let Err(error) = result {
            tasks.failed(error);
        }
    }
}
async fn controlled<T>(
    owner: Arc<Authority>,
    future: impl Future<Output = io::Result<T>>,
) -> io::Result<T> {
    let _operation = owner
        .enter(owner.identity())
        .map_err(|_| io::Error::new(io::ErrorKind::NotConnected, "DNS 使用权已撤销"))?;
    tokio::select! {
        biased;
        _ = owner.canceled() => Err(io::Error::new(io::ErrorKind::NotConnected, "DNS 使用权已撤销")),
        result = future => {
            let value = result?;
            owner.check(owner.identity()).map_err(|_| io::Error::new(io::ErrorKind::NotConnected, "DNS 使用权已撤销"))?;
            Ok(value)
        }
    }
}
impl RuntimeProvider for ManagedRuntime {
    type Handle = ManagedHandle;
    type Timer = <TokioRuntimeProvider as RuntimeProvider>::Timer;
    type Udp = <TokioRuntimeProvider as RuntimeProvider>::Udp;
    type Tcp = <TokioRuntimeProvider as RuntimeProvider>::Tcp;
    fn create_handle(&self) -> Self::Handle {
        ManagedHandle(self.tasks.clone())
    }
    fn connect_tcp(
        &self,
        server: SocketAddr,
        bind: Option<SocketAddr>,
        timeout: Option<Duration>,
    ) -> Pin<Box<dyn Send + Future<Output = io::Result<Self::Tcp>>>> {
        Box::pin(controlled(
            self.tasks.authority.clone(),
            self.delegate.connect_tcp(server, bind, timeout),
        ))
    }
    fn bind_udp(
        &self,
        local: SocketAddr,
        server: SocketAddr,
    ) -> Pin<Box<dyn Send + Future<Output = io::Result<Self::Udp>>>> {
        Box::pin(controlled(
            self.tasks.authority.clone(),
            self.delegate.bind_udp(local, server),
        ))
    }
}

#[cfg(test)]
#[path = "../../Tests/support/dns.rs"]
mod test_upstream;

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn background_budget_failure_is_not_dns_timeout() {
        let runtime = ManagedRuntime::new(Authority::new().unwrap());
        let expected = Error::ResourceBudgetExceeded {
            actual: TASK_BYTES as u64,
            limit: 16 * 1024 * 1024,
        };
        assert_eq!(
            runtime
                .guard(async {
                    runtime.tasks.failed(expected);
                    std::future::pending::<Result<()>>().await
                })
                .await,
            Err(expected)
        );
        runtime.stop().await.unwrap();
    }
    #[tokio::test]
    async fn historical_admission_failure_does_not_poison_new_query() {
        let runtime = ManagedRuntime::new(Authority::new().unwrap());
        runtime.tasks.failed(Error::ResourceLimit);
        assert_eq!(runtime.guard(async { Ok(()) }).await, Ok(()));
        runtime.stop().await.unwrap();
    }

    #[tokio::test]
    async fn same_poll_failure_cannot_be_returned_as_success() {
        let runtime = ManagedRuntime::new(Authority::new().unwrap());
        assert_eq!(
            runtime
                .guard(async {
                    runtime.tasks.failed(Error::ResourceLimit);
                    Ok(())
                })
                .await,
            Err(Error::ResourceLimit)
        );
        assert_eq!(runtime.guard(async { Ok(()) }).await, Ok(()));
    }

    #[tokio::test]
    async fn concurrent_queries_observe_event_without_clearing_each_other() {
        let runtime = ManagedRuntime::new(Authority::new().unwrap());
        let first = runtime.guard(std::future::pending::<Result<()>>());
        let second = runtime.guard(async {
            runtime.tasks.failed(Error::ResourceLimit);
            // 新查询开始后不能清除仍在执行的旧查询所需的故障事件。
            assert_eq!(runtime.guard(async { Ok(()) }).await, Ok(()));
            Ok(())
        });
        let results = tokio::time::timeout(Duration::from_secs(1), async {
            tokio::join!(first, second)
        })
        .await
        .unwrap();
        assert_eq!(
            results,
            (Err(Error::ResourceLimit), Err(Error::ResourceLimit))
        );
    }

    #[tokio::test]
    async fn task_budget_exhaustion_is_retryable_after_lease_release() {
        // 隔离预算但走生产准入流程，不耗尽其他并发测试的全局预算。
        static BUDGET: ByteBudget = ByteBudget::new(TASK_BYTES);
        let runtime = ManagedRuntime::new(Authority::new().unwrap());
        let lease = BUDGET.reserve(TASK_BYTES).unwrap();
        let mut handle = runtime.create_handle();
        let error = Error::ResourceBudgetExceeded {
            actual: TASK_BYTES as u64,
            limit: TASK_BYTES as u64,
        };
        assert_eq!(
            runtime
                .guard(async {
                    handle.spawn_with_budget(std::future::pending(), &BUDGET);
                    Ok(())
                })
                .await,
            Err(error)
        );
        assert_eq!(BUDGET.usage().0, TASK_BYTES);
        drop(lease);
        let (finished, received) = tokio::sync::oneshot::channel();
        runtime
            .guard(async {
                handle.spawn_with_budget(
                    async move {
                        finished.send(()).unwrap();
                    },
                    &BUDGET,
                );
                received.await.map_err(|_| Error::Internal)
            })
            .await
            .unwrap();
        runtime.stop().await.unwrap();
        assert_eq!(BUDGET.usage().0, 0);
    }

    #[tokio::test]
    async fn revoked_runtime_cannot_recover_or_spawn() {
        let authority = Authority::new().unwrap();
        let runtime = ManagedRuntime::new(authority.clone());
        authority.revoke().unwrap();
        assert_eq!(
            runtime.guard(async { Ok(()) }).await,
            Err(Error::InvalidState)
        );
        runtime.create_handle().spawn_bg(async {
            panic!("关闭后不得执行驱动");
        });
        assert!(runtime.tasks.workers.lock().unwrap().is_empty());
        runtime.stop().await.unwrap();
    }

    #[tokio::test]
    async fn all_transports_recover_after_worker_limit() {
        use super::super::{Config, Query, Resolver, Server};
        static BUDGET: ByteBudget = ByteBudget::new(MAX_BACKGROUND_TASKS * TASK_BYTES);
        let service = super::test_upstream::fixture().await;
        let mut roots = rustls::RootCertStore::empty();
        roots.add(service.root.clone()).unwrap();
        let trust = rustls::ClientConfig::builder_with_provider(Arc::new(
            rustls::crypto::ring::default_provider(),
        ))
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_root_certificates(roots)
        .with_no_client_auth();
        for server in [
            Server::Udp {
                address: format!("127.0.0.1:{}", service.port),
            },
            Server::Tcp {
                address: format!("127.0.0.1:{}", service.port),
            },
            Server::Dot {
                address: format!("127.0.0.1:{}", service.dot_port),
                server_name: "localhost".into(),
            },
            Server::Doh {
                url: format!("https://localhost:{}/dns-query", service.doh_port),
                bootstrap: format!("127.0.0.1:{}", service.doh_port),
            },
        ] {
            let resolver = Arc::new(
                Resolver::with_tls_config(
                    Config {
                        servers: vec![server],
                        timeout_ms: 1000,
                        dnssec: false,
                        fake_ip: None,
                        service: None,
                    },
                    trust.clone(),
                )
                .unwrap(),
            );
            let runtime = resolver
                .resources
                .read()
                .await
                .as_ref()
                .unwrap()
                .runtime
                .clone();
            let mut handle = runtime.create_handle();
            for _ in 0..MAX_BACKGROUND_TASKS {
                handle.spawn_with_budget(std::future::pending(), &BUDGET);
            }
            assert_eq!(
                runtime.tasks.workers.lock().unwrap().len(),
                MAX_BACKGROUND_TASKS
            );
            let query = || Query {
                name: "example.test".into(),
                query_type: "A".into(),
            };
            // 真正的 Hickory 驱动准入失败，必须返回 8001，不能误报为 DNS 超时。
            assert_eq!(
                resolver.resolve(query()).await.unwrap_err(),
                Error::ResourceLimit
            );
            runtime.stop().await.unwrap();
            assert_eq!(BUDGET.usage().0, 0);
            // 同一个解析器、上游和会话；不清缓存、不重建 Resolver，也不要求前端介入。
            let answer = resolver.resolve(query()).await.unwrap();
            assert_eq!(answer.addresses, ["192.0.2.44"]);
            assert!(!answer.from_cache);
            assert!(resolver.resolve(query()).await.unwrap().from_cache);
            resolver.close().await.unwrap();
            assert_eq!(
                resolver.resolve(query()).await.unwrap_err(),
                Error::InvalidState
            );
        }
    }
}
