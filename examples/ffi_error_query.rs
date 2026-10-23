//! 演示同线程 ABI 错误查询与确定性句柄释放；不触网、不读写配置、不改变系统设置。
use nodus_core::{
    error::{ErrorCode, ErrorInfo, MAX_ERROR_BYTES},
    ffi,
};
#[allow(unsafe_code)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let handle = ffi::nodus_core_create();
    assert_ne!(handle, 0);
    assert_eq!(
        ffi::nodus_core_start(0),
        i32::from(ErrorCode::InvalidHandle.value())
    );
    // SAFETY: 空目标仅查询所需长度，内核不会解引用。
    let needed = unsafe { ffi::nodus_core_last_error_copy(std::ptr::null_mut(), 0) };
    assert!(needed > 1 && needed <= MAX_ERROR_BYTES);
    let mut short = [0xa5u8; 1];
    // SAFETY: 一字节数组独占可写；容量不足时不得写入任何字节。
    let short_length = unsafe { ffi::nodus_core_last_error_copy(short.as_mut_ptr(), short.len()) };
    assert_eq!(short_length, needed);
    assert_eq!(short, [0xa5]);
    assert_eq!(
        ffi::nodus_core_last_error_code(),
        ErrorCode::InvalidHandle.value()
    );
    let mut output = [0u8; MAX_ERROR_BYTES];
    // SAFETY: 固定数组容量足够，调用期间独占，与内核缓冲区不重叠。
    let copied = unsafe { ffi::nodus_core_last_error_copy(output.as_mut_ptr(), output.len()) };
    assert_eq!(copied, needed);
    assert_eq!(
        ErrorInfo::from_json(&output[..copied])?.code,
        ErrorCode::InvalidHandle
    );
    assert_eq!(ffi::nodus_core_destroy(handle), 0);
    assert_eq!(
        ffi::nodus_core_destroy(handle),
        i32::from(ErrorCode::InvalidHandle.value())
    );
    println!("ABI 错误长度、容量不足不写入、原错误保持及唯一句柄释放验证通过。");
    Ok(())
}
