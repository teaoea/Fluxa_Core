//! 仅实时输出当前结构化诊断；接收方持有队列，Core不保留诊断历史。
use super::{Error, ErrorCode, Result};
use std::{io, sync::Mutex};
/// 诊断位置是集中定义的业务标识，不公开内部路径或对象地址。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticSite {
    IoConversion,
    Cleanup,
    DnsObservation,
    DnsCache,
    DnsService,
    MitmCapture,
    BackgroundJob,
    OutboundReaper,
    Abi,
    GeoipDownload,
    GeoipQuery,
}
/// 只保留受控分类；原始错误字符串与完整错误链不进入公开诊断。
#[derive(Debug, Clone, Copy)]
pub struct Diagnostic {
    pub code: ErrorCode,
    pub site: DiagnosticSite,
    pub io_kind: Option<io::ErrorKind>,
    pub raw_os_error: Option<i32>,
}
impl Diagnostic {
    /// 等级由中央语义分类决定，不解析错误文本，不存重复的动态字段。
    pub const fn level(self) -> super::LogLevel {
        self.code.level()
    }
}
static OUTPUT: Mutex<Option<std::sync::mpsc::SyncSender<Diagnostic>>> = Mutex::new(None);
/// 受信宿主提供发送端；接收方自行缓冲/保存，None解除输出。没有历史读取入口。
pub fn set_output(sender: Option<std::sync::mpsc::SyncSender<Diagnostic>>) -> Result<()> {
    *OUTPUT.lock().map_err(|_| Error::Internal)? = sender;
    Ok(())
}
pub fn record_code(code: ErrorCode, site: DiagnosticSite) {
    append(Diagnostic {
        code,
        site,
        io_kind: None,
        raw_os_error: None,
    });
}
pub fn record(error: Error, site: DiagnosticSite) {
    append(Diagnostic {
        code: error.code(),
        site,
        io_kind: None,
        raw_os_error: None,
    });
}
pub(crate) fn record_io(error: &io::Error, code: ErrorCode) {
    append(Diagnostic {
        code,
        site: DiagnosticSite::IoConversion,
        io_kind: Some(error.kind()),
        raw_os_error: error.raw_os_error(),
    });
}
fn append(record: Diagnostic) {
    // 竞争、无接收方、满队列或失联均不阻塞业务，也不缓存/重试当前事件。
    if let Ok(output) = OUTPUT.try_lock()
        && let Some(output) = output.as_ref()
    {
        let _ = output.try_send(record);
    }
}
/// 保留原 Result；仅记录失败。用于原先的尽力清理/可选缓存，不制造成功策略。
pub(crate) fn observe<T>(result: Result<T>, site: DiagnosticSite) -> Result<T> {
    if let Err(error) = &result {
        record(*error, site);
    }
    result
}

/// 主动 abort 的取消是关闭流程状态；非取消的任务失败必须记录。
pub(crate) fn observe_join(
    result: std::result::Result<(), tokio::task::JoinError>,
    site: DiagnosticSite,
) {
    if let Err(error) = result
        && !error.is_cancelled()
    {
        record(Error::Internal, site);
    }
}
