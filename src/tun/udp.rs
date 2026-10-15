//! 有界 UDP 数据包转发会话；不创建系统隧道，不处理 TCP 或分片；仅接管显式 DNS 端点。
use super::{MAX_MTU, MIN_MTU, checksum, parse, pseudo_sum};
use crate::lifecycle::session::Authority;
use crate::{
    budget::{ByteBudget, ByteLease},
    error::{Error, Result},
    outbound::{Family, Manager, RoutedTarget},
    routing::{Target, Transport},
};
use serde::Deserialize;
use std::{
    collections::HashMap,
    net::{IpAddr, SocketAddr},
    sync::{Arc, Mutex},
};
use tokio::sync::{mpsc, watch};
use zeroize::Zeroizing;

static SESSIONS: ByteBudget = ByteBudget::new(8);
static FLOWS: ByteBudget = ByteBudget::new(64);
static PACKETS: ByteBudget = ByteBudget::new(4 * 1024 * 1024);

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Options {
    pub mtu: usize,
    pub maximum_flows: usize,
    pub output_capacity: usize,
    pub timeout_ms: u64,
    pub idle_timeout_ms: u64,
    /// 虚拟 DNS 端点，最多两个；空列表保持普通 UDP 转发。
    #[serde(default)]
    pub dns_endpoints: Vec<SocketAddr>,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            mtu: 1500,
            maximum_flows: 16,
            output_capacity: 32,
            timeout_ms: 3000,
            idle_timeout_ms: 30000,
            dns_endpoints: Vec::new(),
        }
    }
}
impl Options {
    fn validate(&self) -> Result<()> {
        if !(MIN_MTU..=MAX_MTU).contains(&self.mtu)
            || !(1..=32).contains(&self.maximum_flows)
            || !(1..=64).contains(&self.output_capacity)
            || !(1..=60000).contains(&self.timeout_ms)
            || !(1..=60000).contains(&self.idle_timeout_ms)
        {
            return Err(Error::InvalidInput);
        }
        if self.dns_endpoints.len() > 2 {
            return Err(Error::ResourceLimit);
        }
        for (index, endpoint) in self.dns_endpoints.iter().enumerate() {
            if endpoint.port() != 53
                || endpoint.ip().is_unspecified()
                || endpoint.ip().is_multicast()
                || endpoint.ip() == IpAddr::V4(std::net::Ipv4Addr::BROADCAST)
                || self.dns_endpoints[..index].contains(endpoint)
            {
                return Err(Error::InvalidInput);
            }
        }
        Ok(())
    }
}
/// 输出拥有正文和预算；宿主消费/释放后归还，不借用输入包内存。
pub struct Packet {
    pub bytes: Zeroizing<Vec<u8>>,
    _lease: ByteLease,
}
#[derive(Clone, Copy, Eq, PartialEq, Hash)]
struct Key {
    source: SocketAddr,
    destination: SocketAddr,
    generation: Option<u64>,
}
struct Flow {
    id: u64,
    task: tokio::task::AbortHandle,
}
struct State {
    closed: bool,
    flows: HashMap<Key, Flow>,
    lease: Option<ByteLease>,
}
struct ConnectionLease {
    manager: Arc<Manager>,
    id: u64,
    _lease: ByteLease,
}
impl Drop for ConnectionLease {
    fn drop(&mut self) {
        let _ = crate::error::diagnostics::observe(
            self.manager.release_connection(self.id),
            crate::error::diagnostics::DiagnosticSite::Cleanup,
        );
    }
}
/// 每会话只允许一个提交在途；读返回包可并发，关闭不等待提交锁。
pub struct Session {
    inner: Arc<SessionInner>,
}
struct SessionInner {
    options: Options,
    manager: Arc<Manager>,
    dns: Option<Arc<crate::dns::Resolver>>,
    state: Mutex<State>,
    submit_lock: tokio::sync::Mutex<()>,
    sender: mpsc::Sender<Result<Packet>>,
    receiver: Mutex<mpsc::Receiver<Result<Packet>>>,
    cancel: watch::Sender<bool>,
    authority: Arc<Authority>,
    tasks: Mutex<tokio::task::JoinSet<()>>,
}
impl SessionInner {
    pub fn new(manager: Arc<Manager>, options: Options) -> Result<Self> {
        options.validate()?;
        crate::lifecycle::runtime()?;
        let authority = Authority::new()?;
        let dns = if options.dns_endpoints.is_empty() {
            None
        } else {
            Some(
                manager
                    .prepare_tun_snapshot()?
                    .resolver
                    .clone()
                    .ok_or(Error::InvalidState)?,
            )
        };
        // DNS 地址不能落入合成池，避免后续映射分配与 DNS 接管端点碰撞。
        if dns.as_ref().is_some_and(|resolver| {
            options
                .dns_endpoints
                .iter()
                .any(|endpoint| resolver.is_fake_ip(endpoint.ip()))
        }) {
            return Err(Error::InvalidInput);
        }
        let lease = SESSIONS.reserve(1)?;
        if let Some(resolver) = &dns {
            resolver.bind_tun_fake_ip(&authority)?;
        }
        let (sender, receiver) = mpsc::channel(options.output_capacity);
        let (cancel, _) = watch::channel(false);
        Ok(Self {
            options,
            manager,
            dns,
            state: Mutex::new(State {
                closed: false,
                flows: HashMap::new(),
                lease: Some(lease),
            }),
            submit_lock: tokio::sync::Mutex::new(()),
            sender,
            receiver: Mutex::new(receiver),
            cancel,
            authority,
            tasks: Mutex::new(tokio::task::JoinSet::new()),
        })
    }
    async fn submit(&self, bytes: &[u8], generation: Option<u64>) -> Result<()> {
        let _operation = self.authority.enter(self.authority.identity())?;
        tokio::select! {
            biased;
            _ = self.authority.canceled() => Err(Error::Cancelled),
            result = self.submit_inner(bytes, generation) => result,
        }
    }
    async fn submit_inner(&self, bytes: &[u8], generation: Option<u64>) -> Result<()> {
        let _submission = self
            .submit_lock
            .try_lock()
            .map_err(|_| Error::ResourceLimit)?;
        let info = parse(bytes, self.options.mtu)?;
        if info.protocol != 17 {
            return Err(Error::Unsupported);
        }
        let key = Key {
            source: SocketAddr::new(
                info.source.parse().map_err(|_| Error::InvalidInput)?,
                info.source_port,
            ),
            destination: SocketAddr::new(
                info.destination.parse().map_err(|_| Error::InvalidInput)?,
                info.destination_port,
            ),
            generation,
        };
        for endpoint in [key.source, key.destination] {
            if endpoint.port() == 0
                || endpoint.ip().is_unspecified()
                || endpoint.ip().is_multicast()
                || endpoint.ip() == IpAddr::V4(std::net::Ipv4Addr::BROADCAST)
            {
                return Err(Error::InvalidInput);
            }
        }
        let payload = &bytes[bytes.len() - info.payload_bytes..];
        let mut cancellation = self.cancel.subscribe();
        let existing = {
            let mut state = self.state.lock().map_err(|_| Error::Internal)?;
            if state.closed {
                return Err(Error::InvalidState);
            }
            state.flows.retain(|_, flow| !flow.task.is_finished());
            state.flows.get(&key).map(|flow| flow.id)
        };
        // 一次捕获配置；不能在校验映射后再读取另一份解析器。
        let snapshot = Arc::new(
            self.manager
                .network_snapshot()?
                .for_session(self.authority.clone()),
        );
        if self.options.dns_endpoints.contains(&key.destination) {
            if generation.is_some() {
                return Err(Error::InvalidInput);
            }
            let resolver = self.bound_dns(&snapshot)?;
            snapshot.check_transport(Transport::Udp)?;
            // 先占用有界输出槽；队列已满时不发起查询或分配映射。
            let slot = self
                .sender
                .try_reserve()
                .map_err(|_| Error::ResourceLimit)?;
            let response = tokio::select! {
                biased;
                _ = cancellation.changed() => return Err(Error::Cancelled),
                result = resolver.respond(payload, true) => result?,
            };
            self.bound_dns(self.manager.network_snapshot()?.as_ref())?;
            if self.state.lock().map_err(|_| Error::Internal)?.closed {
                return Err(Error::Cancelled);
            }
            slot.send(encode(
                key.destination,
                key.source,
                &response,
                self.options.mtu,
            ));
            return Ok(());
        }
        let mut mapping_generation = generation;
        if existing.is_none()
            && let Some(resolver) = &self.dns
            && resolver.is_fake_ip(key.destination.ip())
        {
            self.bound_dns(&snapshot)?;
            let owned = resolver.fake_ip_generation().ok_or(Error::InvalidState)?;
            if generation.is_some_and(|value| value != owned) {
                return Err(Error::InvalidState);
            }
            // 流的键保留宿主输入；自动代次只用于本次连接恢复。
            mapping_generation = Some(owned);
        }
        let id = if let Some(id) = existing {
            id
        } else {
            if self.state.lock().map_err(|_| Error::Internal)?.flows.len()
                >= self.options.maximum_flows
            {
                return Err(Error::ResourceLimit);
            }
            let lease = FLOWS.reserve(1)?;
            let request = RoutedTarget {
                target: Target {
                    domain: None,
                    address: Some(key.destination.ip().to_string()),
                    port: key.destination.port(),
                    transport: Transport::Udp,
                },
                timeout_ms: self.options.timeout_ms,
                family: Family::Auto,
                mapping_generation,
            };
            let opened = tokio::select! {
                biased;
                _ = cancellation.changed() => return Err(Error::Cancelled),
                result = self.manager.connect_configured_snapshot(request, snapshot) => result?,
            };
            let connection = ConnectionLease {
                manager: Arc::clone(&self.manager),
                id: opened.connection.id,
                _lease: lease,
            };
            let mut state = self.state.lock().map_err(|_| Error::Internal)?;
            if state.closed {
                return Err(Error::Cancelled);
            }
            let id = connection.id;
            let task = self.tasks.lock().map_err(|_| Error::Internal)?.spawn_on(
                receive_loop(
                    connection,
                    key,
                    self.options.clone(),
                    self.sender.clone(),
                    self.cancel.subscribe(),
                ),
                crate::lifecycle::runtime()?.handle(),
            );
            state.flows.insert(key, Flow { id, task });
            id
        };
        let result = tokio::select! {
            biased;
            _ = cancellation.changed() => Err(Error::Cancelled),
            result = self.manager.write(id, payload, self.options.timeout_ms) => result.map(|_| ()),
        };
        if result.is_err()
            && let Some(flow) = self
                .state
                .lock()
                .map_err(|_| Error::Internal)?
                .flows
                .remove(&key)
        {
            flow.task.abort();
            let _ = crate::error::diagnostics::observe(
                self.manager.release_connection(flow.id),
                crate::error::diagnostics::DiagnosticSite::Cleanup,
            );
        }
        result
    }
    /// 解析器所有者必须一致；配置替换后旧会话不能猜测新代次。
    fn bound_dns(
        &self,
        snapshot: &crate::outbound::NetworkSnapshot,
    ) -> Result<&Arc<crate::dns::Resolver>> {
        let owned = self.dns.as_ref().ok_or(Error::InvalidState)?;
        if !snapshot
            .resolver
            .as_ref()
            .is_some_and(|current| owned.identity() == current.identity())
        {
            return Err(Error::InvalidState);
        }
        owned.ensure_active()?;
        Ok(owned)
    }
    /// 非阻塞取出一个包；异步网络错误同样返回，不能伪装为队列为空。
    pub fn receive(&self) -> Result<Option<Packet>> {
        let _operation = self.authority.enter(self.authority.identity())?;
        if self.state.lock().map_err(|_| Error::Internal)?.closed {
            return Err(Error::InvalidState);
        }
        match self
            .receiver
            .lock()
            .map_err(|_| Error::Internal)?
            .try_recv()
        {
            Ok(result) => result.map(Some),
            Err(mpsc::error::TryRecvError::Empty) => Ok(None),
            Err(mpsc::error::TryRecvError::Disconnected) => Err(Error::InvalidState),
        }
    }
    fn request_close(self: &Arc<Self>) -> Result<()> {
        if self.authority.revoke()? {
            self.cancel.send_replace(true);
            let own = self.clone();
            crate::lifecycle::runtime()?.spawn(async move {
                let result = own.cleanup().await;
                let _ = crate::error::diagnostics::observe(
                    own.authority.complete(result),
                    crate::error::diagnostics::DiagnosticSite::Cleanup,
                );
            });
        }
        Ok(())
    }
    async fn cleanup(&self) -> Result<()> {
        self.state.lock().map_err(|_| Error::Internal)?.closed = true;
        self.tasks.lock().map_err(|_| Error::Internal)?.abort_all();
        // 会话归属连接在内核注册表登记，不能只清理已经进入 Flow 表的连接。
        self.manager.close_owned(self.authority.identity()).await?;
        if let Some(resolver) = &self.dns {
            resolver.close().await?;
        }
        self.authority.wait_idle().await?;
        loop {
            let result = std::future::poll_fn(|context| match self.tasks.lock() {
                Ok(mut tasks) => tasks.poll_join_next(context).map(Ok),
                Err(_) => std::task::Poll::Ready(Err(Error::Internal)),
            })
            .await?;
            match result {
                Some(result) => crate::error::diagnostics::observe_join(
                    result,
                    crate::error::diagnostics::DiagnosticSite::Cleanup,
                ),
                None => break,
            }
        }
        let mut receiver = self.receiver.lock().map_err(|_| Error::Internal)?;
        receiver.close();
        while receiver.try_recv().is_ok() {}
        let mut state = self.state.lock().map_err(|_| Error::Internal)?;
        state.flows.clear();
        state.lease.take();
        Ok(())
    }
}
impl Session {
    pub fn new(manager: Arc<Manager>, options: Options) -> Result<Self> {
        Ok(Self {
            inner: Arc::new(SessionInner::new(manager, options)?),
        })
    }
    pub async fn submit(&self, bytes: &[u8], generation: Option<u64>) -> Result<()> {
        self.inner.submit(bytes, generation).await
    }
    pub fn receive(&self) -> Result<Option<Packet>> {
        self.inner.receive()
    }
    pub(crate) fn same_identity(&self, other: &Self) -> bool {
        self.inner.authority.identity() == other.inner.authority.identity()
    }
    /// 关闭由内核托管；重复或并发等待共享完成结果，旧对象引用不会阻止释放。
    pub async fn close(&self) -> Result<()> {
        self.inner.request_close()?;
        self.inner.authority.wait_closed().await
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        // 兜底仅提交关闭意图；明确 close().await 才能观察清理完成。
        let _ = crate::error::diagnostics::observe(
            self.inner.request_close(),
            crate::error::diagnostics::DiagnosticSite::Cleanup,
        );
    }
}

async fn receive_loop(
    connection: ConnectionLease,
    key: Key,
    options: Options,
    sender: mpsc::Sender<Result<Packet>>,
    mut cancel: watch::Receiver<bool>,
) {
    let header = if key.source.is_ipv4() { 28 } else { 48 };
    loop {
        let result = tokio::select! {
            biased;
            _ = cancel.changed() => return,
            result = connection.manager.read(connection.id, options.mtu - header, options.idle_timeout_ms) => result,
        };
        let packet = match result {
            Ok(read) => encode(key.destination, key.source, &read.bytes, options.mtu),
            Err(Error::NetworkTimeout | Error::Cancelled) => return,
            Err(error) => Err(error),
        };
        let failed = packet.is_err();
        tokio::select! {
            biased;
            _ = cancel.changed() => return,
            result = sender.send(packet) => if result.is_err() { return; },
        }
        if failed {
            return;
        }
    }
}
/// 构造无分片 UDP/IP 包；校验和与解析器共用工具，超 MTU 整包拒绝。
pub fn encode(
    source: SocketAddr,
    destination: SocketAddr,
    payload: &[u8],
    mtu: usize,
) -> Result<Packet> {
    let ip = match (source.ip(), destination.ip()) {
        (IpAddr::V4(_), IpAddr::V4(_)) => 20,
        (IpAddr::V6(_), IpAddr::V6(_)) => 40,
        _ => return Err(Error::InvalidInput),
    };
    let size = payload
        .len()
        .checked_add(ip + 8)
        .ok_or(Error::ResourceLimit)?;
    if !(MIN_MTU..=MAX_MTU).contains(&mtu) || source.port() == 0 || destination.port() == 0 {
        return Err(Error::InvalidInput);
    }
    if size > mtu {
        return Err(Error::ResourceLimit);
    }
    let lease = PACKETS.reserve(size)?;
    let mut bytes = Zeroizing::new(vec![0; size]);
    match (source.ip(), destination.ip()) {
        (IpAddr::V4(src), IpAddr::V4(dst)) => {
            bytes[0] = 0x45;
            bytes[2..4].copy_from_slice(&(size as u16).to_be_bytes());
            bytes[6] = 0x40;
            bytes[8] = 64;
            bytes[9] = 17;
            bytes[12..16].copy_from_slice(&src.octets());
            bytes[16..20].copy_from_slice(&dst.octets());
            let sum = checksum(&bytes[..20], 0);
            bytes[10..12].copy_from_slice(&sum.to_be_bytes());
        }
        (IpAddr::V6(src), IpAddr::V6(dst)) => {
            bytes[0] = 0x60;
            bytes[4..6].copy_from_slice(&((size - 40) as u16).to_be_bytes());
            bytes[6] = 17;
            bytes[7] = 64;
            bytes[8..24].copy_from_slice(&src.octets());
            bytes[24..40].copy_from_slice(&dst.octets());
        }
        _ => return Err(Error::InvalidInput),
    }
    bytes[ip..ip + 2].copy_from_slice(&source.port().to_be_bytes());
    bytes[ip + 2..ip + 4].copy_from_slice(&destination.port().to_be_bytes());
    bytes[ip + 4..ip + 6].copy_from_slice(&((size - ip) as u16).to_be_bytes());
    bytes[ip + 8..].copy_from_slice(payload);
    let sum = checksum(
        &bytes[ip..],
        pseudo_sum(&bytes, if ip == 20 { 4 } else { 6 }, 17, size - ip),
    );
    bytes[ip + 6..ip + 8].copy_from_slice(&(if sum == 0 { u16::MAX } else { sum }).to_be_bytes());
    Ok(Packet {
        bytes,
        _lease: lease,
    })
}

#[cfg(test)]
#[path = "../../tests/test_tun_udp.rs"]
mod test_tun_udp;
