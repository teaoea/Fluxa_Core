//! 整数错误协议与已授权 Rust 字段迁移示例；不发起网络请求或读取配置。
use libfluxa_core::{
    error::{Error, ErrorCode, ErrorInfo},
    mitm::capture::Capture,
    routing::background::{Job, Phase},
};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let error = Error::ResourceLimit;
    let info = error.public_info();
    assert_eq!(info.code, ErrorCode::ResourceLimit);
    assert_eq!(info.code.value(), 1006);
    assert_eq!(std::mem::size_of_val(&info), 2);
    println!("{}", serde_json::to_string(&info)?);
    // 旧结构体字面量新增这一项；成功时填 None，失败填真实 Error 的结构化对象。
    let job = Job {
        id: 1,
        phase: Phase::Failed,
        version: None,
        directory_synced: None,
        error: Some(error.to_string()),
        error_info: Some(info),
    };
    assert_eq!(
        job.error_info.map(|value| value.code),
        Some(ErrorCode::ResourceLimit)
    );
    // 推荐用现有构造器初始化 Capture，新增可选字段不影响该调用形式。
    let mut capture = Capture::new();
    capture.error = Some(Error::Rejected.to_string());
    capture.error_info = Some(Error::Rejected.public_info());
    let decoded = ErrorInfo::from_json(info.code.json_bytes())?;
    assert_eq!(decoded, info);
    println!("整数分类、结构化往返与 Job/Capture 字段迁移通过；无真实网络或系统隧道验证。");
    Ok(())
}
