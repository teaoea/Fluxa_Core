//! C ABI 只借用调用期内的缓冲区；Rust 返回资源句柄，由统一释放入口回收。
use crate::budget::{ByteBudget, ByteLease};
use crate::mitm::{
    MitmProxy,
    authority::Authority,
    capture::{Capture, CaptureStore},
};
use crate::{
    error::Error,
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

const OK: i32 = 0;
const INVALID_HANDLE: i32 = 1;
const UNSUPPORTED: i32 = 2;
const INTERNAL_ERROR: i32 = 3;
const MAX_JSON: usize = 12 * 1024 * 1024;
const MAX_OUTPUT: usize = 24 * 1024 * 1024;
const MAX_OUTPUT_MEMORY: usize = 32 * 1024 * 1024;
#[derive(Default)]
struct Engine {
    client: Mutex<Option<HttpsClient>>,
    geoip: Mutex<Option<crate::geoip::Database>>,
    busy: AtomicBool,
    outbound: Arc<crate::outbound::Manager>,
    dns_service: Mutex<Option<crate::dns::service::Service>>,
    packet_session: Mutex<Option<crate::tun::PacketSession>>,
    tun_udp: Mutex<Option<Arc<crate::tun::udp::Session>>>,
    authority: Mutex<Option<Arc<Authority>>>,
    proxy: Mutex<Option<MitmProxy>>,
    captures: Arc<CaptureStore>,
    runtime_rules: Mutex<Option<crate::routing::background::Service>>,
}
impl Engine {
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
    static RUNTIME: OnceLock<Result<Runtime, Error>> = OnceLock::new();
    RUNTIME
        .get_or_init(|| {
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .map_err(|_| Error::Internal)
        })
        .as_ref()
        .map_err(|e| *e)
}
fn engine(handle: u64) -> Result<Arc<Engine>, Error> {
    match registry().lock().map_err(|_| Error::Internal)?.get(handle) {
        Some(Resource::Engine(engine)) => Ok(Arc::clone(engine)),
        _ => Err(Error::InvalidState),
    }
}
fn buffer(handle: u64) -> Option<Arc<OwnedBuffer>> {
    match registry().lock().ok()?.get(handle) {
        Some(Resource::Buffer(buffer)) => Some(Arc::clone(buffer)),
        _ => None,
    }
}
fn store(bytes: Vec<u8>) -> Option<u64> {
    let bytes = Zeroizing::new(bytes);
    if bytes.len() > MAX_OUTPUT {
        return None;
    }
    static OUTPUT: ByteBudget = ByteBudget::new(MAX_OUTPUT_MEMORY);
    let lease = OUTPUT.reserve(bytes.len()).ok()?;
    registry()
        .lock()
        .ok()?
        .create(Resource::Buffer(Arc::new(OwnedBuffer {
            bytes,
            _lease: lease,
        })))
}
#[derive(Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
enum Command {
    GeoipOpen {
        path: String,
    },
    GeoipLookup {
        address: String,
        language: String,
    },
    GeoipClose,
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
impl From<Error> for Reply {
    fn from(error: Error) -> Self {
        Self {
            error: Some(error.to_string()),
            ..Self::default()
        }
    }
}
struct BusyGuard<'a>(&'a AtomicBool);
impl Drop for BusyGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
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
            Ok(Command::TunUdpClose) => {
                if let Some(session) = engine.tun_udp.lock().map_err(|_| Error::Internal)?.take() {
                    session.close()?;
                }
                return Ok(Reply {
                    success: true,
                    ..Reply::default()
                });
            }
            Ok(Command::OutboundClose { id }) => {
                engine.outbound.close(id)?;
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
                engine.outbound.close_all()?;
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
        Command::GeoipOpen { path } => {
            if path.is_empty() || path.len() > 4096 {
                return Err(Error::InvalidInput);
            }
            let database = crate::geoip::Database::open(std::path::Path::new(&path))?;
            *engine.geoip.lock().map_err(|_| Error::Internal)? = Some(database);
            Ok(Reply {
                success: true,
                ..Reply::default()
            })
        }
        Command::GeoipLookup { address, language } => {
            let address = if address.parse::<std::net::IpAddr>().is_ok() {
                address
            } else {
                let name = crate::network::normalize_host(&address)?;
                let resolver = engine.dns()?;
                let mut selected = None;
                for kind in ["A", "AAAA"] {
                    let answer = runtime()?.block_on(resolver.resolve_real(crate::dns::Query {
                        name: name.clone(),
                        query_type: kind.into(),
                    }))?;
                    if answer.rcode != 0 {
                        return Err(Error::DnsResponse);
                    }
                    if let Some(value) = answer.addresses.into_iter().next() {
                        selected = Some(value);
                        break;
                    }
                }
                selected.ok_or(Error::DnsResponse)?
            };
            let database = engine.geoip.lock().map_err(|_| Error::Internal)?;
            let location = database
                .as_ref()
                .ok_or(Error::InvalidState)?
                .lookup(&address, &language)?;
            Ok(Reply {
                success: true,
                geoip: location,
                ..Reply::default()
            })
        }
        Command::GeoipClose => {
            *engine.geoip.lock().map_err(|_| Error::Internal)? = None;
            Ok(Reply {
                success: true,
                ..Reply::default()
            })
        }
        Command::SubscriptionSchedule(check) => Ok(Reply {
            success: true,
            subscription_schedule: Some(check.evaluate()?),
            ..Reply::default()
        }),
        Command::NetworkConfigure {
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
            let router = Arc::new(crate::routing::Router::compile(routing)?);
            let resolver = dns
                .map(|config| {
                    crate::dns::Resolver::with_tls_config(config, engine.client()?.tls_config())
                        .map(Arc::new)
                })
                .transpose()?;
            let revision = engine.outbound.configure_network_with_outbounds(
                resolver,
                Some(router),
                limits,
                outbounds,
            )?;
            Ok(Reply {
                success: true,
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
            let opened = runtime()?.block_on(engine.outbound.connect_configured(target))?;
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
            let opened = runtime()?.block_on(engine.outbound.connect_configured_fake_ip(target))?;
            Ok(Reply::from(opened))
        }
        Command::OutboundConnect(target) => {
            target.policy.check_support()?;
            let opened = runtime()?.block_on(engine.outbound.connect_configured_domain(target))?;
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
            engine.outbound.close(id)?;
            Ok(Reply {
                success: true,
                ..Reply::default()
            })
        }
        Command::OutboundCloseAll => {
            engine.outbound.close_all()?;
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
            let service = engine
                .dns_service
                .lock()
                .map_err(|_| Error::Internal)?
                .take();
            if let Some(mut service) = service {
                runtime()?.block_on(service.stop())?;
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
            let service = engine
                .dns_service
                .lock()
                .map_err(|_| Error::Internal)?
                .take();
            let mut status = service.as_ref().map(|s| s.status()).unwrap_or_default();
            if let Some(mut service) = service {
                runtime()?.block_on(service.stop())?;
            }
            status.running = false;
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
            *session = Some(Arc::new(crate::tun::udp::Session::new(
                Arc::clone(&engine.outbound),
                options,
            )?));
            Ok(Reply {
                success: true,
                ..Reply::default()
            })
        }
        Command::TunUdpClose => {
            if let Some(session) = engine.tun_udp.lock().map_err(|_| Error::Internal)?.take() {
                session.close()?;
            }
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
    1
}
// 安全条件同上；位 0 代表 HTTPS 客户端，位 1 代表 MITM，位 2 代表 JS，位 3 代表通用 HTTP 调试，位 4 代表内置流量脚本，位 5 仅代表 TUN 数据包检查（不含转发），位 6 代表 DNS 客户端，位 7 代表 Fake-IP，位 8 代表局域网 DNS 服务，位 9 代表路由决策，位 10 代表数字地址 TCP/UDP 出站，位 11 代表配置 DNS 的域名出站；不代表代理出站协议可用。
#[unsafe(no_mangle)]
pub extern "C" fn fluxa_engine_capabilities() -> u64 {
    33554431
}
// 安全条件同上；注册表拥有对象，零返回表示失败。
#[unsafe(no_mangle)]
pub extern "C" fn fluxa_engine_create() -> u64 {
    catch_unwind(AssertUnwindSafe(|| {
        registry()
            .lock()
            .ok()?
            .create(Resource::Engine(Arc::new(Engine::default())))
    }))
    .ok()
    .flatten()
    .unwrap_or(0)
}
// 安全条件同上；先移除所有权，再在注册表锁之外释放对象。
#[unsafe(no_mangle)]
pub extern "C" fn fluxa_engine_destroy(handle: u64) -> i32 {
    catch_unwind(AssertUnwindSafe(|| {
        let resource = match registry().lock() {
            Ok(mut registry) => registry.take(handle),
            Err(_) => return INTERNAL_ERROR,
        };
        if resource.is_some() {
            if let Some(Resource::Engine(engine)) = &resource {
                if let Ok(mut session) = engine.tun_udp.lock()
                    && let Some(session) = session.take()
                {
                    let _ = session.close();
                }
                let _ = engine.outbound.stop();
            }
            drop(resource);
            OK
        } else {
            INVALID_HANDLE
        }
    }))
    .unwrap_or(INTERNAL_ERROR)
}
// 安全条件同上；系统隧道与入口尚未实现，不伪造连接；独立协议出站通过命令调用。
#[unsafe(no_mangle)]
pub extern "C" fn fluxa_engine_start(handle: u64) -> i32 {
    catch_unwind(AssertUnwindSafe(|| {
        if engine(handle).is_ok() {
            UNSUPPORTED
        } else {
            INVALID_HANDLE
        }
    }))
    .unwrap_or(INTERNAL_ERROR)
}
/// 请求仅在调用期间借用；返回响应句柄，需调用统一 destroy 释放。
/// # Safety
/// 调用方必须保证 input 在整个调用期间指向 length 个可读字节且没有并发写入。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn fluxa_engine_command(handle: u64, input: *const u8, length: usize) -> u64 {
    catch_unwind(AssertUnwindSafe(|| {
        if input.is_null() || length == 0 || length > MAX_JSON {
            return 0;
        }
        // SAFETY: 空指针和长度已检查，实际可读性由 C ABI 调用方契约保证；不保留借用。
        let bytes = unsafe { std::slice::from_raw_parts(input, length) };
        let reply = command(handle, bytes).unwrap_or_else(Reply::from);
        serde_json::to_vec(&reply).ok().and_then(store).unwrap_or(0)
    }))
    .unwrap_or(0)
}
// 安全条件同上；只返回固定宽度长度值，无效句柄返回零。
#[unsafe(no_mangle)]
pub extern "C" fn fluxa_engine_buffer_length(handle: u64) -> usize {
    buffer(handle).map_or(0, |b| b.len())
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
    catch_unwind(AssertUnwindSafe(|| {
        let Some(bytes) = buffer(handle) else {
            return 0;
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
    .unwrap_or(0)
}
/// TUN 二进制接口专用状态码；不包含节点凭据或流量正文。
fn tun_status(error: Error) -> i32 {
    match error {
        Error::Unsupported => 2,
        Error::Internal => 3,
        Error::InvalidInput => 4,
        Error::ResourceLimit => 5,
        Error::InvalidState => 6,
        Error::Cancelled => 7,
        Error::Rejected => 9,
        Error::NetworkTimeout | Error::DnsTimeout => 10,
        _ => 8,
    }
}
fn udp_session(handle: u64) -> Result<Arc<crate::tun::udp::Session>, i32> {
    engine(handle)
        .map_err(|_| 1)?
        .tun_udp
        .lock()
        .map_err(|_| 3)?
        .as_ref()
        .map(Arc::clone)
        .ok_or(6)
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
    catch_unwind(AssertUnwindSafe(|| {
        if input.is_null() || length == 0 || length > crate::tun::MAX_MTU {
            return 4;
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
            Ok(()) => 0,
            Err(error) => tun_status(error),
        }
    }))
    .unwrap_or(3)
}
/// 非阻塞获取返回包，11 表示暂无数据；成功返回的缓冲区句柄必须 destroy。
/// # Safety
/// output 必须指向一个调用期间有效、对齐、可写且无并发访问的 u64；不保存其地址。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn fluxa_engine_tun_receive(handle: u64, output: *mut u64) -> i32 {
    catch_unwind(AssertUnwindSafe(|| {
        if output.is_null() {
            return 4;
        }
        // SAFETY: 调用方保证 output 可写且对齐，先清零避免错误时复用旧资源句柄。
        unsafe {
            *output = 0;
        }
        let session = match udp_session(handle) {
            Ok(session) => session,
            Err(code) => return code,
        };
        match session.receive() {
            Ok(Some(packet)) => match store(packet.bytes.to_vec()) {
                Some(handle) => {
                    // SAFETY: output 的借用覆盖本次同步调用；只写值，不转移指针所有权。
                    unsafe {
                        *output = handle;
                    }
                    0
                }
                None => 5,
            },
            Ok(None) => 11,
            Err(error) => tun_status(error),
        }
    }))
    .unwrap_or(3)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn abi_rejects_invalid_and_destroyed_handles() {
        let handle = fluxa_engine_create();
        assert_ne!(handle, 0);
        assert_eq!(fluxa_engine_start(handle), UNSUPPORTED);
        assert_eq!(fluxa_engine_destroy(handle), OK);
        assert_eq!(fluxa_engine_destroy(handle), INVALID_HANDLE);
        assert_eq!(fluxa_engine_start(handle), INVALID_HANDLE);
        assert_eq!(fluxa_engine_capabilities(), 33554431);
    }
    #[test]
    fn concurrent_owners_release_their_own_handles() {
        let threads: Vec<_> = (0..8)
            .map(|_| {
                std::thread::spawn(|| {
                    for _ in 0..100 {
                        let handle = fluxa_engine_create();
                        assert_ne!(handle, 0);
                        assert_eq!(fluxa_engine_destroy(handle), OK);
                        assert_eq!(fluxa_engine_start(handle), INVALID_HANDLE);
                    }
                })
            })
            .collect();
        for thread in threads {
            thread.join().unwrap();
        }
    }
    #[test]
    fn output_buffers_obey_budget_and_handle_type() {
        let handle = store(vec![1; 32]).unwrap();
        assert_eq!(fluxa_engine_start(handle), INVALID_HANDLE);
        assert_eq!(fluxa_engine_buffer_length(handle), 32);
        assert!(store(vec![0; MAX_OUTPUT + 1]).is_none());
        assert_eq!(fluxa_engine_destroy(handle), OK);
        assert_eq!(fluxa_engine_buffer_length(handle), 0);
    }
    #[tokio::test]
    async fn nested_runtime_does_not_detach_failed_http_calls() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let handle = fluxa_engine_create();
        engine(handle)
            .unwrap()
            .outbound
            .configure_network(
                None,
                Some(Arc::new(
                    crate::routing::Router::compile(crate::routing::Config {
                        mode: crate::routing::Mode::Direct,
                        ..Default::default()
                    })
                    .unwrap(),
                )),
                crate::outbound::Limits::default(),
            )
            .unwrap();
        let request=serde_json::to_vec(&serde_json::json!({"operation":"request","method":"GET","url":format!("http://{}/",listener.local_addr().unwrap())})).unwrap();
        // SAFETY: JSON 输入在整个同步调用期间存活、只读，不保留任何跨语言借用。
        let response = unsafe { fluxa_engine_command(handle, request.as_ptr(), request.len()) };
        assert_eq!(response, 0);
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(50), listener.accept())
                .await
                .is_err()
        );
        assert_eq!(fluxa_engine_destroy(handle), 0);
    }
}
