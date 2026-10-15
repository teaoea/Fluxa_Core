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
use crate::test_fixture_https::dns as test_upstream;

#[cfg(test)]
#[path = "../../tests/test_dns_runtime.rs"]
mod test_dns_runtime;
