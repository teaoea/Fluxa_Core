//! 默认仅使用 hosts，不下载、不改系统设置或真实用户缓存。
//! 传入配置路径及可选资源 JSON 清单后只读加载；宿主负责资源下载和保存。
use nodus_core::{configuration::MAX_CONFIG_BYTES, dns::Query, outbound::Manager};
use std::{io::Read, sync::Arc};
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let selected = std::env::args().nth(1);
    let yaml = if let Some(path) = &selected {
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
        if let Some(path) = selected {
            let resources = if let Some(manifest) = std::env::args().nth(2) {
                serde_json::from_str(&std::fs::read_to_string(manifest)?)?
            } else {
                Default::default()
            };
            manager.apply_configuration_file(&path, resources).await?;
        } else {
            manager.apply_configuration_async(&yaml).await?;
        }
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
