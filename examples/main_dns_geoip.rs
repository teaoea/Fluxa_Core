//! 默认仅使用 hosts，不下载、不改系统设置或真实用户缓存。
//! 显式传入配置路径后，才按该配置启用 DNS、GeoIP 和提供者下载。
use libfluxa_core::{configuration::MAX_CONFIG_BYTES, dns::Query, outbound::Manager};
use std::{io::Read, sync::Arc};
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let yaml = if let Some(path) = std::env::args().nth(1) {
        let mut text = String::new();
        std::fs::File::open(path)?
            .take((MAX_CONFIG_BYTES + 1) as u64)
            .read_to_string(&mut text)?;
        text
    } else {
        "dns: {enable: true, server: ['127.0.0.1:9'], hosts: {demo.example: 192.0.2.1}}\ngeoip: {enabled: false}\nrules: ['final,reject']".into()
    };
    let manager = Arc::new(Manager::default());
    let result = async {
        manager.apply_configuration_async(&yaml).await?;
        println!(
            "{}",
            serde_json::to_string(&manager.configuration_status()?)?
        );
        if let Some(resolver) = &manager.network_snapshot()?.resolver {
            println!(
                "{}",
                serde_json::to_string(
                    &resolver
                        .resolve(Query {
                            name: "demo.example".into(),
                            query_type: "A".into(),
                        })
                        .await?
                )?
            );
        }
        Ok::<(), Box<dyn std::error::Error>>(())
    }
    .await;
    // 错误也等待内核停止，不以 drop 代替活动连接和后台计划的确定性关闭。
    let closed = manager.stop().await;
    result?;
    closed?;
    Ok(())
}
