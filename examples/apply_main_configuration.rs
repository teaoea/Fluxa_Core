//! 主配置事务示例：发布规则/节点/限额，不监听、不接管 VPN，也不声明 Running。
//! 使用内存中的最小配置；完整示例包含后续能力，apply 会明确拒绝而非部分应用。
use nodus_core::{error::Error, outbound::Manager};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manager = Manager::default();
    let revision = manager.apply_configuration("rules: ['final,reject']")?;
    println!(
        "{}",
        serde_json::to_string(&manager.configuration_status()?)?
    );
    let failure = manager.apply_configuration("rules: ['final,unknown']");
    assert_eq!(failure, Err(Error::InvalidInput));
    assert_eq!(manager.configuration_status()?.revision, revision);
    Ok(())
}
