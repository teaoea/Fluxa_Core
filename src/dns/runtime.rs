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
    route: Option<Arc<crate::outbound::Manager>>,
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
    fn transport_failed(&self, error: Error) {
        // 普通网络失败仍可尝试列表中的下一上游；资源/权限/内核故障必须保留真实错误，不能被重试掩盖。
        if matches!(
            error,
            Error::ResourceLimit
                | Error::ResourceBudgetExceeded { .. }
                | Error::InvalidState
                | Error::Cancelled
                | Error::Internal
        ) {
            self.failed(error);
        } else {
            crate::error::diagnostics::record(
                error,
                crate::error::diagnostics::DiagnosticSite::DnsService,
            );
        }
    }
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
            route: None,
            tasks: Arc::new(Tasks {
                authority,
                workers: Mutex::new(JoinSet::new()),
                fault: tokio::sync::watch::channel(None).0,
            }),
        }
    }
    /// DNS 的显式节点路由（UDP/加密传输）；共享任务预算和撤销权限，绝不失败后直连。
    pub(crate) fn routed(&self, network: Arc<crate::outbound::Manager>) -> Self {
        Self {
            route: Some(network),
            ..self.clone()
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
    type Udp = RuntimeUdp;
    type Tcp = hickory_resolver::net::runtime::iocompat::AsyncIoTokioAsStd<RuntimeStream>;
    fn create_handle(&self) -> Self::Handle {
        ManagedHandle(self.tasks.clone())
    }
    fn connect_tcp(
        &self,
        server: SocketAddr,
        bind: Option<SocketAddr>,
        timeout: Option<Duration>,
    ) -> Pin<Box<dyn Send + Future<Output = io::Result<Self::Tcp>>>> {
        let owner = self.tasks.authority.clone();
        let delegate = self.delegate.clone();
        let route = self.route.clone();
        let faults = self.tasks.clone();
        Box::pin(controlled(owner, async move {
            let stream: crate::network::SecureStream = match route {
                Some(network) => Box::new(
                    network
                        .connect_stream(&server.ip().to_string(), server.port(), None)
                        .await
                        .map_err(|error| {
                            faults.transport_failed(error);
                            io::Error::other(error)
                        })?,
                ),
                None => Box::new(delegate.connect_tcp(server, bind, timeout).await?.0),
            };
            Ok(hickory_resolver::net::runtime::iocompat::AsyncIoTokioAsStd(
                RuntimeStream(Mutex::new(stream)),
            ))
        }))
    }

    fn bind_udp(
        &self,
        local: SocketAddr,
        server: SocketAddr,
    ) -> Pin<Box<dyn Send + Future<Output = io::Result<Self::Udp>>>> {
        let owner = self.tasks.authority.clone();
        let route = self.route.clone();
        let faults = self.tasks.clone();
        Box::pin(controlled(owner, async move {
            if let Some(network) = route {
                let request = crate::outbound::RoutedTarget {
                    target: crate::routing::Target {
                        domain: None,
                        address: Some(server.ip().to_string()),
                        port: server.port(),
                        transport: crate::routing::Transport::Udp,
                    },
                    timeout_ms: 30000,
                    family: crate::outbound::Family::Auto,
                    mapping_generation: None,
                };
                let opened = network.connect_configured(request).await.map_err(|error| {
                    faults.transport_failed(error);
                    io::Error::other(error)
                })?;
                Ok(RuntimeUdp::Routed {
                    network,
                    id: opened.connection.id,
                    peer: server,
                    faults: ManagedHandle(faults),
                    state: Mutex::new(UdpPoll::default()),
                })
            } else {
                Ok(RuntimeUdp::Direct(
                    tokio::net::UdpSocket::bind(local).await?,
                ))
            }
        }))
    }
}

#[cfg(test)]
use crate::test_fixture_https::dns as test_upstream;

#[cfg(test)]
#[path = "../../tests/test_dns_runtime.rs"]
mod test_dns_runtime;

/// Hickory 要求 Sync；短锁仅覆盖一次 poll，不持锁跨 await，不把业务流变成共享控制接口。
pub(crate) struct RuntimeStream(Mutex<crate::network::SecureStream>);
impl tokio::io::AsyncRead for RuntimeStream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        b: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<io::Result<()>> {
        Pin::new(
            &mut *self
                .0
                .lock()
                .map_err(|_| io::Error::other(Error::Internal))?,
        )
        .poll_read(cx, b)
    }
}
impl tokio::io::AsyncWrite for RuntimeStream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        b: &[u8],
    ) -> std::task::Poll<io::Result<usize>> {
        Pin::new(
            &mut *self
                .0
                .lock()
                .map_err(|_| io::Error::other(Error::Internal))?,
        )
        .poll_write(cx, b)
    }
    fn poll_flush(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<io::Result<()>> {
        Pin::new(
            &mut *self
                .0
                .lock()
                .map_err(|_| io::Error::other(Error::Internal))?,
        )
        .poll_flush(cx)
    }
    fn poll_shutdown(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<io::Result<()>> {
        Pin::new(
            &mut *self
                .0
                .lock()
                .map_err(|_| io::Error::other(Error::Internal))?,
        )
        .poll_shutdown(cx)
    }
}

/// UDP 适配仍由共用出站注册真实连接；轮询状态有限，取消由内核会话决定。
pub(crate) enum RuntimeUdp {
    Direct(tokio::net::UdpSocket),
    Routed {
        network: Arc<crate::outbound::Manager>,
        id: u64,
        peer: SocketAddr,
        faults: ManagedHandle,
        state: Mutex<UdpPoll>,
    },
}
type ReadFuture = Pin<Box<dyn Future<Output = crate::error::Result<crate::outbound::Read>> + Send>>;
type WriteFuture = Pin<Box<dyn Future<Output = crate::error::Result<usize>> + Send>>;
#[derive(Default)]
pub(crate) struct UdpPoll {
    read: Option<ReadFuture>,
    write: Option<([u8; 32], WriteFuture)>,
}
impl hickory_resolver::net::runtime::DnsUdpSocket for RuntimeUdp {
    type Time = hickory_resolver::net::runtime::TokioTime;
    fn poll_recv_from(
        &self,
        cx: &mut std::task::Context<'_>,
        buf: &mut [u8],
    ) -> std::task::Poll<io::Result<(usize, SocketAddr)>> {
        use std::task::Poll;
        match self {
            Self::Direct(socket) => {
                let mut buffer = tokio::io::ReadBuf::new(buf);
                match socket.poll_recv_from(cx, &mut buffer) {
                    Poll::Ready(Ok(peer)) => Poll::Ready(Ok((buffer.filled().len(), peer))),
                    Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
                    Poll::Pending => Poll::Pending,
                }
            }
            Self::Routed {
                network,
                id,
                peer,
                state,
                ..
            } => {
                let mut state = state
                    .lock()
                    .map_err(|_| io::Error::other(Error::Internal))?;
                if state.read.is_none() {
                    let network = network.clone();
                    let id = *id;
                    state.read = Some(Box::pin(
                        async move { network.read(id, 65535, 30000).await },
                    ));
                }
                match state
                    .read
                    .as_mut()
                    .ok_or_else(|| io::Error::other(Error::Internal))?
                    .as_mut()
                    .poll(cx)
                {
                    Poll::Pending => Poll::Pending,
                    Poll::Ready(result) => {
                        state.read = None;
                        let value = result.map_err(|error| self.failure(error))?;
                        if value.bytes.len() > buf.len() {
                            return Poll::Ready(Err(io::Error::other(Error::ResourceLimit)));
                        }
                        buf[..value.bytes.len()].copy_from_slice(&value.bytes);
                        Poll::Ready(Ok((value.bytes.len(), *peer)))
                    }
                }
            }
        }
    }
    fn poll_send_to(
        &self,
        cx: &mut std::task::Context<'_>,
        bytes: &[u8],
        target: SocketAddr,
    ) -> std::task::Poll<io::Result<usize>> {
        use std::task::Poll;
        match self {
            Self::Direct(socket) => socket.poll_send_to(cx, bytes, target),
            Self::Routed {
                network,
                id,
                peer,
                state,
                ..
            } => {
                if target != *peer || bytes.len() > 4096 {
                    return Poll::Ready(Err(io::Error::other(Error::InvalidInput)));
                }
                let mut state = state
                    .lock()
                    .map_err(|_| io::Error::other(Error::Internal))?;
                if state.write.is_none() {
                    let network = network.clone();
                    let id = *id;
                    let bytes = bytes.to_vec();
                    let key = ring::digest::digest(&ring::digest::SHA256, &bytes)
                        .as_ref()
                        .try_into()
                        .map_err(|_| io::Error::other(Error::Internal))?;
                    state.write = Some((
                        key,
                        Box::pin(async move { network.write(id, &bytes, 30000).await }),
                    ));
                }
                let (key, future) = state
                    .write
                    .as_mut()
                    .ok_or_else(|| io::Error::other(Error::Internal))?;
                if key.as_slice() != ring::digest::digest(&ring::digest::SHA256, bytes).as_ref() {
                    return Poll::Ready(Err(self.failure(Error::ResourceLimit)));
                }
                match future.as_mut().poll(cx) {
                    Poll::Pending => Poll::Pending,
                    Poll::Ready(value) => {
                        state.write = None;
                        Poll::Ready(value.map_err(|error| self.failure(error)))
                    }
                }
            }
        }
    }
}

impl Drop for RuntimeUdp {
    fn drop(&mut self) {
        if let Self::Routed { network, id, .. } = self {
            network.release_dns_datagram(*id);
        }
    }
}

impl RuntimeUdp {
    fn failure(&self, error: Error) -> io::Error {
        if let Self::Routed { faults, .. } = self {
            faults.0.transport_failed(error);
        }
        io::Error::other(error)
    }
}
