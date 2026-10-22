//! 专用 Debug ABI：只操作独立 IPC，不把普通 command 变成调试授权入口。
use super::*;
use crate::routing::debug::ipc::{Client, Request, Service};
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ServerOptions {
    policy_file: std::path::PathBuf,
    directory: std::path::PathBuf,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ClientOptions {
    endpoint: std::path::PathBuf,
    credential: std::path::PathBuf,
    server_uid: u32,
    server_requirement: String,
    lifetime_seconds: u64,
}
pub(super) struct Handle {
    closed: AtomicBool,
    client: tokio::sync::Mutex<Option<Client>>,
}
impl Handle {
    async fn command(&self, request: &Request) -> Result<crate::routing::debug::ipc::Reply, Error> {
        let mut client = self.client.lock().await;
        if self.closed.load(Ordering::Acquire) {
            return Err(Error::DebugSessionClosed);
        }
        let client = client.as_mut().ok_or(Error::DebugSessionClosed)?;
        client.command(request).await
    }
    pub(super) async fn close(&self) -> Result<(), Error> {
        self.closed.store(true, Ordering::Release);
        self.client.lock().await.take();
        Ok(())
    }
}
fn parse<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> Result<T, Error> {
    serde_json::from_slice(bytes).map_err(|_| Error::InvalidInput)
}
/// # Safety
/// input 必须在调用期间指向 length 个可读字节，不能并发写入；借用不保存。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nodus_core_debug_ipc_start(
    handle: u64,
    input: *const u8,
    length: usize,
) -> i32 {
    clear_last_error();
    catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: 固定 FFI 输入契约与边界检查共用，不将指针带入异步任务。
        let result = (|| {
            // SAFETY: 已声明调用期间有效且不保存指针的 FFI 输入契约。
            let bytes = unsafe { input_bytes(input, length)? };
            let options: ServerOptions = parse(bytes)?;
            let engine = engine(handle)?;
            let mut service = engine.debug_ipc.lock().map_err(|_| Error::Internal)?;
            if service.is_some() {
                return Err(Error::InvalidState);
            }
            let manager = engine.outbound.clone();
            let started = run_network(async move {
                Service::start(&options.policy_file, &options.directory, manager).await
            })?;
            *service = Some(started);
            Ok(())
        })();
        match result {
            Ok(()) => OK,
            Err(error) => error_status(error),
        }
    }))
    .unwrap_or_else(|_| error_status(Error::Internal))
}
unsafe fn input_bytes<'a>(input: *const u8, length: usize) -> Result<&'a [u8], Error> {
    if input.is_null() || length == 0 {
        return Err(Error::InvalidInput);
    }
    if length > 65536 {
        return Err(Error::ResourceLimit);
    }
    // SAFETY: 调用方承诺当前同步借用可读；空指针和长度上限已检查，不保存。
    Ok(unsafe { std::slice::from_raw_parts(input, length) })
}
/// # Safety
/// 输入只在同步调用期间借用，调用方保证可读且无并发写入。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nodus_core_debug_ipc_connect(input: *const u8, length: usize) -> u64 {
    clear_last_error();
    catch_unwind(AssertUnwindSafe(|| {
        let result = (|| {
            // SAFETY: 借用在同步入口内结束，异步仅接收拥有的受校验值。
            let options: ClientOptions = parse(unsafe { input_bytes(input, length)? })?;
            let client = run_network(async move {
                Client::connect(
                    &options.endpoint,
                    &options.credential,
                    options.server_uid,
                    &options.server_requirement,
                    options.lifetime_seconds,
                )
                .await
            })?;
            registry()
                .lock()
                .map_err(|_| Error::Internal)?
                .create(Resource::DebugClient(Arc::new(Handle {
                    closed: AtomicBool::new(false),
                    client: tokio::sync::Mutex::new(Some(client)),
                })))
                .ok_or(Error::ResourceLimit)
        })();
        result.unwrap_or_else(boundary_failure)
    }))
    .unwrap_or_else(|_| boundary_error(ErrorCode::Internal))
}
/// # Safety
/// 输入为 Debug 请求 JSON；响应仍用内核缓冲区句柄，不转移裸指针。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nodus_core_debug_ipc_command(
    handle: u64,
    input: *const u8,
    length: usize,
) -> u64 {
    clear_last_error();
    catch_unwind(AssertUnwindSafe(|| {
        let result = (|| {
            // 配额先于读取和请求执行获取，输出失败不能隐式产生新的调试状态。
            let mut output = OutputReservation::new(SMALL_OUTPUT)?;
            // SAFETY: 指针不离开本同步入口，受限反序列化后拥有数据。
            let request: Request = parse(unsafe { input_bytes(input, length)? })?;
            let client = match registry().lock().map_err(|_| Error::Internal)?.get(handle) {
                Some(Resource::DebugClient(client)) => client.clone(),
                _ => return Err(Error::InvalidState),
            };
            let reply = run_network(async move { client.command(&request).await })?;
            if let Some(error) = reply.error_info {
                set_last_error(error);
            }
            serde_json::to_writer(&mut output, &reply).map_err(|_| Error::Internal)?;
            output.finish()
        })();
        result.unwrap_or_else(boundary_failure)
    }))
    .unwrap_or_else(|_| boundary_error(ErrorCode::Internal))
}
