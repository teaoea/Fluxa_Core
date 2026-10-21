//! 规则提供者示例：默认仅验证本地正文，无网络或 HOME 副作用。
//! 传入显式配置路径才提交；可选第二参数为刷新名称，* 表示刷新全部；最后由内核完成关闭。
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
    let yaml = std::fs::read_to_string(path)?;
    let manager = std::sync::Arc::new(Manager::default());
    let outcome = async {
        manager.apply_configuration_async(&yaml).await?;
        if let Some(name) = args.next() {
            manager
                .refresh_providers((name != "*").then_some(name))
                .await?;
        }
        println!("{}", serde_json::to_string(&manager.provider_status()?)?);
        Ok::<(), Box<dyn std::error::Error>>(())
    }
    .await;
    let closed = manager.stop().await;
    outcome?;
    closed?;
    Ok(())
}
