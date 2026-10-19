//! 需要实际签名 App Group 权限的 Apple 宿主；不应在未授权终端中冒充容器。
fn main() -> Result<(), libfluxa_core::error::Error> {
    let created = libfluxa_core::storage::initialize()?;
    println!("默认配置是否由本次创建：{created}");
    // 不输出真实文件路径、节点凭据或完整配置；初始化不启动网络接管。
    Ok(())
}
