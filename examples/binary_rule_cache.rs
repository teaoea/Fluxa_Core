//! 可运行的缓存生成、重新加载及旧快照示例；不建立网络连接，不写入真实用户配置。
use nodus_core::routing::{
    Config, Match, Policy, Rule, Target, Transport,
    runtime::{Layer, Source},
    store::Store,
};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let directory =
        std::env::temp_dir().join(format!("nodus-binary-example-{}", std::process::id()));
    std::fs::create_dir(&directory)?;
    let path = directory.join("rules.bin");
    let store = Store::memory();
    let source = |policy| Source {
        id: "local".into(),
        layer: Layer::Local,
        enabled: true,
        rules: vec![Rule {
            protocol: None,
            r#type: Match::DomainSuffix,
            value: "example.com".into(),
            policy,
            outbound: None,
        }],
    };
    // 本示例是 CLI 宿主；Core 只编译和返回内存编码，宿主决定保存。
    store.update(Config::default(), vec![source(Policy::Reject)])?;
    let old = store.snapshot()?;
    store.update(Config::default(), vec![source(Policy::Direct)])?;
    let target = Target {
        domain: Some("www.example.com".into()),
        address: None,
        port: 443,
        transport: Transport::Tcp,
    };
    assert_eq!(old.evaluate(&target)?.policy, Policy::Reject);
    assert_eq!(store.snapshot()?.evaluate(&target)?.policy, Policy::Direct);
    std::fs::write(&path, store.snapshot()?.to_binary()?.as_ref())?;
    store.close()?;
    drop(store);
    let reopened = Store::open(&path)?;
    assert_eq!(
        reopened.snapshot()?.evaluate(&target)?.policy,
        Policy::Direct
    );
    println!("二进制缓存恢复成功；旧快照仍拒绝，新快照直连，示例未建立网络连接。");
    reopened.close()?;
    std::fs::remove_dir_all(directory)?;
    Ok(())
}
