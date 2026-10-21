//! 专用 IPC 客户端示例：凭证由管理员安全配置，本示例不创建网络监听或绕过授权。
#[cfg(target_os = "macos")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    use fluxa_core::routing::debug::ipc::{Client, Request};
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    if arguments.len() != 5 {
        return Err("参数：端点绝对路径 凭证绝对路径 服务UID 服务代码签名requirement 授权秒数；路径不得经过符号链接".into());
    }
    let mut client = Client::connect(
        std::path::Path::new(&arguments[0]),
        std::path::Path::new(&arguments[1]),
        arguments[2].parse()?,
        &arguments[3],
        arguments[4].parse()?,
    )
    .await?;
    let reply = client.command(&Request::Status {}).await?;
    println!("{}", serde_json::to_string(&reply)?);
    if !reply.success {
        return Err("状态查询被内核明确拒绝，详见返回的整数错误代码".into());
    }
    Ok(())
}
#[cfg(not(target_os = "macos"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    Err("本示例当前仅在 macOS 支持系统代码签名验证，不降级为仅检查 UID".into())
}
