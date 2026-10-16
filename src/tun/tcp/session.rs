//! 每 TCP 流一个有界工作任务；共用路由出站，输入排队不产生逐包任务。
use super::{
    state::{Phase, State as TcpState},
    wire::{self, Flags},
};
use crate::{
    budget::{ByteBudget, ByteLease},
    dns::Resolver,
    error::{Error, Result},
    outbound::{Family, Manager, NetworkSnapshot, RoutedTarget},
    routing::{Target, Transport},
    tun::{Packet, udp},
};
use ring::rand::{SecureRandom, SystemRandom};
use serde::Deserialize;
use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::sync::{mpsc, watch};
use zeroize::Zeroizing;
// 每流：双 32 KiB 环、4 KiB 位图、发送记录、DNS 原响应和分帧副本及临时正文。
// 合计不足 256 KiB；最多 32 条，包队列另受共享 4 MiB 总预算约束。
static MEMORY: ByteBudget = ByteBudget::new(8 * 1024 * 1024);
const FLOW_MEMORY: usize = 256 * 1024;
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Options {
    pub maximum_flows: usize,
    pub input_capacity: usize,
    pub idle_timeout_ms: u64,
    pub time_wait_ms: u64,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            maximum_flows: 16,
            input_capacity: 8,
            idle_timeout_ms: 30000,
            time_wait_ms: 30000,
        }
    }
}
impl Options {
    pub fn validate(&self) -> Result<()> {
        if !(1..=32).contains(&self.maximum_flows)
            || !(1..=16).contains(&self.input_capacity)
            || !(100..=300000).contains(&self.idle_timeout_ms)
            || !(1..=120000).contains(&self.time_wait_ms)
        {
            return Err(Error::InvalidInput);
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Hash, Eq, PartialEq)]
struct Key {
    source: SocketAddr,
    destination: SocketAddr,
}
struct Flow {
    sender: mpsc::Sender<Packet>,
    task: tokio::task::AbortHandle,
    generation: Option<u64>,
}
struct State {
    closed: bool,
    flows: HashMap<Key, Flow>,
}
pub struct Session {
    manager: Arc<Manager>,
    owner: Arc<udp::Session>,
    options: Options,
    state: Mutex<State>,
    sender: mpsc::Sender<Result<Packet>>,
    receiver: Mutex<mpsc::Receiver<Result<Packet>>>,
}
impl Session {
    pub fn new(owner: Arc<udp::Session>, options: Options) -> Result<Self> {
        let manager = owner.manager();
        options.validate()?;
        let (sender, receiver) = mpsc::channel(owner.options().output_capacity);
        Ok(Self {
            manager,
            owner,
            options,
            state: Mutex::new(State {
                closed: false,
                flows: HashMap::new(),
            }),
            sender,
            receiver: Mutex::new(receiver),
        })
    }
    pub fn submit(&self, bytes: &[u8], generation: Option<u64>) -> Result<()> {
        let authority = self.owner.authority();
        let _submission = authority.enter(authority.identity())?;
        let segment = wire::decode(bytes, self.owner.options().mtu)?;
        let key = Key {
            source: segment.source,
            destination: segment.destination,
        };
        let mut state = self.state.lock().map_err(|_| Error::Internal)?;
        if state.closed {
            return Err(Error::InvalidState);
        }
        state.flows.retain(|_, flow| !flow.task.is_finished());
        if let Some(flow) = state.flows.get(&key) {
            if generation.is_some() && generation != flow.generation {
                return Err(Error::InvalidState);
            }
            let slot = flow.sender.try_reserve().map_err(|error| match error {
                mpsc::error::TrySendError::Full(_) => Error::ResourceLimit,
                mpsc::error::TrySendError::Closed(_) => Error::InvalidState,
            })?;
            slot.send(Packet::copy(bytes)?);
            return Ok(());
        }
        if !segment.flags.has(Flags::SYN)
            || segment.flags.has(Flags::ACK | Flags::FIN | Flags::RST)
            || !segment.payload.is_empty()
        {
            return Err(Error::InvalidState);
        }
        if state.flows.len() >= self.options.maximum_flows {
            return Err(Error::ResourceLimit);
        }
        // WS 协商后接收环从 32 KiB 扩至 128 KiB，位图增加 12 KiB；
        // 每流预留增至 384 KiB，全局仍为 8 MiB，不因协商绕过预算。
        let lease = MEMORY.reserve(if segment.window_scale.is_some() {
            384 * 1024
        } else {
            FLOW_MEMORY
        })?;
        let (snapshot, generation) = self.owner.route_context(key.destination.ip(), generation)?;
        snapshot.check_transport(Transport::Tcp)?;
        let dns = self.owner.dns_context(key.destination)?;
        let (sender, receiver) = mpsc::channel(self.options.input_capacity);
        let context = Context {
            manager: self.manager.clone(),
            owner: self.owner.clone(),
            options: self.options.clone(),
            key,
            snapshot: Some(snapshot),
            generation,
            dns,
            sender: self.sender.clone(),
            cancel: self.owner.cancellation(),
            _operation: authority.enter(authority.identity())?,
            _lease: lease,
        };
        let first = Packet::copy(bytes)?;
        let task = self.owner.spawn(run_flow(context, first, receiver))?;
        state.flows.insert(
            key,
            Flow {
                sender,
                task,
                generation,
            },
        );
        Ok(())
    }
    pub fn receive(&self) -> Result<Option<Packet>> {
        let authority = self.owner.authority();
        let _operation = authority.enter(authority.identity())?;
        if self.state.lock().map_err(|_| Error::Internal)?.closed {
            return Err(Error::InvalidState);
        }
        match self
            .receiver
            .lock()
            .map_err(|_| Error::Internal)?
            .try_recv()
        {
            Ok(packet) => packet.map(Some),
            Err(mpsc::error::TryRecvError::Empty) => Ok(None),
            Err(_) => Err(Error::InvalidState),
        }
    }
    /// 共享所有者撤销并等待真实任务、连接和解析器释放，不能把 abort 当作完成。
    pub async fn close(&self) -> Result<()> {
        self.owner.close().await?;
        let mut state = self.state.lock().map_err(|_| Error::Internal)?;
        state.closed = true;
        state.flows.clear();
        let mut receiver = self.receiver.lock().map_err(|_| Error::Internal)?;
        receiver.close();
        while receiver.try_recv().is_ok() {}
        Ok(())
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        let _ = crate::error::diagnostics::observe(
            self.owner.request_close(),
            crate::error::diagnostics::DiagnosticSite::Cleanup,
        );
    }
}

struct Context {
    manager: Arc<Manager>,
    owner: Arc<udp::Session>,
    options: Options,
    key: Key,
    snapshot: Option<Arc<NetworkSnapshot>>,
    generation: Option<u64>,
    dns: Option<Arc<Resolver>>,
    sender: mpsc::Sender<Result<Packet>>,
    cancel: watch::Receiver<bool>,
    _lease: ByteLease,
    _operation: crate::lifecycle::session::Operation,
}
struct Connection {
    manager: Arc<Manager>,
    id: u64,
}
impl Drop for Connection {
    fn drop(&mut self) {
        let _ = crate::error::diagnostics::observe(
            self.manager.release_connection(self.id),
            crate::error::diagnostics::DiagnosticSite::Cleanup,
        );
    }
}
async fn send(context: &mut Context, packet: Result<Packet>) -> Result<()> {
    tokio::select! { biased; _ = context.cancel.changed() => Err(Error::Cancelled), result = tokio::time::timeout(Duration::from_millis(context.options.idle_timeout_ms), context.sender.send(packet)) => result.map_err(|_| Error::NetworkTimeout)?.map_err(|_| Error::Cancelled) }
}
async fn run_flow(mut context: Context, first: Packet, mut input: mpsc::Receiver<Packet>) {
    let result = async {
        let syn = wire::decode(&first.bytes, context.owner.options().mtu)?;
        let mut random = [0; 4];
        SystemRandom::new()
            .fill(&mut random)
            .map_err(|_| Error::Internal)?;
        let mut tcp = TcpState::new(
            &syn,
            u32::from_be_bytes(random),
            context.owner.options().mtu,
            Duration::from_millis(context.options.time_wait_ms),
            Instant::now(),
        )?;
        let result = relay(&mut context, &mut tcp, &mut input).await;
        if let Err(error) = result
            && error != Error::Cancelled
            && tcp.phase() != Phase::Closed
        {
            let _ = send(&mut context, Ok(tcp.reset()?)).await;
        }
        result
    }
    .await;
    if let Err(error) = result
        && error != Error::Cancelled
    {
        let _ = send(&mut context, Err(error)).await;
    }
}
async fn relay(
    context: &mut Context,
    tcp: &mut TcpState,
    input: &mut mpsc::Receiver<Packet>,
) -> Result<()> {
    let snapshot = context.snapshot.take().ok_or(Error::InvalidState)?;
    let mut connection = if context.dns.is_some() {
        drop(snapshot);
        None
    } else {
        let request = RoutedTarget {
            target: Target {
                domain: None,
                address: Some(context.key.destination.ip().to_string()),
                port: context.key.destination.port(),
                transport: Transport::Tcp,
            },
            timeout_ms: context.owner.options().timeout_ms,
            family: Family::Auto,
            mapping_generation: context.generation,
        };
        // 握手硬期限也覆盖真实出站准备，不能在长连接超时配置下无限延后首次 SYN-ACK。
        let opened = tokio::select! {
            biased;
            _ = context.cancel.changed() => return Err(Error::Cancelled),
            result = tokio::time::timeout_at(
                tokio::time::Instant::from_std(tcp.handshake_deadline()),
                context.manager.connect_configured_snapshot(request, snapshot),
            ) => result.map_err(|_| Error::NetworkTimeout)?,
        };
        match opened {
            Ok(opened) => Some(Connection {
                manager: context.manager.clone(),
                id: opened.connection.id,
            }),
            Err(error) => {
                return Err(error);
            }
        }
    };
    let timeout = context.owner.options().timeout_ms;
    let mut activity = Instant::now();
    let mut write_closed = false;
    let mut response: Option<(Zeroizing<Vec<u8>>, usize)> = None;
    loop {
        let now = Instant::now();
        if now.duration_since(activity) >= Duration::from_millis(context.options.idle_timeout_ms)
            && tcp.phase() != Phase::TimeWait
        {
            return Err(Error::NetworkTimeout);
        }
        for _ in 0..4 {
            let packet = match tcp.emit(now) {
                Ok(Some(packet)) => packet,
                Ok(None) => break,
                Err(error) => {
                    return Err(error);
                }
            };
            send(context, Ok(packet)).await?;
        }
        if tcp.phase() == Phase::Closed {
            return Ok(());
        }
        if let Some(connection) = &connection {
            if tcp.pending_input() != 0 {
                let bytes = tcp.input(16384);
                tokio::select! { biased; _ = context.cancel.changed() => return Err(Error::Cancelled), result = context.manager.write(connection.id, &bytes, timeout) => { result?; } }
                tcp.consume_input(bytes.len())?;
                activity = Instant::now();
                continue;
            }
            if tcp.input_finished() && !write_closed {
                tokio::select! { biased; _ = context.cancel.changed() => return Err(Error::Cancelled), result = context.manager.shutdown_write(connection.id, timeout) => { result?; } }
                write_closed = true;
            }
        } else if context.dns.is_some() {
            if let Some((bytes, offset)) = &mut response {
                let count = tcp.output_capacity().min(bytes.len() - *offset).min(16384);
                if count != 0 {
                    tcp.output(&bytes[*offset..*offset + count])?;
                    *offset += count;
                    activity = Instant::now();
                }
                if *offset == bytes.len() {
                    response = None;
                }
            }
            if response.is_none() && tcp.pending_input() >= 2 {
                let length = tcp.input(2);
                let length = usize::from(u16::from_be_bytes([length[0], length[1]]));
                if !(12..=4096).contains(&length) {
                    return Err(Error::InvalidInput);
                }
                if tcp.pending_input() >= length + 2 {
                    context.owner.check_dns_owner()?;
                    let query = tcp.input(length + 2);
                    let resolver = context.dns.as_ref().ok_or(Error::Internal)?;
                    let bytes = tokio::select! { biased; _ = context.cancel.changed() => return Err(Error::Cancelled), result = resolver.respond(&query[2..], false) => result? };
                    context.owner.check_dns_owner()?;
                    if bytes.len() > u16::MAX as usize {
                        return Err(Error::ResourceLimit);
                    }
                    let mut framed = Zeroizing::new(Vec::with_capacity(bytes.len() + 2));
                    framed.extend_from_slice(&(bytes.len() as u16).to_be_bytes());
                    framed.extend_from_slice(&bytes);
                    tcp.consume_input(length + 2)?;
                    response = Some((framed, 0));
                    activity = Instant::now();
                    continue;
                }
            }
            if tcp.input_finished() && response.is_none() {
                tcp.output_finished();
            }
        }
        if tcp.phase() == Phase::TimeWait && tcp.input_finished() && write_closed {
            connection.take();
        }
        let now = Instant::now();
        let idle = activity + Duration::from_millis(context.options.idle_timeout_ms);
        let deadline = tcp
            .deadline(now)
            .unwrap_or(idle)
            .min(if tcp.phase() == Phase::TimeWait {
                now + Duration::from_secs(120)
            } else {
                idle
            });
        let capacity = tcp.output_capacity().min(16384);
        tokio::select! {
            biased;
            _ = context.cancel.changed() => return Err(Error::Cancelled),
            packet = input.recv() => {
                let packet = packet.ok_or(Error::Cancelled)?; let segment = wire::decode(&packet.bytes, context.owner.options().mtu)?;
                tcp.process(&segment, Instant::now())?;
                // 未完成握手的任意入包不能刷新空闲期限，防止半开流永久占额。
                if tcp.phase() != Phase::SynReceived { activity = Instant::now(); }
            },
            read = async { context.manager.read(connection.as_ref().ok_or(Error::Internal)?.id, capacity, timeout).await }, if connection.is_some() && capacity != 0 => {
                match read { Ok(read) => { if read.eof { tcp.output_finished(); } else { tcp.output(&read.bytes)?; } activity = Instant::now(); }, Err(Error::NetworkTimeout) => {}, Err(error) => { return Err(error); } }
            },
            _ = tokio::time::sleep_until(tokio::time::Instant::from_std(deadline)) => {}
        }
    }
}

#[cfg(test)]
#[path = "../../../tests/test_tun_tcp_session.rs"]
mod test_tun_tcp_session;
