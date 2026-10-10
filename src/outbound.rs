//! 共用数字地址出站：TCP 字节流与连接式 UDP，拒绝和未实现代理不触网。
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
enum Socket {
    Detached,
    Tcp {
        reader: tokio::sync::Mutex<tokio::net::tcp::OwnedReadHalf>,
        writer: tokio::sync::Mutex<tokio::net::tcp::OwnedWriteHalf>,
    },
    Udp {
        socket: UdpSocket,
        reader: tokio::sync::Mutex<()>,
    },
}
type Dialed = (Socket, ByteLease, SocketAddr);
struct Connection {
    socket: Socket,
    cancel: watch::Sender<bool>,
    _lease: ByteLease,
    address: SocketAddr,
    transport: Transport,
    revision: u64,
    idle_timeout: Duration,
    usage: Mutex<resources::Usage>,
}
impl Connection {
    fn close(&self) {
        self.cancel.send_replace(true);
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
        Ok(self
            .establish(duration(milliseconds)?, dial(address, transport))
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
        if snapshot
            .resolver
            .as_ref()
            .is_some_and(|dns| dns.is_fake_ip(address.ip()))
        {
            return Err(Error::Unsupported);
        }
        Ok(self
            .establish_with(snapshot, duration(milliseconds)?, dial(address, transport))
            .await?
            .id)
    }
    async fn establish(
        &self,
        deadline: Duration,
        future: impl std::future::Future<Output = Result<Dialed>>,
    ) -> Result<Opened> {
        self.establish_with(self.network_snapshot()?, deadline, future)
            .await
    }
    async fn establish_with(
        &self,
        snapshot: Arc<NetworkSnapshot>,
        deadline: Duration,
        future: impl std::future::Future<Output = Result<Dialed>>,
    ) -> Result<Opened> {
        self.start_reaper()?;
        let mut generation = self.generation.subscribe();
        let epoch = *generation.borrow();
        if self.state.lock().map_err(|_| Error::Internal)?.stopped {
            return Err(Error::InvalidState);
        }
        let (socket, lease, address) = tokio::select! {
            biased;
            _ = generation.changed() => return Err(Error::Cancelled),
            result = timeout(deadline, future) => result.map_err(|_| Error::NetworkTimeout)??,
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
            Socket::Udp { .. } => Transport::Udp,
            Socket::Detached => return Err(Error::Internal),
        };
        let (cancel, _) = watch::channel(false);
        let id = state
            .registry
            .create(Arc::new(Connection {
                socket,
                cancel,
                _lease: lease,
                address,
                transport,
                revision: snapshot.revision,
                idle_timeout: Duration::from_millis(snapshot.limits.idle_timeout_ms),
                usage: Mutex::new(resources::Usage::default()),
            }))
            .ok_or(Error::ResourceLimit)?;
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
        let tcp = matches!(&connection.socket, Socket::Tcp { .. });
        let result = connection
            .run(milliseconds, async {
                match &connection.socket {
                    Socket::Tcp { writer, .. } => {
                        writer.lock().await.write_all(bytes).await?;
                        Ok(bytes.len())
                    }
                    Socket::Detached => Err(Error::Unsupported),
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
        if tcp
            && matches!(
                result,
                Err(Error::Network | Error::NetworkTimeout | Error::Cancelled)
            )
        {
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
        connection
            .run(milliseconds, async {
                let udp = matches!(&connection.socket, Socket::Udp { .. });
                let capacity = if udp { MAX_BUFFER } else { maximum };
                let lease = BUFFERS.reserve(capacity)?;
                let mut bytes = Zeroizing::new(vec![0; capacity]);
                let length = match &connection.socket {
                    Socket::Tcp { reader, .. } => reader.lock().await.read(&mut bytes).await?,
                    Socket::Detached => return Err(Error::Unsupported),
                    Socket::Udp { socket, reader } => {
                        let _read = reader.lock().await;
                        socket.recv(&mut bytes).await?
                    }
                };
                // UDP 始终接收整个报文，超过调用者上限时丢弃并报错，不交付截断成功。
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
            .await
    }
    pub async fn shutdown_write(&self, id: u64, milliseconds: u64) -> Result<()> {
        let connection = self.connection(id)?;
        connection
            .run(milliseconds, async {
                match &connection.socket {
                    Socket::Tcp { writer, .. } => {
                        writer.lock().await.shutdown().await?;
                        Ok(())
                    }
                    Socket::Udp { .. } | Socket::Detached => Err(Error::Unsupported),
                }
            })
            .await
    }
    pub fn close(&self, id: u64) -> Result<()> {
        let connection = self
            .state
            .lock()
            .map_err(|_| Error::Internal)?
            .registry
            .take(id)
            .ok_or(Error::InvalidState)?;
        connection.close();
        Ok(())
    }
    pub fn close_all(&self) -> Result<()> {
        self.clear(false)
    }
    pub fn stop(&self) -> Result<()> {
        self.clear(true)
    }
    fn clear(&self, stop: bool) -> Result<()> {
        let connections = {
            let mut state = self.state.lock().map_err(|_| Error::Internal)?;
            state.stopped |= stop;
            let next = (*self.generation.borrow())
                .checked_add(1)
                .ok_or(Error::ResourceLimit)?;
            self.generation.send_replace(next);
            state.registry.take_all()
        };
        for connection in connections {
            connection.close();
        }
        Ok(())
    }
}
impl Drop for Manager {
    fn drop(&mut self) {
        let _ = self.stop();
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
            let (reader, writer) = socket.into_split();
            Socket::Tcp {
                reader: tokio::sync::Mutex::new(reader),
                writer: tokio::sync::Mutex::new(writer),
            }
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
