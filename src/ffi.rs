//! C ABI 只借用调用期内的缓冲区；Rust 返回资源句柄，由统一释放入口回收。
use crate::budget::{ByteBudget, ByteLease};
use crate::mitm::{
    MitmProxy,
    authority::Authority,
    capture::{Capture, CaptureStore},
};
use crate::{
    error::{AbiStatus, Error, ErrorCode, ErrorInfo, NO_ERROR_CODE},
    http::{Header, MAX_BODY},
    https::HttpsClient,
    lifecycle::Registry,
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use rustls::pki_types::{CertificateDer, pem::PemObject};
use serde::{Deserialize, Serialize};
use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
};
use tokio::runtime::Runtime;
use zeroize::Zeroizing;

const OK: i32 = AbiStatus::Success.value();
const INVALID_HANDLE: i32 = ErrorCode::InvalidHandle.value() as i32;
const UNSUPPORTED: i32 = ErrorCode::Unsupported.value() as i32;
const INTERNAL_ERROR: i32 = ErrorCode::Internal.value() as i32;
const MAX_JSON: usize = 12 * 1024 * 1024;
const MAX_OUTPUT: usize = 24 * 1024 * 1024;
const MAX_OUTPUT_MEMORY: usize = 32 * 1024 * 1024;
const SMALL_OUTPUT: usize = 256 * 1024;
const READ_REPLY_OVERHEAD: usize = 4096;
static OUTPUT: ByteBudget = ByteBudget::new(MAX_OUTPUT_MEMORY);
#[derive(Clone)]
enum ManagedTun {
    Udp(Arc<crate::tun::udp::Session>),
    Mixed(Arc<crate::tun::session::Session>),
}
impl ManagedTun {
    async fn submit(&self, bytes: &[u8], generation: Option<u64>) -> Result<(), Error> {
        match self {
            Self::Udp(session) => session.submit(bytes, generation).await,
            Self::Mixed(session) => session.submit(bytes, generation).await,
        }
    }
    fn receive(&self) -> Result<Option<crate::tun::Packet>, Error> {
        match self {
            Self::Udp(session) => session.receive(),
            Self::Mixed(session) => session.receive(),
        }
    }
    fn same_identity(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Udp(a), Self::Udp(b)) => a.same_identity(b),
            (Self::Mixed(a), Self::Mixed(b)) => a.same_identity(b),
            _ => false,
        }
    }
    async fn close(&self) -> Result<(), Error> {
        match self {
            Self::Udp(session) => session.close().await,
            Self::Mixed(session) => session.close().await,
        }
    }
}
#[derive(Default)]
struct Engine {
    #[cfg(unix)]
    debug_ipc: Mutex<Option<crate::routing::debug::ipc::Service>>,
    client: Mutex<Option<HttpsClient>>,
    geoip: Mutex<Arc<crate::geoip::Service>>,
    busy: AtomicBool,
    outbound: Arc<crate::outbound::Manager>,
    dns_service: Mutex<Option<crate::dns::service::Service>>,
    packet_session: Mutex<Option<crate::tun::PacketSession>>,
    tun_udp: Mutex<Option<ManagedTun>>,
    authority: Mutex<Option<Arc<Authority>>>,
    proxy: Mutex<Option<MitmProxy>>,
    captures: Arc<CaptureStore>,
    runtime_rules: Mutex<Option<crate::routing::background::Service>>,
}
impl Engine {
    /// 关闭过程中保留登记项；并发关闭等待同一内核结果，新开启不能越过 Closing。
    fn close_tun(&self) -> Result<(), Error> {
        let current = self.tun_udp.lock().map_err(|_| Error::Internal)?.clone();
        if let Some(session) = current {
            let closing = session.clone();
            run_network(async move { closing.close().await })?;
            let mut slot = self.tun_udp.lock().map_err(|_| Error::Internal)?;
            if slot
                .as_ref()
                .is_some_and(|active| active.same_identity(&session))
            {
                slot.take();
            }
        }
        Ok(())
    }
    /// 不移走仍在关闭的监听对象；并发停止等待真实完成后再清除登记。
    fn close_dns_service(&self) -> Result<crate::dns::service::Status, Error> {
        let mut slot = self.dns_service.lock().map_err(|_| Error::Internal)?;
        let mut status = slot
            .as_ref()
            .map(|service| service.status())
            .unwrap_or_default();
        if let Some(service) = slot.as_mut() {
            runtime()?.block_on(service.stop())?;
        }
        slot.take();
        status.running = false;
        Ok(status)
    }
    fn client(&self) -> Result<HttpsClient, Error> {
        let mut client = self.client.lock().map_err(|_| Error::Internal)?;
        if client.is_none() {
            *client = Some(HttpsClient::system()?.with_network(Arc::clone(&self.outbound)));
        }
        client.clone().ok_or(Error::Internal)
    }
    fn router(&self) -> Result<Arc<crate::routing::Router>, Error> {
        self.outbound
            .network_snapshot()?
            .router
            .clone()
            .ok_or(Error::InvalidState)
    }
    fn dns(&self) -> Result<Arc<crate::dns::Resolver>, Error> {
        self.outbound
            .network_snapshot()?
            .resolver
            .clone()
            .ok_or(Error::InvalidState)
    }
    fn authority(&self) -> Result<Arc<Authority>, Error> {
        let mut authority = self.authority.lock().map_err(|_| Error::Internal)?;
        if authority.is_none() {
            *authority = Some(Arc::new(Authority::generate()?));
        }
        authority.as_ref().map(Arc::clone).ok_or(Error::Internal)
    }
}
enum Resource {
    #[cfg(unix)]
    DebugClient(Arc<debug::Handle>),
    PendingOutput,
    Engine(Arc<Engine>),
    Buffer(Arc<OwnedBuffer>),
}
struct OwnedBuffer {
    bytes: Zeroizing<Vec<u8>>,
    _lease: ByteLease,
}
impl std::ops::Deref for OwnedBuffer {
    type Target = Vec<u8>;
    fn deref(&self) -> &Self::Target {
        &self.bytes
    }
}
fn registry() -> &'static Mutex<Registry<Resource>> {
    static REGISTRY: OnceLock<Mutex<Registry<Resource>>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(Registry::default()))
}
fn runtime() -> Result<&'static Runtime, Error> {
    crate::lifecycle::runtime()
}

fn engine(handle: u64) -> Result<Arc<Engine>, Error> {
    match registry().lock().map_err(|_| Error::Internal)?.get(handle) {
        Some(Resource::Engine(engine)) => Ok(Arc::clone(engine)),
        _ => Err(Error::InvalidState),
    }
}
fn checked_buffer(handle: u64) -> Result<Arc<OwnedBuffer>, ErrorInfo> {
    match registry()
        .lock()
        .map_err(|_| Error::Internal.public_info())?
        .get(handle)
    {
        Some(Resource::Buffer(buffer)) => Ok(Arc::clone(buffer)),
        _ => Err(ErrorInfo::new(ErrorCode::InvalidHandle)),
    }
}
#[cfg(test)]
fn buffer(handle: u64) -> Option<Arc<OwnedBuffer>> {
    checked_buffer(handle).ok()
}
/// 在执行命令前占住响应名额、预算和缓冲区；失败及取消由守卫统一释放。
struct OutputReservation {
    handle: Option<u64>,
    lease: Option<ByteLease>,
    bytes: Zeroizing<Vec<u8>>,
    limit: usize,
}
impl OutputReservation {
    fn new(limit: usize) -> Result<Self, Error> {
        if limit > MAX_OUTPUT {
            return Err(Error::ResourceLimit);
        }
        let lease = OUTPUT.reserve(limit)?;
        let mut bytes = Zeroizing::new(Vec::new());
        bytes
            .try_reserve_exact(limit)
            .map_err(|_| Error::ResourceLimit)?;
        if bytes.capacity() > limit {
            return Err(Error::ResourceLimit);
        }
        let handle = registry()
            .lock()
            .map_err(|_| Error::Internal)?
            .create(Resource::PendingOutput)
            .ok_or(Error::ResourceLimit)?;
        Ok(Self {
            handle: Some(handle),
            lease: Some(lease),
            bytes,
            limit,
        })
    }
    fn finish(mut self) -> Result<u64, Error> {
        self.bytes.shrink_to_fit();
        let mut lease = self.lease.take().ok_or(Error::Internal)?;
        lease.shrink_to(self.bytes.capacity());
        let resource = Resource::Buffer(Arc::new(OwnedBuffer {
            bytes: std::mem::take(&mut self.bytes),
            _lease: lease,
        }));
        let handle = self.handle.ok_or(Error::Internal)?;
        let mut registry = registry().lock().map_err(|_| Error::Internal)?;
        let slot = registry.get_mut(handle).ok_or(Error::Internal)?;
        if !matches!(slot, Resource::PendingOutput) {
            return Err(Error::Internal);
        }
        *slot = resource;
        self.handle = None;
        Ok(handle)
    }
}
impl std::io::Write for OutputReservation {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if self
            .bytes
            .len()
            .checked_add(bytes.len())
            .is_none_or(|n| n > self.limit)
        {
            return Err(std::io::Error::other(Error::ResourceLimit));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
impl Drop for OutputReservation {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            match registry().lock() {
                Ok(mut registry) => {
                    registry.take(handle);
                }
                Err(_) => crate::error::diagnostics::record(
                    Error::Internal,
                    crate::error::diagnostics::DiagnosticSite::Cleanup,
                ),
            }
        }
    }
}
/// 小命令不预留大正文预算；TCP 读取按请求上限预留 base64 与固定元数据。
fn output_limit(input: &[u8]) -> usize {
    #[derive(Deserialize)]
    struct Hint {
        operation: String,
        maximum: Option<usize>,
    }
    match serde_json::from_slice::<Hint>(input) {
        Ok(hint) if hint.operation == "outbound_read" => {
            let maximum = hint.maximum.unwrap_or(0).min(crate::outbound::MAX_BUFFER);
            maximum.div_ceil(3) * 4 + READ_REPLY_OVERHEAD
        }
        Ok(hint)
            if matches!(
                hint.operation.as_str(),
                "request"
                    | "https"
                    | "subscription_fetch"
                    | "subscription_rules"
                    | "script"
                    | "mitm_peek_capture"
            ) =>
        {
            MAX_OUTPUT
        }
        _ => SMALL_OUTPUT,
    }
}
#[cfg(test)]
fn store(bytes: Vec<u8>) -> Option<u64> {
    use std::io::Write;
    let mut output = OutputReservation::new(bytes.len()).ok()?;
    output.write_all(&bytes).ok()?;
    output.finish().ok()
}
#[derive(Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
enum Command {
    GeoipOpen {},
    GeoipLookup {
        address: String,
        language: String,
    },
    GeoipClose {},
    GeoipConfigure {
        geoip_url: Option<String>,
    },
    GeoipRefresh {},
    PublicIp {
        ipv6: bool,
    },
    SubscriptionSchedule(crate::subscription::Check),
    SubscriptionRules {
        #[serde(default)]
        validate_only: bool,
        yaml: String,
        policy: crate::routing::Policy,
        outbound: Option<String>,
    },
    SubscriptionFetch {
        url: String,
    },
    NetworkConfigure {
        #[serde(default)]
        geoip_url: Option<String>,
        dns: Option<crate::dns::Config>,
        routing: crate::routing::Config,
        limits: crate::outbound::Limits,
        #[serde(default)]
        outbounds: Vec<crate::protocol::config::Profile>,
    },
    OutboundLimits(crate::outbound::Limits),
    OutboundStatus,
    OutboundProbe {
        host: String,
        port: u16,
        timeout_ms: u64,
    },
    OutboundConnect(crate::outbound::DomainTarget),
    OutboundRoute(crate::outbound::RoutedTarget),
    OutboundConnectFakeIp(crate::outbound::FakeIPTarget),
    OutboundFakeIp {
        domain: String,
        ipv6: bool,
        policy: crate::routing::Policy,
    },
    OutboundOpen {
        address: String,
        transport: crate::routing::Transport,
        policy: crate::routing::Policy,
        timeout_ms: u64,
    },
    OutboundWrite {
        id: u64,
        body: String,
        timeout_ms: u64,
    },
    OutboundRead {
        id: u64,
        maximum: usize,
        timeout_ms: u64,
    },
    OutboundShutdownWrite {
        id: u64,
        timeout_ms: u64,
    },
    OutboundClose {
        id: u64,
    },
    OutboundCloseAll,
    DnsConfigure(crate::dns::Config),
    DnsValidate(crate::dns::Config),
    RuntimeRulesOpen {
        directory: String,
    },
    RuntimeRulesStatus,
    RuntimeRulesBegin {
        settings: crate::routing::Config,
        sources: Vec<crate::routing::runtime::SourceInput>,
    },
    RuntimeRulesBeginFile {
        path: String,
    },
    RuntimeRulesJob {
        id: u64,
    },
    RuntimeRulesCancel {
        id: u64,
    },
    RuntimeRulesClose,
    RouteConfigure(crate::routing::Config),
    RouteEvaluate(crate::routing::Target),
    RouteCheck(crate::routing::Target),
    RouteClose,
    DnsClose,
    DnsServiceStart,
    DnsServiceStop,
    DnsServiceStatus,
    DnsQuery(crate::dns::Query),
    DnsLastQuery,
    DnsClearCache,
    DnsFakeIpReverse {
        address: String,
    },
    DnsFakeIpReset,
    PacketOpen {
        mtu: usize,
    },
    PacketInspect {
        packet: String,
    },
    PacketClose,
    TunStart,
    TunUdpOpen(crate::tun::udp::Options),
    TunUdpClose,
    TunOpen(crate::tun::session::Options),
    TunClose,
    Script {
        source: String,
        input: serde_json::Value,
    },
    Https(RequestInput),
    Request(RequestInput),
    MitmCertificate,
    MitmExportIdentity,
    MitmRestoreIdentity {
        certificate_pem: String,
        private_key_pem: String,
    },
    MitmStart {
        port: u16,
        hosts: Vec<String>,
        #[serde(default)]
        capture_bodies: bool,
        #[serde(default)]
        allow_lan: bool,
        #[serde(default)]
        upstream_proxy_port: Option<u16>,
        #[serde(default)]
        rewrites: Vec<crate::rewrite::Rewrite>,
    },
    MitmStop,
    MitmPeekCapture,
    MitmAcknowledgeCapture {
        id: u64,
    },
    AddTrustedRoot {
        certificate_pem: String,
    },
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RequestInput {
    method: String,
    url: String,
    #[serde(default)]
    headers: Vec<Header>,
    #[serde(default)]
    body: String,
    #[serde(default)]
    proxy_port: Option<u16>,
}
struct Secret(Zeroizing<String>);
impl serde::Serialize for Secret {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}
#[derive(Serialize)]
struct RuntimeRulesSummary {
    report_verification: crate::routing::runtime::ReportVerification,
    snapshot_handle: u64,
    version: u64,
    original: usize,
    duplicates: usize,
    conflicts: usize,
    pruned: usize,
    effective: usize,
    directory_synced: Option<bool>,
}
impl RuntimeRulesSummary {
    fn new(snapshot: &crate::routing::runtime::Snapshot, synced: Option<bool>) -> Self {
        let document = snapshot.document();
        let report = &document.report;
        Self {
            report_verification: snapshot.report_verification(),
            snapshot_handle: snapshot.instance_handle(),
            version: document.version,
            original: report.original,
            duplicates: report.duplicates,
            conflicts: report.conflicts,
            pruned: report.pruned,
            effective: report.effective,
            directory_synced: synced,
        }
    }
}
#[derive(Default, Serialize)]
struct Reply {
    #[serde(skip_serializing_if = "Option::is_none")]
    warn: Option<crate::geoip::Warning>,
    #[serde(skip_serializing_if = "Option::is_none")]
    level: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    geoip_status: Option<crate::geoip::Status>,
    #[serde(skip_serializing_if = "Option::is_none")]
    address: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    geoip: Option<crate::geoip::Location>,
    #[serde(skip_serializing_if = "Option::is_none")]
    subscription_schedule: Option<crate::subscription::Decision>,
    #[serde(skip_serializing_if = "Option::is_none")]
    subscription_rules: Option<Vec<crate::routing::Rule>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    subscription_rules_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    network_revision: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    runtime_rules: Option<RuntimeRulesSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    runtime_rules_job: Option<crate::routing::background::Job>,
    #[serde(skip_serializing_if = "Option::is_none")]
    outbound_status: Option<crate::outbound::Status>,
    #[serde(skip_serializing_if = "Option::is_none")]
    outbound_id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    outbound_peer: Option<String>,
    dns_generation: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    outbound_eof: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    outbound_written: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    dns_answer: Option<crate::dns::Answer>,
    #[serde(skip_serializing_if = "Option::is_none")]
    dns_observation: Option<crate::dns::observation::Observation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    dns_service_status: Option<crate::dns::service::Status>,
    #[serde(skip_serializing_if = "Option::is_none")]
    fake_ip_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    packet_info: Option<crate::tun::PacketInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    route_decision: Option<crate::routing::Decision>,
    success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error_info: Option<ErrorInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    status: Option<u16>,
    headers: Vec<Header>,
    body: String,
    duration_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    port: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    certificate_pem: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    certificate_der: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    private_key_pem: Option<Secret>,
    #[serde(skip_serializing_if = "Option::is_none")]
    capture: Option<Capture>,
    dropped_captures: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    script_result_json: Option<String>,
}
impl From<crate::https::Response> for Reply {
    fn from(response: crate::https::Response) -> Self {
        Self {
            success: true,
            status: Some(response.status),
            headers: response.headers,
            body: STANDARD.encode(response.body),
            duration_ms: response.duration_ms,
            ..Self::default()
        }
    }
}
impl Reply {
    fn dns_result(answer: crate::dns::Answer, generation: Option<u64>) -> Self {
        Self {
            success: true,
            dns_answer: Some(answer),
            dns_generation: generation,
            ..Self::default()
        }
    }
    fn dns(resolver: &crate::dns::Resolver, query: crate::dns::Query) -> Result<Self, Error> {
        Ok(Self::dns_result(
            runtime()?.block_on(resolver.resolve(query))?,
            resolver.fake_ip_generation(),
        ))
    }
}
impl From<crate::outbound::Opened> for Reply {
    fn from(opened: crate::outbound::Opened) -> Self {
        Self {
            success: true,
            outbound_id: Some(opened.id),
            outbound_peer: Some(opened.address),
            ..Self::default()
        }
    }
}
impl From<crate::outbound::RoutedOpened> for Reply {
    fn from(opened: crate::outbound::RoutedOpened) -> Self {
        let mut reply = Self::from(opened.connection);
        reply.route_decision = Some(opened.decision);
        reply
    }
}
impl Reply {
    fn failure(error: Error) -> Self {
        crate::error::diagnostics::record(error, crate::error::diagnostics::DiagnosticSite::Abi);
        set_last_error(error.public_info());
        Self {
            // 所有命令提供机器错误；文本仅作诊断，不参与分类。
            error_info: Some(error.public_info()),
            level: Some("error"),
            ..Self::default()
        }
    }
}
impl From<Error> for Reply {
    fn from(error: Error) -> Self {
        Self::failure(error)
    }
}
// 每线程单独保存同步 ABI 的最后失败，不持有业务对象；查询不会清除结果。
thread_local! {
    static LAST_ERROR: std::cell::Cell<Option<ErrorInfo>> = const { std::cell::Cell::new(None) };
}
fn clear_last_error() {
    LAST_ERROR.set(None);
}
fn set_last_error(info: ErrorInfo) {
    LAST_ERROR.set(Some(info));
}
fn boundary_error(code: ErrorCode) -> u64 {
    set_last_error(ErrorInfo::new(code));
    0
}
fn boundary_failure(error: Error) -> u64 {
    set_last_error(error.public_info());
    0
}
fn error_status(error: Error) -> i32 {
    set_last_error(error.public_info());
    tun_status(error)
}
struct BusyGuard<'a>(&'a AtomicBool);
impl Drop for BusyGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}
/// 大型 DNS/路由 future 在有界 Rust 工作线程轮询，不消耗宿主 GCD 的小栈。
/// 命令请求配额仍约束任务数；同步等到结果，异常退出时中止子任务，不能脱离所有者。
struct NetworkTask<T>(tokio::task::JoinHandle<Result<T, Error>>);
impl<T> Drop for NetworkTask<T> {
    fn drop(&mut self) {
        self.0.abort();
    }
}
/// 同步 FFI 关闭不得嵌套启动 Tokio；无异步资源时可立即完成。
/// Rust 异步线程存在待释放资源时返回状态错误，不能阻塞该线程或伪造关闭完成。
fn finish_shutdown<F: std::future::Future<Output = Result<(), Error>>>(
    future: F,
) -> Result<(), Error> {
    if tokio::runtime::Handle::try_current().is_ok() {
        let mut future = std::pin::pin!(future);
        let mut context = std::task::Context::from_waker(std::task::Waker::noop());
        match future.as_mut().poll(&mut context) {
            std::task::Poll::Ready(result) => result,
            std::task::Poll::Pending => Err(Error::InvalidState),
        }
    } else {
        runtime()?.block_on(future)
    }
}
fn run_network<F, T>(future: F) -> Result<T, Error>
where
    F: std::future::Future<Output = Result<T, Error>> + Send + 'static,
    T: Send + 'static,
{
    let executor = runtime()?;
    let mut task = NetworkTask(executor.spawn(future));
    executor
        .block_on(&mut task.0)
        .map_err(|_| Error::Internal)?
}
// 将大型异步 future 的构造隔离到独立栈帧，避免增加每个 FFI 请求的调用栈占用。
#[inline(never)]
fn public_ip(engine: &Engine, ipv6: bool) -> Result<Reply, Error> {
    let address = runtime()?.block_on(crate::geoip::current_address(&engine.client()?, ipv6))?;
    Ok(Reply {
        success: true,
        address: Some(address),
        ..Reply::default()
    })
}
#[inline(never)]
fn probe_latency(engine: &Engine, host: &str, port: u16, timeout_ms: u64) -> Result<Reply, Error> {
    let duration_ms = runtime()?.block_on(engine.outbound.probe_latency(host, port, timeout_ms))?;
    Ok(Reply {
        success: true,
        duration_ms,
        ..Reply::default()
    })
}
// 外部宿主工作线程的栈可能很小；网络 future 由已有、有界的 Rust 运行时执行。
// 普通 FFI 请求配额仍限制为八个，不为每个数据包创建任务。
#[inline(never)]
fn execute_request(engine: &Engine, input: RequestInput, https_only: bool) -> Result<Reply, Error> {
    if https_only && !input.url.starts_with("https://") {
        return Err(Error::InvalidInput);
    }
    let body = Zeroizing::new(
        STANDARD
            .decode(input.body)
            .map_err(|_| Error::InvalidInput)?,
    );
    if body.len() > MAX_BODY {
        return Err(Error::ResourceLimit);
    }
    let client = engine.client()?;
    let executor = runtime()?;
    let response = executor.block_on(async move {
        executor
            .spawn(async move {
                client
                    .execute_request(
                        &input.method,
                        &input.url,
                        &input.headers,
                        &body,
                        input.proxy_port,
                    )
                    .await
            })
            .await
            .map_err(|_| Error::Internal)?
    })?;
    Ok(Reply::from(response))
}
fn command(handle: u64, bytes: &[u8]) -> Result<Reply, Error> {
    let engine = engine(handle)?;
    // 小型取消命令不等待普通请求配额，否则八个阻塞读可能阻止停止；输入仍有界。
    if bytes.len() <= 256 {
        match serde_json::from_slice::<Command>(bytes) {
            Ok(Command::TunUdpClose | Command::TunClose) => {
                engine.close_tun()?;
                return Ok(Reply {
                    success: true,
                    ..Reply::default()
                });
            }
            Ok(Command::OutboundClose { id }) => {
                runtime()?.block_on(engine.outbound.close(id))?;
                return Ok(Reply {
                    success: true,
                    ..Reply::default()
                });
            }
            Ok(Command::RuntimeRulesCancel { id }) => {
                let guard = engine.runtime_rules.lock().map_err(|_| Error::Internal)?;
                guard.as_ref().ok_or(Error::InvalidState)?.cancel(id)?;
                return Ok(Reply {
                    success: true,
                    ..Reply::default()
                });
            }
            Ok(Command::OutboundCloseAll) => {
                runtime()?.block_on(engine.outbound.close_all())?;
                return Ok(Reply {
                    success: true,
                    ..Reply::default()
                });
            }
            _ => {}
        }
    }
    static REQUESTS: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(8);
    let _permit = REQUESTS.try_acquire().map_err(|_| Error::ResourceLimit)?;
    let command: Command = serde_json::from_slice(bytes).map_err(|_| Error::InvalidInput)?;
    // 出站连接自身管理读写锁与取消；允许 TCP 同时读写，其余业务保持原有排他。
    let concurrent = matches!(
        &command,
        Command::OutboundOpen { .. }
            | Command::OutboundConnect(_)
            | Command::OutboundRoute(_)
            | Command::OutboundConnectFakeIp(_)
            | Command::OutboundRead { .. }
            | Command::OutboundWrite { .. }
            | Command::OutboundShutdownWrite { .. }
            | Command::OutboundClose { .. }
            | Command::OutboundCloseAll
            | Command::OutboundStatus
            | Command::RuntimeRulesJob { .. }
            | Command::RuntimeRulesCancel { .. }
            | Command::RuntimeRulesStatus
            | Command::RouteEvaluate(_)
            | Command::RouteCheck(_)
            | Command::SubscriptionSchedule(_)
    );
    let _busy = if concurrent {
        None
    } else {
        if engine
            .busy
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            return Err(Error::InvalidState);
        }
        Some(BusyGuard(&engine.busy))
    };
    let checked_route = matches!(&command, Command::RouteCheck(_));
    match command {
        Command::SubscriptionRules {
            yaml,
            policy,
            outbound,
            validate_only,
        } => {
            let rules = crate::subscription::parse_rules(&yaml, policy, outbound)?;
            Ok(Reply {
                success: true,
                subscription_rules_count: Some(rules.len()),
                subscription_rules: if validate_only { None } else { Some(rules) },
                ..Reply::default()
            })
        }
        Command::SubscriptionFetch { url } => Ok(Reply::from(
            runtime()?.block_on(crate::subscription::fetch(&engine.client()?, &url))?,
        )),
        Command::OutboundProbe {
            host,
            port,
            timeout_ms,
        } => probe_latency(&engine, &host, port, timeout_ms),
        Command::PublicIp { ipv6 } => public_ip(&engine, ipv6),
        Command::GeoipLookup { address, language } => {
            let service = Arc::clone(&*engine.geoip.lock().map_err(|_| Error::Internal)?);
            if service.is_disabled() {
                return Ok(Reply {
                    success: true,
                    warn: service.warning(),
                    geoip_status: Some(crate::geoip::Status::disabled()),
                    ..Reply::default()
                });
            }
            let database = service.database()?;
            let address = if address.parse::<std::net::IpAddr>().is_ok() {
                address
            } else {
                let name = crate::network::normalize_host(&address)?;
                let resolver = engine.dns().map_err(crate::geoip::query_error)?;
                let mut selected = None;
                for kind in ["A", "AAAA"] {
                    let answer = runtime()?
                        .block_on(resolver.resolve_real(crate::dns::Query {
                            name: name.clone(),
                            query_type: kind.into(),
                        }))
                        .map_err(crate::geoip::query_error)?;
                    if answer.rcode != 0 {
                        return Err(crate::geoip::query_error(Error::DnsResponse));
                    }
                    if let Some(value) = answer.addresses.into_iter().next() {
                        selected = Some(value);
                        break;
                    }
                }
                selected
                    .ok_or(Error::DnsResponse)
                    .map_err(crate::geoip::query_error)?
            };
            let location = database.lookup(&address, &language)?;
            Ok(Reply {
                success: true,
                geoip_status: Some(if location.is_some() {
                    crate::geoip::Status::ready()
                } else {
                    crate::geoip::Status::no_match()
                }),
                geoip: location,
                ..Reply::default()
            })
        }
        Command::GeoipClose {} => {
            engine.geoip.lock().map_err(|_| Error::Internal)?.close()?;
            Ok(Reply {
                success: true,
                ..Reply::default()
            })
        }
        Command::GeoipConfigure { geoip_url } => {
            let service = Arc::new(crate::geoip::Service::new(crate::geoip::Config {
                geoip_url,
            })?);
            let warn = service.warning();
            *engine.geoip.lock().map_err(|_| Error::Internal)? = service;
            Ok(Reply {
                success: true,
                geoip_status: warn.as_ref().map(|_| crate::geoip::Status::disabled()),
                warn,
                ..Reply::default()
            })
        }
        Command::GeoipOpen {} | Command::GeoipRefresh {} => {
            let service = Arc::clone(&*engine.geoip.lock().map_err(|_| Error::Internal)?);
            // 先检查来源，不为无配置路径初始化证书或联网。
            if service.is_disabled() {
                return Ok(Reply {
                    success: true,
                    warn: service.warning(),
                    geoip_status: Some(crate::geoip::Status::disabled()),
                    ..Reply::default()
                });
            }
            let client = engine.client()?;
            run_network(async move { service.refresh(&client).await })?;
            Ok(Reply {
                success: true,
                geoip_status: Some(crate::geoip::Status::ready()),
                ..Reply::default()
            })
        }
        Command::SubscriptionSchedule(check) => Ok(Reply {
            success: true,
            subscription_schedule: Some(check.evaluate()?),
            ..Reply::default()
        }),
        Command::NetworkConfigure {
            geoip_url,
            dns,
            routing,
            limits,
            outbounds,
        } => {
            if engine
                .dns_service
                .lock()
                .map_err(|_| Error::Internal)?
                .is_some()
            {
                return Err(Error::InvalidState);
            }
            limits.validate()?;
            let geoip = Arc::new(crate::geoip::Service::new(crate::geoip::Config {
                geoip_url,
            })?);
            let warn = geoip.warning();
            let router = Arc::new(crate::routing::Router::compile(routing)?);
            let resolver = dns
                .map(|config| {
                    crate::dns::Resolver::with_tls_config(config, engine.client()?.tls_config())
                        .map(Arc::new)
                })
                .transpose()?;
            // 来源锁与网络快照提交按一次命令顺序更新；下载不会持有此锁。
            let mut geoip_guard = engine.geoip.lock().map_err(|_| Error::Internal)?;
            let revision = engine.outbound.configure_network_with_outbounds(
                resolver,
                Some(router),
                limits,
                outbounds,
            )?;
            *geoip_guard = geoip;
            Ok(Reply {
                success: true,
                geoip_status: warn.as_ref().map(|_| crate::geoip::Status::disabled()),
                warn,
                network_revision: Some(revision),
                ..Reply::default()
            })
        }
        Command::OutboundLimits(limits) => {
            let revision = engine.outbound.configure_limits(limits)?;
            Ok(Reply {
                success: true,
                network_revision: Some(revision),
                ..Reply::default()
            })
        }
        Command::OutboundStatus => Ok(Reply {
            success: true,
            outbound_status: Some(engine.outbound.status()?),
            ..Reply::default()
        }),
        Command::OutboundRoute(target) => {
            let outbound = engine.outbound.clone();
            let opened = run_network(async move { outbound.connect_configured(target).await })?;
            Ok(Reply::from(opened))
        }
        Command::OutboundFakeIp {
            domain,
            ipv6,
            policy,
        } => {
            policy.check_support()?;
            let resolver = engine.dns()?;
            let query = crate::dns::Query {
                name: domain,
                query_type: if ipv6 { "AAAA" } else { "A" }.into(),
            };
            Ok(Reply::dns_result(
                resolver.allocate_fake_ip(query)?,
                resolver.fake_ip_generation(),
            ))
        }
        Command::OutboundConnectFakeIp(target) => {
            target.policy.check_support()?;
            let outbound = engine.outbound.clone();
            let opened =
                run_network(async move { outbound.connect_configured_fake_ip(target).await })?;
            Ok(Reply::from(opened))
        }
        Command::OutboundConnect(target) => {
            target.policy.check_support()?;
            let outbound = engine.outbound.clone();
            let opened =
                run_network(async move { outbound.connect_configured_domain(target).await })?;
            Ok(Reply::from(opened))
        }
        Command::OutboundOpen {
            address,
            transport,
            policy,
            timeout_ms,
        } => {
            policy.check_support()?;
            let address = crate::outbound::endpoint(&address)?;
            let id = runtime()?.block_on(
                engine
                    .outbound
                    .open_configured_numeric(address, transport, policy, timeout_ms),
            )?;
            Ok(Reply {
                success: true,
                outbound_id: Some(id),
                ..Reply::default()
            })
        }
        Command::OutboundWrite {
            id,
            body,
            timeout_ms,
        } => {
            if body.len() > 87384 {
                return Err(Error::ResourceLimit);
            }
            let bytes = Zeroizing::new(STANDARD.decode(body).map_err(|_| Error::InvalidInput)?);
            let written = runtime()?.block_on(engine.outbound.write(id, &bytes, timeout_ms))?;
            Ok(Reply {
                success: true,
                outbound_written: Some(written),
                ..Reply::default()
            })
        }
        Command::OutboundRead {
            id,
            maximum,
            timeout_ms,
        } => {
            let read = runtime()?.block_on(engine.outbound.read(id, maximum, timeout_ms))?;
            Ok(Reply {
                success: true,
                body: STANDARD.encode(&read.bytes),
                outbound_eof: Some(read.eof),
                ..Reply::default()
            })
        }
        Command::OutboundShutdownWrite { id, timeout_ms } => {
            runtime()?.block_on(engine.outbound.shutdown_write(id, timeout_ms))?;
            Ok(Reply {
                success: true,
                ..Reply::default()
            })
        }
        Command::OutboundClose { id } => {
            runtime()?.block_on(engine.outbound.close(id))?;
            Ok(Reply {
                success: true,
                ..Reply::default()
            })
        }
        Command::OutboundCloseAll => {
            runtime()?.block_on(engine.outbound.close_all())?;
            Ok(Reply {
                success: true,
                ..Reply::default()
            })
        }
        Command::RuntimeRulesOpen { directory } => {
            if directory.len() > 4096 {
                return Err(Error::InvalidInput);
            }
            let mut guard = engine.runtime_rules.lock().map_err(|_| Error::Internal)?;
            if guard.is_some() {
                return Err(Error::InvalidState);
            }
            let store = crate::routing::store::Store::open(directory)?;
            let summary = match store.snapshot() {
                Ok(snapshot) => {
                    engine.outbound.configure_routing(Some(Arc::new(
                        crate::routing::Router::from_runtime(Arc::clone(&snapshot))?,
                    )))?;
                    Some(RuntimeRulesSummary::new(&snapshot, None))
                }
                Err(Error::InvalidState) => {
                    engine.outbound.configure_routing(None)?;
                    None
                }
                Err(error) => return Err(error),
            };
            *guard = Some(crate::routing::background::Service::new(
                store,
                Arc::clone(&engine.outbound),
            ));
            Ok(Reply {
                success: true,
                runtime_rules: summary,
                ..Reply::default()
            })
        }
        Command::RuntimeRulesBegin { settings, sources } => {
            let guard = engine.runtime_rules.lock().map_err(|_| Error::Internal)?;
            let job = guard
                .as_ref()
                .ok_or(Error::InvalidState)?
                .start_inputs(settings, sources)?;
            Ok(Reply {
                success: true,
                runtime_rules_job: Some(job),
                ..Reply::default()
            })
        }
        Command::RuntimeRulesBeginFile { path } => {
            let guard = engine.runtime_rules.lock().map_err(|_| Error::Internal)?;
            let job = guard
                .as_ref()
                .ok_or(Error::InvalidState)?
                .start_file(path.into())?;
            Ok(Reply {
                success: true,
                runtime_rules_job: Some(job),
                ..Reply::default()
            })
        }
        Command::RuntimeRulesJob { id } => {
            let guard = engine.runtime_rules.lock().map_err(|_| Error::Internal)?;
            let job = guard.as_ref().ok_or(Error::InvalidState)?.job(id)?;
            Ok(Reply {
                success: true,
                runtime_rules_job: Some(job),
                ..Reply::default()
            })
        }
        Command::RuntimeRulesCancel { id } => {
            let guard = engine.runtime_rules.lock().map_err(|_| Error::Internal)?;
            guard.as_ref().ok_or(Error::InvalidState)?.cancel(id)?;
            Ok(Reply {
                success: true,
                ..Reply::default()
            })
        }
        Command::RuntimeRulesStatus => {
            let guard = engine.runtime_rules.lock().map_err(|_| Error::Internal)?;
            let snapshot = guard
                .as_ref()
                .ok_or(Error::InvalidState)?
                .store
                .snapshot()?;
            Ok(Reply {
                success: true,
                runtime_rules: Some(RuntimeRulesSummary::new(&snapshot, None)),
                ..Reply::default()
            })
        }
        Command::RuntimeRulesClose => {
            let mut guard = engine.runtime_rules.lock().map_err(|_| Error::Internal)?;
            // 先阻止后台提交，再清空活动路由，避免关闭后被迟到的任务重新装回。
            if let Some(store) = guard.take() {
                store.close()?;
            }
            engine.outbound.configure_routing(None)?;
            Ok(Reply {
                success: true,
                ..Reply::default()
            })
        }
        Command::RouteConfigure(config) => {
            let router = crate::routing::Router::compile(config)?;
            engine.outbound.configure_routing(Some(Arc::new(router)))?;
            Ok(Reply {
                success: true,
                ..Reply::default()
            })
        }
        Command::RouteEvaluate(target) | Command::RouteCheck(target) => {
            let router = engine.router()?;
            let decision = router.evaluate(target)?;
            if checked_route {
                decision.check_support()?;
            }
            Ok(Reply {
                success: true,
                route_decision: Some(decision),
                ..Reply::default()
            })
        }
        Command::RouteClose => {
            engine.outbound.configure_routing(None)?;
            Ok(Reply {
                success: true,
                ..Reply::default()
            })
        }
        Command::DnsValidate(config) => {
            config.validate()?;
            Ok(Reply {
                success: true,
                ..Reply::default()
            })
        }
        Command::DnsClose => {
            engine.close_dns_service()?;
            if let Some(resolver) = engine.outbound.network_snapshot()?.resolver.clone() {
                run_network(async move { resolver.close().await })?;
            }
            engine.outbound.configure_dns(None)?;
            Ok(Reply {
                success: true,
                ..Reply::default()
            })
        }
        Command::DnsServiceStart => {
            if engine
                .dns_service
                .lock()
                .map_err(|_| Error::Internal)?
                .is_some()
            {
                return Err(Error::InvalidState);
            }
            let service = runtime()?.block_on(engine.dns()?.serve())?;
            let status = service.status();
            *engine.dns_service.lock().map_err(|_| Error::Internal)? = Some(service);
            Ok(Reply {
                success: true,
                dns_service_status: Some(status),
                ..Reply::default()
            })
        }
        Command::DnsServiceStop => {
            let status = engine.close_dns_service()?;
            Ok(Reply {
                success: true,
                dns_service_status: Some(status),
                ..Reply::default()
            })
        }
        Command::DnsServiceStatus => {
            let status = engine
                .dns_service
                .lock()
                .map_err(|_| Error::Internal)?
                .as_ref()
                .map(|s| s.status())
                .unwrap_or_default();
            Ok(Reply {
                success: true,
                dns_service_status: Some(status),
                ..Reply::default()
            })
        }
        Command::DnsConfigure(config) => {
            if engine
                .dns_service
                .lock()
                .map_err(|_| Error::Internal)?
                .is_some()
            {
                return Err(Error::InvalidState);
            }
            let resolver =
                crate::dns::Resolver::with_tls_config(config, engine.client()?.tls_config())?;
            let generation = resolver.fake_ip_generation();
            engine.outbound.configure_dns(Some(Arc::new(resolver)))?;
            Ok(Reply {
                success: true,
                dns_generation: generation,
                ..Reply::default()
            })
        }
        Command::DnsLastQuery => Ok(Reply {
            success: true,
            dns_observation: engine.dns()?.last_query()?,
            ..Reply::default()
        }),
        Command::DnsQuery(query) => {
            let resolver = engine.dns()?;
            Reply::dns(&resolver, query)
        }
        Command::DnsFakeIpReverse { address } => Ok(Reply {
            success: true,
            fake_ip_name: engine.dns()?.reverse_fake_ip(&address)?,
            ..Reply::default()
        }),
        Command::DnsFakeIpReset => {
            engine.dns()?.reset_fake_ip()?;
            Ok(Reply {
                success: true,
                ..Reply::default()
            })
        }
        Command::DnsClearCache => {
            if let Some(resolver) = engine.outbound.network_snapshot()?.resolver.as_ref() {
                resolver.clear()?;
            }
            Ok(Reply {
                success: true,
                ..Reply::default()
            })
        }
        Command::TunStart => Err(Error::Unsupported),
        Command::TunUdpOpen(options) => {
            let mut session = engine.tun_udp.lock().map_err(|_| Error::Internal)?;
            if session.is_some() {
                return Err(Error::InvalidState);
            }
            *session = Some(ManagedTun::Udp(Arc::new(crate::tun::udp::Session::new(
                Arc::clone(&engine.outbound),
                options,
            )?)));
            Ok(Reply {
                success: true,
                ..Reply::default()
            })
        }
        Command::TunOpen(options) => {
            let mut session = engine.tun_udp.lock().map_err(|_| Error::Internal)?;
            if session.is_some() {
                return Err(Error::InvalidState);
            }
            *session = Some(ManagedTun::Mixed(Arc::new(
                crate::tun::session::Session::new(engine.outbound.clone(), options)?,
            )));
            Ok(Reply {
                success: true,
                ..Reply::default()
            })
        }
        Command::TunUdpClose | Command::TunClose => {
            engine.close_tun()?;
            Ok(Reply {
                success: true,
                ..Reply::default()
            })
        }
        Command::PacketOpen { mtu } => {
            let mut session = engine.packet_session.lock().map_err(|_| Error::Internal)?;
            if session.is_some() {
                return Err(Error::InvalidState);
            }
            *session = Some(crate::tun::PacketSession::new(mtu)?);
            Ok(Reply {
                success: true,
                ..Reply::default()
            })
        }
        Command::PacketInspect { packet } => {
            if packet.len() > crate::tun::MAX_MTU.div_ceil(3) * 4 {
                return Err(Error::ResourceLimit);
            }
            let bytes = Zeroizing::new(STANDARD.decode(packet).map_err(|_| Error::InvalidInput)?);
            let mut session = engine.packet_session.lock().map_err(|_| Error::Internal)?;
            let info = session
                .as_mut()
                .ok_or(Error::InvalidState)?
                .inspect(&bytes)?;
            Ok(Reply {
                success: true,
                packet_info: Some(info),
                ..Reply::default()
            })
        }
        Command::PacketClose => {
            if let Some(mut session) = engine
                .packet_session
                .lock()
                .map_err(|_| Error::Internal)?
                .take()
            {
                session.close();
            }
            Ok(Reply {
                success: true,
                ..Reply::default()
            })
        }
        Command::Script { source, input } => {
            let result = crate::script::execute(&source, &input)?;
            Ok(Reply {
                success: true,
                script_result_json: Some(
                    serde_json::to_string(&result).map_err(|_| Error::Internal)?,
                ),
                ..Reply::default()
            })
        }
        Command::MitmCertificate | Command::MitmExportIdentity => {
            let authority = engine.authority()?;
            let export_private = matches!(command, Command::MitmExportIdentity);
            Ok(Reply {
                success: true,
                certificate_pem: Some(authority.certificate_pem().to_owned()),
                certificate_der: Some(STANDARD.encode(authority.certificate_der())),
                private_key_pem: if export_private {
                    Some(Secret(authority.private_key_pem()))
                } else {
                    None
                },
                ..Reply::default()
            })
        }
        Command::MitmRestoreIdentity {
            certificate_pem,
            private_key_pem,
        } => {
            let key = Zeroizing::new(private_key_pem);
            if certificate_pem.len() > 64 * 1024 || key.len() > 16 * 1024 {
                return Err(Error::ResourceLimit);
            }
            if engine.proxy.lock().map_err(|_| Error::Internal)?.is_some() {
                return Err(Error::InvalidState);
            }
            let authority = Authority::restore(&certificate_pem, &key)?;
            *engine.authority.lock().map_err(|_| Error::Internal)? = Some(Arc::new(authority));
            Ok(Reply {
                success: true,
                ..Reply::default()
            })
        }
        Command::MitmStart {
            port,
            hosts,
            capture_bodies,
            allow_lan,
            upstream_proxy_port,
            rewrites,
        } => {
            if engine.proxy.lock().map_err(|_| Error::Internal)?.is_some() {
                return Err(Error::InvalidState);
            }
            let proxy = runtime()?.block_on(MitmProxy::start_with_options(
                crate::mitm::ProxyOptions {
                    port,
                    hosts,
                    capture_bodies,
                    allow_lan,
                    upstream_proxy_port,
                    rewrites,
                },
                engine.authority()?,
                engine.client()?,
                Arc::clone(&engine.captures),
            ))?;
            let port = proxy.port();
            *engine.proxy.lock().map_err(|_| Error::Internal)? = Some(proxy);
            Ok(Reply {
                success: true,
                port: Some(port),
                ..Reply::default()
            })
        }
        Command::MitmStop => {
            let proxy = engine.proxy.lock().map_err(|_| Error::Internal)?.take();
            if let Some(proxy) = proxy {
                runtime()?.block_on(proxy.stop())?;
            }
            Ok(Reply {
                success: true,
                ..Reply::default()
            })
        }
        Command::MitmPeekCapture => Ok(Reply {
            success: true,
            capture: engine.captures.peek()?,
            dropped_captures: engine.captures.dropped()?,
            ..Reply::default()
        }),
        Command::MitmAcknowledgeCapture { id } => {
            engine.captures.acknowledge(id)?;
            Ok(Reply {
                success: true,
                ..Reply::default()
            })
        }
        Command::AddTrustedRoot { certificate_pem } => {
            if engine.proxy.lock().map_err(|_| Error::Internal)?.is_some() {
                return Err(Error::InvalidState);
            }
            if certificate_pem.len() > 64 * 1024 {
                return Err(Error::ResourceLimit);
            }
            let certificate = CertificateDer::from_pem_slice(certificate_pem.as_bytes())
                .map_err(|_| Error::Certificate)?;
            let client = HttpsClient::system_with_additional_roots(vec![certificate])?
                .with_network(Arc::clone(&engine.outbound));
            *engine.client.lock().map_err(|_| Error::Internal)? = Some(client);
            Ok(Reply {
                success: true,
                ..Reply::default()
            })
        }
        Command::Https(_) | Command::Request(_) => {
            let https_only = matches!(command, Command::Https(_));
            let (Command::Https(input) | Command::Request(input)) = command else {
                unreachable!()
            };
            execute_request(&engine, input, https_only)
        }
    }
}
// 安全条件：导出符号具有唯一前缀，固定宽度值类型不共享 Rust 对象地址。
#[unsafe(no_mangle)]
pub extern "C" fn fluxa_engine_abi_version() -> u32 {
    2
}
// 安全条件同上；位 0 代表 HTTPS 客户端，位 1 代表 MITM，位 2 代表 JS，位 3 代表通用 HTTP 调试，位 4 代表内置流量脚本，位 5 仅代表 TUN 数据包检查（不含转发），位 6 代表 DNS 客户端，位 7 代表 Fake-IP，位 8 代表局域网 DNS 服务，位 9 代表路由决策，位 10 代表数字地址 TCP/UDP 出站，位 11 代表配置 DNS 的域名出站；不代表代理出站协议可用。
#[unsafe(no_mangle)]
pub extern "C" fn fluxa_engine_capabilities() -> u64 {
    // 位 25 代表整数错误查询及显式结构化命令扩展，原能力位不改义。
    const STRUCTURED_ERROR_CAPABILITY: u64 = 1 << 25;
    // 位 27：正式域名与 TCP/UDP 组合规则，不改变既有能力位语义。
    const DOMAIN_TRANSPORT_CAPABILITY: u64 = 1 << 27;
    // 位 28：自研 TCP/UDP 包会话；不代表系统 VPN 已接管。
    const TUN_TCP_CAPABILITY: u64 = 1 << 28;
    33554431
        | TUN_TCP_CAPABILITY
        | STRUCTURED_ERROR_CAPABILITY
        | DOMAIN_TRANSPORT_CAPABILITY
        | if cfg!(target_os = "macos") {
            1 << 26
        } else {
            0
        }
}
// 安全条件同上；注册表拥有对象，零返回表示失败。
#[unsafe(no_mangle)]
pub extern "C" fn fluxa_engine_create() -> u64 {
    clear_last_error();
    catch_unwind(AssertUnwindSafe(|| {
        let mut registry = match registry().lock() {
            Ok(registry) => registry,
            Err(_) => return boundary_error(ErrorCode::Internal),
        };
        registry
            .create(Resource::Engine(Arc::new(Engine::default())))
            .unwrap_or_else(|| boundary_error(ErrorCode::ResourceLimit))
    }))
    .unwrap_or_else(|_| boundary_error(ErrorCode::Internal))
}
// 安全条件同上；先移除所有权，再在注册表锁之外释放对象。
#[unsafe(no_mangle)]
pub extern "C" fn fluxa_engine_destroy(handle: u64) -> i32 {
    clear_last_error();
    catch_unwind(AssertUnwindSafe(|| {
        let resource = match registry().lock() {
            Ok(mut registry) => registry.take(handle),
            Err(_) => {
                set_last_error(Error::Internal.public_info());
                return INTERNAL_ERROR;
            }
        };
        if resource.is_some() {
            #[cfg(unix)]
            if let Some(Resource::DebugClient(client)) = &resource {
                let client = client.clone();
                if let Err(error) = run_network(async move { client.close().await }) {
                    return error_status(error);
                }
            }
            if let Some(Resource::Engine(engine)) = &resource {
                #[cfg(unix)]
                {
                    let closed = (|| {
                        let mut slot = engine.debug_ipc.lock().map_err(|_| Error::Internal)?;
                        if let Some(service) = slot.as_mut() {
                            runtime()?.block_on(service.close())?;
                        }
                        slot.take();
                        Ok::<_, Error>(())
                    })();
                    if let Err(error) = closed {
                        return error_status(error);
                    }
                }
                if let Err(error) = engine.close_tun() {
                    return error_status(error);
                }
                if let Err(error) = finish_shutdown(engine.outbound.stop()) {
                    return error_status(error);
                }
                let dns = match engine.outbound.network_snapshot() {
                    Ok(snapshot) => snapshot.resolver.clone(),
                    Err(error) => return error_status(error),
                };
                if let Some(resolver) = dns
                    && let Err(error) = finish_shutdown(resolver.close())
                {
                    return error_status(error);
                }
                if let Err(error) = engine.close_dns_service() {
                    return error_status(error);
                }
                let closed_proxy = (|| {
                    let mut proxy = engine.proxy.lock().map_err(|_| Error::Internal)?;
                    if let Some(service) = proxy.take() {
                        finish_shutdown(service.stop())?;
                    }
                    Ok::<_, Error>(())
                })();
                if let Err(error) = closed_proxy {
                    return error_status(error);
                }
            }
            drop(resource);
            OK
        } else {
            set_last_error(ErrorInfo::new(ErrorCode::InvalidHandle));
            INVALID_HANDLE
        }
    }))
    .unwrap_or_else(|_| {
        set_last_error(Error::Internal.public_info());
        INTERNAL_ERROR
    })
}
// 安全条件同上；系统隧道与入口尚未实现，不伪造连接；独立协议出站通过命令调用。
#[unsafe(no_mangle)]
pub extern "C" fn fluxa_engine_start(handle: u64) -> i32 {
    clear_last_error();
    catch_unwind(AssertUnwindSafe(|| match engine(handle) {
        Ok(_) => {
            set_last_error(Error::Unsupported.public_info());
            UNSUPPORTED
        }
        Err(error) => {
            let info = if error == Error::InvalidState {
                ErrorInfo::new(ErrorCode::InvalidHandle)
            } else {
                error.public_info()
            };
            set_last_error(info);
            i32::from(info.code.value())
        }
    }))
    .unwrap_or_else(|_| {
        set_last_error(Error::Internal.public_info());
        INTERNAL_ERROR
    })
}
/// 请求仅在调用期间借用；返回响应句柄，需调用统一 destroy 释放。
/// 响应名额或预算不足时，在执行命令前返回零；释放旧响应后可重试，读取数据未被消费。
/// # Safety
/// 调用方必须保证 input 在整个调用期间指向 length 个可读字节且没有并发写入。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn fluxa_engine_command(handle: u64, input: *const u8, length: usize) -> u64 {
    // SAFETY: 此入口沿用调用方的输入借用契约，失败统一提供分段代码与受限上下文。
    unsafe { command_boundary(handle, input, length, false) }
}
/// 与 v1 共用命令执行及预算流程，显式选择新增结构化错误字段。
/// # Safety
/// input 在整个同步调用期间指向 length 个可读且无并发写入的字节。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn fluxa_engine_command_v2(
    handle: u64,
    input: *const u8,
    length: usize,
) -> u64 {
    // SAFETY: 输入借用在调用期间有效，不保存指针。
    unsafe { command_boundary(handle, input, length, true) }
}
unsafe fn command_boundary(
    handle: u64,
    input: *const u8,
    length: usize,
    reject_invalid_handle: bool,
) -> u64 {
    clear_last_error();
    catch_unwind(AssertUnwindSafe(|| {
        if input.is_null() || length == 0 {
            return boundary_error(ErrorCode::InvalidInput);
        }
        if length > MAX_JSON {
            return boundary_error(ErrorCode::ResourceLimit);
        }
        // SAFETY: 指针可读性由调用方保证；已检查空指针和长度，不保存借用。
        let bytes = unsafe { std::slice::from_raw_parts(input, length) };
        let mut output = match OutputReservation::new(output_limit(bytes)) {
            Ok(output) => output,
            Err(error) => return boundary_failure(error),
        };
        let handle_error = engine(handle).err().map(|error| {
            if error == Error::InvalidState {
                ErrorInfo::new(ErrorCode::InvalidHandle)
            } else {
                error.public_info()
            }
        });
        if reject_invalid_handle && let Some(info) = handle_error {
            set_last_error(info);
            return 0;
        }
        let result = command(handle, bytes);
        // 先查句柄再解析命令的原顺序不变；无效句柄明确返回统一代码 1201。
        let invalid_handle_result = matches!(result, Err(Error::InvalidState))
            && handle_error.is_some_and(|info| info.code == ErrorCode::InvalidHandle);
        let mut reply = result.unwrap_or_else(Reply::failure);
        if invalid_handle_result {
            let info = ErrorInfo::new(ErrorCode::InvalidHandle);
            set_last_error(info);
            reply.error_info = Some(info);
        }
        if let Err(error) = serde_json::to_writer(&mut output, &reply) {
            let code = if error.io_error_kind().is_some() {
                ErrorCode::ResourceLimit
            } else {
                ErrorCode::SerializationFailed
            };
            return boundary_error(code);
        }
        match output.finish() {
            Ok(handle) => handle,
            Err(error) => boundary_failure(error),
        }
    }))
    .unwrap_or_else(|_| boundary_error(ErrorCode::Internal))
}
/// 返回当前同步调用线程的固定整数错误代码；零是“无错误”状态，不是错误代码。
/// 只读取线程本地值，不创建响应、不分配、不加锁，不清除错误。
#[unsafe(no_mangle)]
pub extern "C" fn fluxa_engine_last_error_code() -> u16 {
    LAST_ERROR
        .get()
        .map_or(NO_ERROR_CODE, |info| info.code.value())
}
/// 读取当前调用线程最后一个 ABI 操作的结构化错误；不分配内存、不创建句柄。
/// 返回所需长度；没有错误返回零。容量不足/空指针时不写入，查询本身不覆盖错误。
/// # Safety
/// 容量足够时 output 指向 capacity 个可写字节，且调用期间无并发访问。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn fluxa_engine_last_error_copy(output: *mut u8, capacity: usize) -> usize {
    let Some(info) = LAST_ERROR.get() else {
        return 0;
    };
    let mut buffer = [0u8; crate::error::MAX_ERROR_BYTES];
    let mut writer = std::io::Cursor::new(buffer.as_mut_slice());
    let result = serde_json::to_writer(&mut writer, &info);
    let length = writer.position() as usize;
    let bytes = if result.is_ok() {
        &buffer[..length]
    } else {
        // 编码异常仍返回失败对象，不能用零伪装成没有错误；原失败留在受控诊断。
        crate::error::diagnostics::record_code(
            info.code,
            crate::error::diagnostics::DiagnosticSite::Abi,
        );
        ErrorCode::SerializationFailed
            .json_bytes()
            .expect("无上下文的固定编码失败契约")
    };
    if !output.is_null() && capacity >= bytes.len() {
        // SAFETY: 源在同步复制期间有效；已检查容量，目标可写性由宿主保证。
        unsafe {
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), output, bytes.len());
        }
    }
    bytes.len()
}
// 安全条件同上；只返回固定宽度长度值，无效句柄返回零。
#[unsafe(no_mangle)]
pub extern "C" fn fluxa_engine_buffer_length(handle: u64) -> usize {
    clear_last_error();
    match checked_buffer(handle) {
        Ok(buffer) => buffer.len(),
        Err(info) => {
            set_last_error(info);
            0
        }
    }
}
/// 复制到调用方提供的缓冲区；容量不足时返回所需长度且不写入。
/// # Safety
/// 容量足够时，output 必须指向 capacity 个可写字节，且无并发访问。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn fluxa_engine_buffer_copy(
    handle: u64,
    output: *mut u8,
    capacity: usize,
) -> usize {
    clear_last_error();
    catch_unwind(AssertUnwindSafe(|| {
        let bytes = match checked_buffer(handle) {
            Ok(bytes) => bytes,
            Err(info) => {
                set_last_error(info);
                return 0;
            }
        };
        if output.is_null() || capacity < bytes.len() {
            return bytes.len();
        }
        // SAFETY: 源由 Arc 保持存活且不暴露指针；容量检查通过，目标可写性由调用方保证。
        unsafe {
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), output, bytes.len());
        }
        bytes.len()
    }))
    .unwrap_or_else(|_| boundary_error(ErrorCode::Internal) as usize)
}
/// TUN 二进制接口专用状态码；不包含节点凭据或流量正文。
fn tun_status(error: Error) -> i32 {
    i32::from(error.code().value())
}
fn udp_session(handle: u64) -> Result<ManagedTun, i32> {
    engine(handle)
        .map_err(|error| {
            let info = if error == Error::InvalidState {
                ErrorInfo::new(ErrorCode::InvalidHandle)
            } else {
                error.public_info()
            };
            set_last_error(info);
            i32::from(info.code.value())
        })?
        .tun_udp
        .lock()
        .map_err(|_| {
            set_last_error(Error::Internal.public_info());
            ErrorCode::Internal.value() as i32
        })?
        .as_ref()
        .cloned()
        .ok_or_else(|| {
            set_last_error(Error::InvalidState.public_info());
            ErrorCode::InvalidState.value() as i32
        })
}
/// 同步提交一个完整 UDP/IP 包；generation=0 表示不提供 Fake-IP 代次。
/// # Safety
/// input 必须在调用期间指向 length 个可读且无并发写入的字节；不会保留该借用。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn fluxa_engine_tun_submit(
    handle: u64,
    input: *const u8,
    length: usize,
    generation: u64,
) -> i32 {
    clear_last_error();
    catch_unwind(AssertUnwindSafe(|| {
        if input.is_null() || length == 0 || length > crate::tun::MAX_MTU {
            return error_status(Error::InvalidInput);
        }
        let session = match udp_session(handle) {
            Ok(session) => session,
            Err(code) => return code,
        };
        // SAFETY: 已检查空指针与最大长度；调用方保证实际可读性和调用期内独占写入。
        let bytes = unsafe { std::slice::from_raw_parts(input, length) };
        match runtime().and_then(|runtime| {
            runtime.block_on(session.submit(bytes, (generation != 0).then_some(generation)))
        }) {
            Ok(()) => AbiStatus::Success.value(),
            Err(error) => error_status(error),
        }
    }))
    .unwrap_or_else(|_| error_status(Error::Internal))
}
/// 非阻塞获取返回包，11 表示暂无数据；成功返回的缓冲区句柄必须 destroy。
/// # Safety
/// output 必须指向一个调用期间有效、对齐、可写且无并发访问的 u64；不保存其地址。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn fluxa_engine_tun_receive(handle: u64, output: *mut u64) -> i32 {
    clear_last_error();
    catch_unwind(AssertUnwindSafe(|| {
        if output.is_null() {
            return error_status(Error::InvalidInput);
        }
        // SAFETY: 调用方保证 output 可写且对齐，先清零避免错误时复用旧资源句柄。
        unsafe {
            *output = 0;
        }
        let session = match udp_session(handle) {
            Ok(session) => session,
            Err(code) => return code,
        };
        let mut reservation = match OutputReservation::new(crate::tun::MAX_MTU) {
            Ok(value) => value,
            Err(error) => {
                set_last_error(error.public_info());
                return i32::from(error.code().value());
            }
        };
        match session.receive() {
            Ok(Some(packet)) => {
                // 保留具体失败；响应发布内部错误不能被合并为资源不足，数值失败也使用四位代码。
                let result = std::io::Write::write_all(&mut reservation, &packet.bytes)
                    .map_err(Error::from)
                    .and_then(|()| reservation.finish());
                match result {
                    Ok(handle) => {
                        // SAFETY: output 的借用覆盖本次同步调用；只写值，不转移指针所有权。
                        unsafe {
                            *output = handle;
                        }
                        AbiStatus::Success.value()
                    }
                    Err(error) => {
                        set_last_error(error.public_info());
                        i32::from(error.code().value())
                    }
                }
            }
            Ok(None) => AbiStatus::Empty.value(),
            Err(error) => error_status(error),
        }
    }))
    .unwrap_or_else(|_| error_status(Error::Internal))
}

#[cfg(unix)]
mod debug;

#[cfg(test)]
#[path = "../tests/test_ffi.rs"]
mod test_ffi;
