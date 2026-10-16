//! 共用数字地址出站：TCP 字节流与连接式 UDP，拒绝和未实现代理不触网。
use crate::lifecycle::session::{Authority, Identity, Operation};
use crate::{
    budget::{ByteBudget, ByteLease},
    error::{Error, Result},
    lifecycle::Registry,
    routing::{Policy, Transport},
};
use std::{
    net::SocketAddr,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpStream, UdpSocket},
    sync::watch,
    time::timeout,
};
use zeroize::Zeroizing;

pub const MAX_BUFFER: usize = 65536;
pub const MAX_DATAGRAM: usize = 65507;
static CONNECTIONS: ByteBudget = ByteBudget::new(128);
static BUFFERS: ByteBudget = ByteBudget::new(16 * 1024 * 1024);
fn duration(milliseconds: u64) -> Result<Duration> {
    if !(1..=60000).contains(&milliseconds) {
        return Err(Error::InvalidInput);
    }
    Ok(Duration::from_millis(milliseconds))
}
/// 接口不解析域名；调用者必须显式提供数字地址，端口零和未指定地址拒绝。
pub fn endpoint(value: &str) -> Result<SocketAddr> {
    if value.len() > 64 {
        return Err(Error::InvalidInput);
    }
    let address: SocketAddr = value.parse().map_err(|_| Error::InvalidInput)?;
    if address.port() == 0 || address.ip().is_unspecified() {
        return Err(Error::InvalidInput);
    }
    Ok(address)
}
type BoxStream = crate::network::SecureStream;
fn tcp_socket(stream: BoxStream) -> Socket {
    let (reader, writer) = tokio::io::split(stream);
    Socket::Tcp {
        reader: tokio::sync::Mutex::new(reader),
        writer: tokio::sync::Mutex::new(writer),
    }
}
enum Socket {
    Detached,
    Tcp {
        reader: tokio::sync::Mutex<tokio::io::ReadHalf<BoxStream>>,
        writer: tokio::sync::Mutex<tokio::io::WriteHalf<BoxStream>>,
    },
    ProxyUdp {
        socket: crate::protocol::Datagram,
    },
    Udp {
        socket: UdpSocket,
        reader: tokio::sync::Mutex<()>,
    },
}
type Dialed = (Socket, ByteLease, SocketAddr);
type StreamSlot = Arc<Mutex<Option<BoxStream>>>;
struct SocketResources {
    socket: Socket,
    _lease: ByteLease,
    detached: Option<StreamSlot>,
    // 真实 Socket 释放后才归还会话活动资源计数；引用计数不作为租约判据。
    _session: Option<Operation>,
}
impl Drop for SocketResources {
    fn drop(&mut self) {
        if let Some(slot) = &self.detached {
            match slot.lock() {
                Ok(mut stream) => {
                    stream.take();
                }
                Err(_) => crate::error::diagnostics::record(
                    Error::Internal,
                    crate::error::diagnostics::DiagnosticSite::Cleanup,
                ),
            }
        }
    }
}
struct Connection {
    resources: Mutex<Option<Arc<SocketResources>>>,
    authority: Arc<Authority>,
    owner: Option<Arc<Authority>>,
    cancel: watch::Sender<bool>,
    address: SocketAddr,
    transport: Transport,
    revision: u64,
    idle_timeout: Duration,
    usage: Mutex<resources::Usage>,
    protection: crate::routing::TrafficProtection,
    quic: Mutex<crate::protocol::quic::Flow>,
}
// 字段按顺序释放：先归还真实 Socket 引用，再通知在途操作结束。
struct SocketOperation {
    resources: Arc<SocketResources>,
    _operation: Operation,
    _session: Option<Operation>,
}
impl Connection {
    fn inspect_datagram(self: &Arc<Self>, bytes: &[u8]) -> Result<()> {
        let quic = self
            .quic
            .lock()
            .map_err(|_| Error::Internal)?
            .observe(bytes);
        if let Err(error) = self.protection.check(quic) {
            self.close();
            return Err(error);
        }
        Ok(())
    }

    fn operation(&self) -> Result<SocketOperation> {
        self.protection.check(false)?;
        if *self.cancel.borrow() {
            return Err(Error::Cancelled);
        }
        let session = self
            .owner
            .as_ref()
            .map(|owner| owner.enter(owner.identity()))
            .transpose()?;
        let operation = self.authority.enter(self.authority.identity())?;
        let resources = self
            .resources
            .lock()
            .map_err(|_| Error::Internal)?
            .as_ref()
            .cloned()
            .ok_or(Error::InvalidState)?;
        Ok(SocketOperation {
            resources,
            _operation: operation,
            _session: session,
        })
    }
    fn close(self: &Arc<Self>) {
        let result = self.request_close();
        let _ = crate::error::diagnostics::observe(
            result,
            crate::error::diagnostics::DiagnosticSite::Cleanup,
        );
    }
    fn request_close(self: &Arc<Self>) -> Result<()> {
        if self.authority.revoke()? {
            self.cancel.send_replace(true);
            let own = self.clone();
            crate::lifecycle::runtime()?.spawn(async move {
                let result = async {
                    own.authority.wait_idle().await?;
                    own.resources.lock().map_err(|_| Error::Internal)?.take();
                    Ok(())
                }
                .await;
                let _ = crate::error::diagnostics::observe(
                    own.authority.complete(result),
                    crate::error::diagnostics::DiagnosticSite::Cleanup,
                );
            });
        }
        Ok(())
    }
    async fn wait_closed(&self) -> Result<()> {
        self.authority.wait_closed().await
    }
    async fn run<T>(
        &self,
        milliseconds: u64,
        future: impl std::future::Future<Output = Result<T>>,
    ) -> Result<T> {
        let deadline = duration(milliseconds)?;
        let mut cancellation = self.cancel.subscribe();
        if *cancellation.borrow() {
            return Err(Error::Cancelled);
        }
        tokio::select! {
            biased;
            _ = cancellation.changed() => Err(Error::Cancelled),
            result = timeout(deadline, future) => result.map_err(|_| Error::NetworkTimeout)?,
        }
    }
}
/// 返回缓冲区由 Rust 拥有，释放时清零并归还预算。
pub struct Read {
    pub bytes: Zeroizing<Vec<u8>>,
    pub eof: bool,
    _lease: ByteLease,
}
struct State {
    debug: Option<Arc<crate::routing::debug::Session>>,
    registry: Registry<Arc<Connection>>,
    stopped: bool,
    network: Arc<NetworkSnapshot>,
}
pub struct Manager {
    state: Arc<Mutex<State>>,
    reaper: Mutex<Option<tokio::task::JoinHandle<()>>>,
    generation: watch::Sender<u64>,
}
impl Default for Manager {
    fn default() -> Self {
        let (generation, _) = watch::channel(0);
        Self {
            state: Arc::new(Mutex::new(State {
                debug: None,
                registry: Registry::default(),
                stopped: false,
                network: Arc::new(NetworkSnapshot::default()),
            })),
            reaper: Mutex::new(None),
            generation,
        }
    }
}
impl Manager {
    pub async fn open(
        &self,
        address: SocketAddr,
        transport: Transport,
        policy: Policy,
        milliseconds: u64,
    ) -> Result<u64> {
        if address.port() == 0 || address.ip().is_unspecified() {
            return Err(Error::InvalidInput);
        }
        policy.check_support()?;
        let snapshot = self.network_snapshot()?;
        snapshot.check_transport(transport)?;
        Ok(self
            .establish_with(snapshot, duration(milliseconds)?, async |_| {
                dial(address, transport).await
            })
            .await?
            .id)
    }
    /// 具有配置上下文的数值诊断也拒绝合成池地址，不混用前后配置。
    pub async fn open_configured_numeric(
        &self,
        address: SocketAddr,
        transport: Transport,
        policy: Policy,
        milliseconds: u64,
    ) -> Result<u64> {
        if address.port() == 0 || address.ip().is_unspecified() {
            return Err(Error::InvalidInput);
        }
        policy.check_support()?;
        let snapshot = self.network_snapshot()?;
        snapshot.check_transport(transport)?;
        if snapshot
            .resolver
            .as_ref()
            .is_some_and(|dns| dns.is_fake_ip(address.ip()))
        {
            return Err(Error::Unsupported);
        }
        Ok(self
            .establish_with(snapshot, duration(milliseconds)?, async |_| {
                dial(address, transport).await
            })
            .await?
            .id)
    }
    async fn establish_with(
        &self,
        snapshot: Arc<NetworkSnapshot>,
        deadline: Duration,
        future: impl AsyncFnOnce(Option<&crate::dns::ResolverOperation>) -> Result<Dialed>,
    ) -> Result<Opened> {
        let configuration = snapshot.clone();
        self.establish_guarded(snapshot, deadline, async |permission| {
            let dialed = future(permission).await?;
            let target = crate::routing::Target {
                domain: None,
                address: Some(dialed.2.ip().to_string()),
                port: dialed.2.port(),
                transport: match &dialed.0 {
                    Socket::Tcp { .. } => Transport::Tcp,
                    _ => Transport::Udp,
                },
            };
            let protection = configuration
                .router
                .as_ref()
                .map(|router| router.protection(target))
                .transpose()?
                .unwrap_or_default();
            Ok((dialed, protection))
        })
        .await
    }
    async fn establish_guarded(
        &self,
        snapshot: Arc<NetworkSnapshot>,
        deadline: Duration,
        future: impl AsyncFnOnce(
            Option<&crate::dns::ResolverOperation>,
        ) -> Result<(Dialed, crate::routing::TrafficProtection)>,
    ) -> Result<Opened> {
        crate::lifecycle::runtime()?;
        snapshot.ensure_active()?;
        let owner = snapshot.owner()?;
        let _session_operation = owner
            .as_ref()
            .map(|owner| owner.enter(owner.identity()))
            .transpose()?;
        let dns_operation = snapshot
            .resolver
            .as_ref()
            .map(|resolver| resolver.operation())
            .transpose()?;
        let revoked = async {
            if let Some(owner) = &owner {
                owner.canceled().await;
            } else {
                std::future::pending::<()>().await;
            }
        };
        let dns_revoked = async {
            if let Some(resolver) = &snapshot.resolver {
                resolver.canceled().await;
            } else {
                std::future::pending::<()>().await;
            }
        };
        let authority = Authority::new()?;
        self.start_reaper()?;
        let mut generation = self.generation.subscribe();
        let epoch = *generation.borrow();
        if self.state.lock().map_err(|_| Error::Internal)?.stopped {
            return Err(Error::InvalidState);
        }
        let ((socket, lease, address), protection) = tokio::select! {
            biased;
            _ = generation.changed() => return Err(Error::Cancelled),
            _ = revoked => return Err(Error::InvalidState),
            _ = dns_revoked => return Err(Error::InvalidState),
            result = timeout(deadline, future(dns_operation.as_ref())) => result.map_err(|_| Error::NetworkTimeout)??,
        };
        resources::reap(&self.state)?;
        let mut state = self.state.lock().map_err(|_| Error::Internal)?;
        if state.stopped || *generation.borrow() != epoch {
            return Err(Error::Cancelled);
        }
        if state.registry.len() >= state.network.limits.maximum_connections {
            return Err(Error::ResourceLimit);
        }
        let transport = match &socket {
            Socket::Tcp { .. } => Transport::Tcp,
            Socket::Udp { .. } | Socket::ProxyUdp { .. } => Transport::Udp,
            Socket::Detached => return Err(Error::Internal),
        };
        let (cancel, _) = watch::channel(false);
        let resource_lease = owner
            .as_ref()
            .map(|owner| owner.enter(owner.identity()))
            .transpose()?;
        let id = snapshot.commit(dns_operation.as_ref(), || {
            protection.check(false)?;
            state
                .registry
                .create(Arc::new(Connection {
                    resources: Mutex::new(Some(Arc::new(SocketResources {
                        socket,
                        _lease: lease,
                        detached: None,
                        _session: resource_lease,
                    }))),
                    authority,
                    owner,
                    cancel,
                    address,
                    transport,
                    revision: snapshot.revision,
                    idle_timeout: Duration::from_millis(snapshot.limits.idle_timeout_ms),
                    usage: Mutex::new(resources::Usage::default()),
                    protection,
                    quic: Mutex::new(crate::protocol::quic::Flow::default()),
                }))
                .ok_or(Error::ResourceLimit)
        })?;
        // 监听 Debug 撤销与到期，主动唤醒挂起读写并释放 Socket，不能只在下一次调用检查。
        if let Some(connection) = state.registry.get(id).cloned()
            && connection.protection.has_debug_grant()
        {
            let mut canceled = connection.cancel.subscribe();
            let state = Arc::downgrade(&self.state);
            crate::lifecycle::runtime()?.spawn(async move {
                tokio::select! {biased; _=canceled.wait_for(|closed|*closed)=>(),result=connection.protection.invalidated()=>{
                    if let Err(error)=result {crate::error::diagnostics::record(error,crate::error::diagnostics::DiagnosticSite::Cleanup);}
                    connection.close();
                }}
                let result=connection.wait_closed().await;
                if let Err(error)=result {crate::error::diagnostics::record(error,crate::error::diagnostics::DiagnosticSite::Cleanup);}
                else if let Some(state)=state.upgrade() {
                    match state.lock() {Ok(mut state)=>{state.registry.take(id);},Err(_)=>crate::error::diagnostics::record(Error::Internal,crate::error::diagnostics::DiagnosticSite::Cleanup)}
                }
            });
        }
        Ok(Opened {
            id,
            address: address.to_string(),
        })
    }
    fn connection(&self, id: u64) -> Result<Arc<Connection>> {
        self.state
            .lock()
            .map_err(|_| Error::Internal)?
            .registry
            .get(id)
            .cloned()
            .ok_or(Error::InvalidState)
    }
    pub async fn write(&self, id: u64, bytes: &[u8], milliseconds: u64) -> Result<usize> {
        if bytes.len() > MAX_BUFFER {
            return Err(Error::ResourceLimit);
        }
        let connection = self.connection(id)?;
        let operation = connection.operation()?;
        let tcp = matches!(&operation.resources.socket, Socket::Tcp { .. });
        if !tcp {
            connection.inspect_datagram(bytes)?;
        }
        let framed = matches!(&operation.resources.socket, Socket::ProxyUdp { .. });
        let result = connection
            .run(milliseconds, async {
                match &operation.resources.socket {
                    Socket::Tcp { writer, .. } => {
                        writer.lock().await.write_all(bytes).await?;
                        Ok(bytes.len())
                    }
                    Socket::Detached => Err(Error::Unsupported),
                    Socket::ProxyUdp { socket } => socket.send(bytes).await,
                    Socket::Udp { socket, .. } => {
                        if bytes.len() > MAX_DATAGRAM {
                            return Err(Error::ResourceLimit);
                        }
                        let sent = socket.send(bytes).await?;
                        if sent != bytes.len() {
                            return Err(Error::Network);
                        }
                        Ok(sent)
                    }
                }
            })
            .await;
        // TCP 部分写入后无法安全重试；超时/网络错误使连接失效，绝不重复发送。
        if (tcp || framed)
            && matches!(
                result,
                Err(Error::Network | Error::NetworkTimeout | Error::Cancelled)
            )
        {
            connection.close();
        }
        if matches!(&operation.resources.socket, Socket::ProxyUdp {socket} if socket.failed()) {
            connection.close();
        }
        if let Ok(length) = result {
            connection.activity(length, true, !tcp)?;
        }
        result
    }
    pub async fn read(&self, id: u64, maximum: usize, milliseconds: u64) -> Result<Read> {
        if maximum == 0 || maximum > MAX_BUFFER {
            return Err(Error::InvalidInput);
        }
        let connection = self.connection(id)?;
        let operation = connection.operation()?;
        let result = connection
            .run(milliseconds, async {
                let udp = matches!(
                    &operation.resources.socket,
                    Socket::Udp { .. } | Socket::ProxyUdp { .. }
                );
                let capacity = if udp { MAX_BUFFER } else { maximum };
                let lease = BUFFERS.reserve(capacity)?;
                let mut bytes = Zeroizing::new(vec![0; capacity]);
                let length = match &operation.resources.socket {
                    Socket::Tcp { reader, .. } => reader.lock().await.read(&mut bytes).await?,
                    Socket::Detached => return Err(Error::Unsupported),
                    Socket::ProxyUdp { socket } => socket.receive(&mut bytes).await?,
                    Socket::Udp { socket, reader } => {
                        let _read = reader.lock().await;
                        socket.recv(&mut bytes).await?
                    }
                };
                // UDP 始终接收整个报文，超过调用者上限时丢弃并报错，不交付截断成功。
                if udp {
                    connection.inspect_datagram(&bytes[..length])?;
                }
                connection.activity(length, false, udp)?;
                if length > maximum {
                    return Err(Error::ResourceLimit);
                }
                bytes.truncate(length);
                Ok(Read {
                    bytes,
                    eof: !udp && length == 0,
                    _lease: lease,
                })
            })
            .await;
        if matches!(&operation.resources.socket, Socket::ProxyUdp {socket} if socket.failed()) {
            connection.close();
        }
        result
    }
    pub async fn shutdown_write(&self, id: u64, milliseconds: u64) -> Result<()> {
        let connection = self.connection(id)?;
        let operation = connection.operation()?;
        connection
            .run(milliseconds, async {
                match &operation.resources.socket {
                    Socket::Tcp { writer, .. } => {
                        writer.lock().await.shutdown().await?;
                        Ok(())
                    }
                    Socket::Udp { .. } | Socket::ProxyUdp { .. } | Socket::Detached => {
                        Err(Error::Unsupported)
                    }
                }
            })
            .await
    }
    fn revoke_owned(&self, owner: Identity) -> Result<Vec<Arc<Connection>>> {
        let mut state = self.state.lock().map_err(|_| Error::Internal)?;
        let keys: Vec<_> = state
            .registry
            .iter()
            .filter(|(_, connection)| {
                connection
                    .owner
                    .as_ref()
                    .is_some_and(|authority| authority.identity() == owner)
            })
            .map(|(id, _)| id)
            .collect();
        let connections: Vec<_> = keys
            .into_iter()
            .filter_map(|id| state.registry.take(id))
            .collect();
        for connection in &connections {
            connection.request_close()?;
        }
        Ok(connections)
    }
    pub(crate) async fn close_owned(&self, owner: Identity) -> Result<()> {
        let connections = self.revoke_owned(owner)?;
        for connection in connections {
            connection.wait_closed().await?;
        }
        Ok(())
    }
    /// 内部幂等取消请求；真实完成仍由独立内核清理任务登记。
    pub(crate) fn release_connection(&self, id: u64) -> Result<()> {
        let connection = self
            .state
            .lock()
            .map_err(|_| Error::Internal)?
            .registry
            .take(id);
        if let Some(connection) = connection {
            connection.request_close()?;
        }
        Ok(())
    }
    pub async fn close(&self, id: u64) -> Result<()> {
        let connection = self
            .state
            .lock()
            .map_err(|_| Error::Internal)?
            .registry
            .take(id)
            .ok_or(Error::InvalidState)?;
        connection.request_close()?;
        connection.wait_closed().await
    }
    pub async fn close_all(&self) -> Result<()> {
        self.clear(false).await
    }
    pub async fn stop(&self) -> Result<()> {
        self.clear(true).await
    }
    async fn clear(&self, stop: bool) -> Result<()> {
        let connections = {
            let mut state = self.state.lock().map_err(|_| Error::Internal)?;
            state.stopped |= stop;
            if stop && let Some(debug) = state.debug.take() {
                debug.close()?;
            }
            let next = (*self.generation.borrow())
                .checked_add(1)
                .ok_or(Error::ResourceLimit)?;
            self.generation.send_replace(next);
            state.registry.take_all()
        };
        for connection in &connections {
            connection.request_close()?;
        }
        for connection in connections {
            connection.wait_closed().await?;
        }
        Ok(())
    }
}
impl Drop for Manager {
    fn drop(&mut self) {
        let connections = match self.state.lock() {
            Ok(mut state) => {
                state.stopped = true;
                state.registry.take_all()
            }
            Err(_) => {
                crate::error::diagnostics::record(
                    Error::Internal,
                    crate::error::diagnostics::DiagnosticSite::Cleanup,
                );
                Vec::new()
            }
        };
        for connection in connections {
            connection.close();
        }
        if let Ok(task) = self.reaper.get_mut()
            && let Some(task) = task.take()
        {
            task.abort();
        }
    }
}

/// DNS 与数字地址路径共用真实连接及配额；竞争失败的 socket 随任务所有权释放。
async fn dial(address: SocketAddr, transport: Transport) -> Result<Dialed> {
    let lease = CONNECTIONS.reserve(1)?;
    let socket = match transport {
        Transport::Tcp => {
            let socket = TcpStream::connect(address).await?;
            socket.set_nodelay(true)?;
            tcp_socket(Box::new(socket))
        }
        Transport::Udp => {
            let bind = if address.is_ipv4() {
                "0.0.0.0:0"
            } else {
                "[::]:0"
            };
            let socket = UdpSocket::bind(bind).await?;
            socket.connect(address).await?;
            Socket::Udp {
                socket,
                reader: tokio::sync::Mutex::new(()),
            }
        }
    };
    Ok((socket, lease, address))
}
mod resolution;
pub use resolution::{DomainTarget, FakeIPTarget, Family, Opened};

mod execution;
pub use execution::{RoutedOpened, RoutedTarget};

mod configuration;
pub use configuration::{Limits, NetworkSnapshot};
mod resources;
pub use resources::{SessionStatus, Status};

mod stream;
pub use stream::ManagedStream;

#[cfg(test)]
#[path = "../tests/test_outbound.rs"]
mod test_outbound;
