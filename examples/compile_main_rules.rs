//! 编译主配置本地候选并查询示例目标；不下载提供者、不切换运行配置、不创建连接。
use nodus_core::{
    configuration::{Document, MAX_CONFIG_BYTES},
    routing::{Target, Transport},
};
use std::io::Read;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "example_config.yaml".into());
    let mut text = String::new();
    std::fs::File::open(path)?
        .take((MAX_CONFIG_BYTES + 1) as u64)
        .read_to_string(&mut text)?;
    let snapshot = Document::from_yaml(&text)?.compile_local_rules(1)?;
    for (host, port, transport) in [
        ("a.example.com", 53, Transport::Udp),
        ("ads.example.com", 443, Transport::Tcp),
        ("unmatched.invalid", 443, Transport::Tcp),
    ] {
        let target = Target {
            domain: Some(host.into()),
            address: Some("203.0.113.1".into()),
            port,
            transport,
        };
        println!("{}", serde_json::to_string(&snapshot.evaluate(&target)?)?);
    }
    Ok(())
}
