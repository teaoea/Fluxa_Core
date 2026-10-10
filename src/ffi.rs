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
    busy: AtomicBool,
    authority: Mutex<Option<Arc<Authority>>>,
    proxy: Mutex<Option<MitmProxy>>,
    captures: Arc<CaptureStore>,
}
impl Engine {
    fn client(&self) -> Result<HttpsClient, Error> {
        let mut client = self.client.lock().map_err(|_| Error::Internal)?;
        if client.is_none() {
            *client = Some(HttpsClient::system()?);
        }
        client.clone().ok_or(Error::Internal)
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
#[derive(Default, Serialize)]
struct Reply {
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
fn command(handle: u64, bytes: &[u8]) -> Result<Reply, Error> {
    let engine = engine(handle)?;
    if engine
        .busy
        .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
        .is_err()
    {
        return Err(Error::InvalidState);
    }
    let _busy = BusyGuard(&engine.busy);
    static REQUESTS: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(8);
    let _permit = REQUESTS.try_acquire().map_err(|_| Error::ResourceLimit)?;
    let command: Command = serde_json::from_slice(bytes).map_err(|_| Error::InvalidInput)?;
    match command {
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
            let client = HttpsClient::system_with_additional_roots(vec![certificate])?;
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
            let response = runtime()?.block_on(engine.client()?.execute_request(
                &input.method,
                &input.url,
                &input.headers,
                &body,
                input.proxy_port,
            ))?;
            Ok(Reply::from(response))
        }
    }
}
// 安全条件：导出符号具有唯一前缀，固定宽度值类型不共享 Rust 对象地址。
#[unsafe(no_mangle)]
pub extern "C" fn fluxa_engine_abi_version() -> u32 {
    1
}
// 安全条件同上；位 0 代表 HTTPS 客户端，位 1 代表 MITM，位 2 代表 JS，位 3 代表通用 HTTP 调试，位 4 代表内置流量脚本；不代表代理出站协议可用。
#[unsafe(no_mangle)]
pub extern "C" fn fluxa_engine_capabilities() -> u64 {
    31
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
            drop(resource);
            OK
        } else {
            INVALID_HANDLE
        }
    }))
    .unwrap_or(INTERNAL_ERROR)
}
// 安全条件同上；尚未实现代理出站，不降级直连。
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
        assert_eq!(fluxa_engine_capabilities(), 31);
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
}
