//! 只生成并非阻塞输出当前事件。接收方持有队列或本地套接字；Core无日志集合或历史读取。
use crate::{
    configuration::LogFilter,
    error::{Error, ErrorInfo, Result},
};
use serde::{Deserialize, Serialize};
#[cfg(unix)]
use std::os::unix::net::UnixDatagram;
use std::{
    sync::{
        Mutex,
        atomic::{AtomicU8, AtomicU64, Ordering},
        mpsc::{SyncSender, TrySendError},
    },
    time::{Instant, SystemTime, UNIX_EPOCH},
};
pub const MAX_EVENT_BYTES: usize = 4096;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Debug,
    Info,
    Warn,
    Error,
}
impl From<crate::error::LogLevel> for Level {
    fn from(value: crate::error::LogLevel) -> Self {
        match value {
            crate::error::LogLevel::Warn => Self::Warn,
            crate::error::LogLevel::Info => Self::Info,
            crate::error::LogLevel::Error => Self::Error,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Event {
    ConfigurationRequested,
    ConfigurationCommitted,
    ConfigurationFailed,
    VpnPhaseEntered,
    VpnStageFailed,
    VpnCleanupFinished,
    VpnCleanupFailed,
    ConnectionCreated,
    ConnectionEstablished,
    ConnectionFailed,
    ConnectionClosed,
    RouteMatched,
    RouteSelected,
    PolicyBlocked,
    NodeConnecting,
    NodeHandshake,
    NodeConnected,
    NodeFailed,
    NodeRetry,
    CoreStopped,
}
impl Event {
    fn module(self) -> &'static str {
        match self {
            Self::ConfigurationRequested
            | Self::ConfigurationCommitted
            | Self::ConfigurationFailed => "configuration",
            Self::ConnectionCreated
            | Self::ConnectionEstablished
            | Self::ConnectionFailed
            | Self::ConnectionClosed => "connection",
            Self::RouteMatched | Self::RouteSelected | Self::PolicyBlocked => "routing",
            Self::NodeConnecting | Self::NodeHandshake | Self::NodeConnected | Self::NodeFailed => {
                "node"
            }
            Self::NodeRetry => "failover",
            Self::CoreStopped => "core",
            _ => "vpn",
        }
    }
}
/// 原始已验证节点名称，完整传递、不替换或脱敏；固定容量，不接收任意日志正文。
#[derive(Clone, Copy, Debug)]
pub struct NodeName {
    bytes: [u8; 128],
    len: usize,
}
impl NodeName {
    pub(crate) fn new(value: &str) -> Option<Self> {
        if value.len() > 128 {
            return None;
        }
        let mut bytes = [0; 128];
        bytes[..value.len()].copy_from_slice(value.as_bytes());
        Some(Self {
            bytes,
            len: value.len(),
        })
    }
}
impl Serialize for NodeName {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        let value =
            std::str::from_utf8(&self.bytes[..self.len]).map_err(serde::ser::Error::custom)?;
        serializer.serialize_str(value)
    }
}
#[derive(Clone, Copy, Debug, Default, Serialize)]
pub struct Context {
    pub connection_id: Option<u64>,
    pub node: Option<NodeName>,
    pub next_node: Option<NodeName>,
    pub policy: Option<crate::routing::Policy>,
    pub rule_index: Option<usize>,
    pub rule_version: Option<u64>,
    pub rule_source: Option<u16>,
    pub rule_sequence: Option<u32>,
    pub route_reason: Option<crate::routing::Reason>,
    pub uploaded: Option<u64>,
    pub downloaded: Option<u64>,
    pub duration_ms: Option<u64>,
}
impl Context {
    pub(crate) fn route(id: u64, decision: &crate::routing::Decision) -> Self {
        Self {
            connection_id: Some(id),
            policy: Some(decision.policy),
            rule_index: decision.rule_index,
            rule_version: decision.rule_version,
            rule_source: decision.origin.as_ref().map(|v| v.source),
            rule_sequence: decision.origin.as_ref().map(|v| v.sequence),
            route_reason: Some(decision.reason),
            ..Self::default()
        }
    }
}
#[derive(Clone, Copy, Debug, Serialize)]
pub struct Record {
    pub schema_version: u32,
    pub core_version: &'static str,
    pub sequence: u64,
    pub timestamp_unix_ms: Option<u64>,
    pub elapsed_us: u64,
    pub level: Level,
    pub module: &'static str,
    pub event: Event,
    pub transaction: Option<u64>,
    pub revision: Option<u64>,
    pub phase: Option<crate::vpn::transaction::Phase>,
    pub error_info: Option<ErrorInfo>,
    #[serde(flatten)]
    pub context: Context,
}
enum Output {
    // 队列由接收方创建并持有Receiver；Core仅持有try_send端，不提供读取端。
    Channel(SyncSender<Record>),
    #[cfg(unix)]
    Datagram(UnixDatagram),
}
#[derive(Default)]
struct OutputState {
    sink: Option<Output>,
    encrypted: bool,
    cipher: Option<std::sync::Arc<Cipher>>,
}
/// 版本化HPKE exporter记录协议；每条记录独立nonce，丢包不影响后续解密。
const HEADER_BYTES: usize = 85;
const WIRE_MAGIC: &[u8; 8] = b"NCLOG001";
const HPKE_INFO: &[u8] = b"Nodus Core log transport v1";
const EXPORT_LABEL: &[u8] = b"Nodus Core log AES-256-GCM record key v1";
pub(crate) struct Cipher {
    key: ring::aead::LessSafeKey,
    encapsulated: [u8; 65],
    prefix: [u8; 4],
    next_nonce: AtomicU64,
}
impl std::fmt::Debug for Cipher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Cipher { .. }")
    }
}
impl Cipher {
    pub(crate) fn new(public_key: &str) -> Result<Self> {
        use base64::Engine;
        use hpke::{Deserializable, Serializable};
        use ring::rand::SecureRandom;
        type Kem = hpke::kem::DhP256HkdfSha256;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(public_key)
            .map_err(|_| Error::InvalidInput)?;
        if bytes.len() != 65 || bytes.first() != Some(&4) {
            return Err(Error::InvalidInput);
        }
        let recipient =
            <Kem as hpke::Kem>::PublicKey::from_bytes(&bytes).map_err(|_| Error::InvalidInput)?;
        // OS随机数失败明确返回错误；不用unwrap式随机源，不降级为确定性随机数。
        // 使用既有ring随机源驱动HPKE，所有随机请求均可失败由setup边界处理。
        let mut rng = LogRng { failed: false };
        let (enc, context) = hpke::setup_sender::<
            hpke::aead::AesGcm256,
            hpke::kdf::HkdfSha256,
            Kem,
            _,
        >(&hpke::OpModeS::Base, &recipient, HPKE_INFO, &mut rng)
        .map_err(|_| Error::Internal)?;
        if rng.failed {
            return Err(Error::Internal);
        }
        let mut key_bytes = zeroize::Zeroizing::new([0u8; 32]);
        context
            .export(EXPORT_LABEL, key_bytes.as_mut())
            .map_err(|_| Error::Internal)?;
        let key = ring::aead::UnboundKey::new(&ring::aead::AES_256_GCM, key_bytes.as_ref())
            .map_err(|_| Error::Internal)?;
        let mut prefix = [0u8; 4];
        ring::rand::SystemRandom::new()
            .fill(&mut prefix)
            .map_err(|_| Error::Internal)?;
        let mut encapsulated = [0u8; 65];
        encapsulated.copy_from_slice(enc.to_bytes().as_slice());
        Ok(Self {
            key: ring::aead::LessSafeKey::new(key),
            encapsulated,
            prefix,
            next_nonce: AtomicU64::new(1),
        })
    }
    fn seal(&self, plain: &[u8], output: &mut [u8]) -> Result<usize> {
        let size = HEADER_BYTES
            .checked_add(plain.len())
            .and_then(|n| n.checked_add(16))
            .ok_or(Error::ResourceLimit)?;
        if plain.len() > MAX_EVENT_BYTES || size > output.len() {
            return Err(Error::ResourceLimit);
        }
        let counter = self
            .next_nonce
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |v| v.checked_add(1))
            .map_err(|_| Error::ResourceLimit)?;
        let mut nonce = [0u8; 12];
        nonce[..4].copy_from_slice(&self.prefix);
        nonce[4..].copy_from_slice(&counter.to_be_bytes());
        output[..8].copy_from_slice(WIRE_MAGIC);
        output[8..73].copy_from_slice(&self.encapsulated);
        output[73..85].copy_from_slice(&nonce);
        let (header, body) = output[..size].split_at_mut(HEADER_BYTES);
        body[..plain.len()].copy_from_slice(plain);
        let tag = self
            .key
            .seal_in_place_separate_tag(
                ring::aead::Nonce::assume_unique_for_key(nonce),
                ring::aead::Aad::from(header),
                &mut body[..plain.len()],
            )
            .map_err(|_| Error::Internal)?;
        body[plain.len()..].copy_from_slice(tag.as_ref());
        Ok(size)
    }
}
// HPKE的RngCore接口不能返回错误；失败时填零但标记整个候选无效，绝不发布其上下文。
struct LogRng {
    failed: bool,
}
impl hpke::rand_core::RngCore for LogRng {
    fn next_u32(&mut self) -> u32 {
        let mut b = [0; 4];
        self.fill_bytes(&mut b);
        u32::from_le_bytes(b)
    }
    fn next_u64(&mut self) -> u64 {
        let mut b = [0; 8];
        self.fill_bytes(&mut b);
        u64::from_le_bytes(b)
    }
    fn fill_bytes(&mut self, dest: &mut [u8]) {
        use ring::rand::SecureRandom;
        if ring::rand::SystemRandom::new().fill(dest).is_err() {
            self.failed = true;
            dest.fill(0);
        }
    }
}
impl hpke::rand_core::CryptoRng for LogRng {}
pub(crate) struct Journal {
    started: Instant,
    next: AtomicU64,
    filter: AtomicU8,
    output: Mutex<OutputState>,
}
impl Default for Journal {
    fn default() -> Self {
        Self {
            started: Instant::now(),
            next: AtomicU64::new(1),
            filter: AtomicU8::new(0),
            output: Mutex::new(OutputState::default()),
        }
    }
}
fn filter_code(filter: LogFilter) -> u8 {
    match filter {
        LogFilter::Silent => 0,
        LogFilter::Info => 1,
        LogFilter::Debug => 2,
        LogFilter::Warn => 3,
        LogFilter::Error => 4,
    }
}
fn accepts(filter: u8, level: Level) -> bool {
    filter == 1
        || matches!(
            (filter, level),
            (2, Level::Debug) | (3, Level::Warn) | (4, Level::Error)
        )
}
impl Journal {
    pub(crate) fn apply_settings(&self, settings: &crate::configuration::models::Log) {
        // 配置校验已在候选阶段生成加密上下文；提交只交换Arc，不在代理线程生成密钥。
        if let Ok(mut output) = self.output.lock() {
            output.encrypted = settings.encryption;
            output.cipher = settings.cipher.clone();
        }
    }
    pub(crate) fn configure(&self, filter: LogFilter) {
        self.filter.store(filter_code(filter), Ordering::Release);
    }
    pub(crate) fn attach(&self, sender: SyncSender<Record>) -> Result<()> {
        self.output.lock().map_err(|_| Error::Internal)?.sink = Some(Output::Channel(sender));
        Ok(())
    }
    #[cfg(unix)]
    pub(crate) fn attach_unix(&self, socket: UnixDatagram) -> Result<()> {
        socket.peer_addr().map_err(|_| Error::InvalidInput)?;
        socket
            .set_nonblocking(true)
            .map_err(|_| Error::InvalidInput)?;
        self.output.lock().map_err(|_| Error::Internal)?.sink = Some(Output::Datagram(socket));
        Ok(())
    }
    pub(crate) fn detach(&self) -> Result<()> {
        self.output.lock().map_err(|_| Error::Internal)?.sink.take();
        Ok(())
    }
    pub(crate) fn emit(
        &self,
        event: Event,
        transaction: Option<u64>,
        revision: Option<u64>,
        phase: Option<crate::vpn::transaction::Phase>,
        error: Option<Error>,
    ) {
        self.event(
            event,
            transaction,
            revision,
            phase,
            error,
            Context::default(),
        );
    }
    pub(crate) fn event(
        &self,
        event: Event,
        transaction: Option<u64>,
        revision: Option<u64>,
        phase: Option<crate::vpn::transaction::Phase>,
        error: Option<Error>,
        context: Context,
    ) {
        let level = error.map_or(
            if event == Event::ConfigurationRequested {
                Level::Debug
            } else {
                Level::Info
            },
            |e| e.code().level().into(),
        );
        if !accepts(self.filter.load(Ordering::Acquire), level) {
            return;
        }
        // 序号先分配；竞争或接收方背压丢弃时，后续序号缺口可由接收方观察。
        let Ok(sequence) = self
            .next
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |v| v.checked_add(1))
        else {
            return;
        };
        let Ok(output) = self.output.try_lock() else {
            return;
        };
        let Some(sink) = output.sink.as_ref() else {
            return;
        };
        let record = Record {
            schema_version: 1,
            core_version: env!("CARGO_PKG_VERSION"),
            sequence,
            timestamp_unix_ms: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .ok()
                .and_then(|v| u64::try_from(v.as_millis()).ok()),
            elapsed_us: u64::try_from(self.started.elapsed().as_micros()).unwrap_or(u64::MAX),
            level,
            module: event.module(),
            event,
            transaction,
            revision,
            phase,
            error_info: error.map(Error::public_info),
            context,
        };
        match sink {
            Output::Channel(_) if output.encrypted => (),
            Output::Channel(sender) => match sender.try_send(record) {
                Ok(()) | Err(TrySendError::Full(_)) | Err(TrySendError::Disconnected(_)) => (),
            },
            #[cfg(unix)]
            Output::Datagram(socket) => {
                let mut bytes = [0u8; MAX_EVENT_BYTES];
                let mut cursor = std::io::Cursor::new(bytes.as_mut_slice());
                if serde_json::to_writer(&mut cursor, &record).is_ok() {
                    let length = cursor.position() as usize;
                    // 非阻塞数据报整条发送；背压、接收端关闭或编码失败直接放弃当前事件，不重试、不缓存。
                    if output.encrypted {
                        if let Some(cipher) = &output.cipher {
                            let mut packet = [0u8; MAX_EVENT_BYTES + HEADER_BYTES + 16];
                            if let Ok(size) = cipher.seal(&bytes[..length], &mut packet) {
                                let _ = socket.send(&packet[..size]);
                            }
                        }
                    } else {
                        let _ = socket.send(&bytes[..length]);
                    }
                }
            }
        }
    }
}
#[cfg(test)]
#[path = "../tests/test_logging.rs"]
mod test_logging;
