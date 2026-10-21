//! 规则提供者示例：默认仅验证本地正文，无网络或 HOME 副作用。
//! 参数为绝对配置路径及可选 JSON 资源清单路径；宿主下载落盘，Core 只读；最后由内核完成关闭。
use fluxa_core::{outbound::Manager, providers, routing::Policy};
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let Some(path) = args.next() else {
        println!(
            "{}",
            serde_json::to_string(&providers::validate_rules(
                include_str!("../rules.yaml"),
                Policy::Direct,
                None
            )?)?
        );
        return Ok(());
    };
    let resources = if let Some(manifest) = args.next() {
        serde_json::from_str(&std::fs::read_to_string(manifest)?)?
    } else {
        fluxa_core::storage::Resources::default()
    };
    let manager = std::sync::Arc::new(Manager::default());
    let outcome = async {
        manager.apply_configuration_file(&path, resources).await?;
        println!("{}", serde_json::to_string(&manager.provider_status()?)?);
        Ok::<(), Box<dyn std::error::Error>>(())
    }
    .await;
    let closed = manager.stop().await;
    outcome?;
    closed?;
    Ok(())
}
