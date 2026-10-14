//! 无正文、无凭据的固定诊断环；显式清理失败与底层 I/O 分类不会静默消失。
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
pub const DIAGNOSTIC_CAPACITY: usize = 64;
struct Ring {
    records: [Option<Diagnostic>; DIAGNOSTIC_CAPACITY],
    next: usize,
    overwritten: u64,
}
static RING: Mutex<Ring> = Mutex::new(Ring {
    records: [None; DIAGNOSTIC_CAPACITY],
    next: 0,
    overwritten: 0,
});
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
pub(super) fn record_io(error: &io::Error, code: ErrorCode) {
    append(Diagnostic {
        code,
        site: DiagnosticSite::IoConversion,
        io_kind: Some(error.kind()),
        raw_os_error: error.raw_os_error(),
    });
}
fn append(record: Diagnostic) {
    // 专用诊断锁中毒后恢复仅这个固定环，不恢复任何业务锁或失效快照。
    let mut ring = RING
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let index = ring.next;
    if ring.records[index].is_some() {
        ring.overwritten = ring.overwritten.saturating_add(1);
    }
    ring.records[index] = Some(record);
    ring.next = (index + 1) % DIAGNOSTIC_CAPACITY;
}
/// 固定大小快照不会清空记录；覆盖数量明确可见，诊断不会形成无界日志。
pub fn snapshot() -> ([Option<Diagnostic>; DIAGNOSTIC_CAPACITY], u64) {
    let ring = RING
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    (ring.records, ring.overwritten)
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
