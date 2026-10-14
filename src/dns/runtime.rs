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
    pub(crate) fn fault(&self) -> Result<Option<Error>> {
        Ok(*self.tasks.fault.borrow())
    }
    /// 驱动准入失败立即保留真实资源原因，不等待 DNS 超时后误分类。
    pub(crate) async fn guard<T>(&self, future: impl Future<Output = Result<T>>) -> Result<T> {
        let mut fault = self.tasks.fault.subscribe();
        let failure = async {
            loop {
                if let Some(error) = *fault.borrow() {
                    return error;
                }
                if fault.changed().await.is_err() {
                    return Error::Internal;
                }
            }
        };
        tokio::select! { biased; error = failure => Err(error), result = future => result }
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
        let tasks = self.0.clone();
        let result = tasks.authority.with_active(tasks.authority.identity(), || {
            let mut workers = tasks.workers.lock().map_err(|_| Error::Internal)?;
            while let Some(result) = workers.try_join_next() {
                record_join(result);
            }
            if workers.len() >= MAX_BACKGROUND_TASKS {
                return Err(Error::ResourceLimit);
            }
            let lease = TASK_BUDGET.reserve(TASK_BYTES)?;
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
mod tests {
    use super::*;
    #[tokio::test]
    async fn background_budget_failure_is_not_dns_timeout() {
        let runtime = ManagedRuntime::new(Authority::new().unwrap());
        let expected = Error::ResourceBudgetExceeded {
            actual: TASK_BYTES as u64,
            limit: 16 * 1024 * 1024,
        };
        runtime.tasks.failed(expected);
        assert_eq!(
            runtime.guard(std::future::pending::<Result<()>>()).await,
            Err(expected)
        );
        runtime.stop().await.unwrap();
    }
}
