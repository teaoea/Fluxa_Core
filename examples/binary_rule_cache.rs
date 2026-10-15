//! 可运行的缓存生成、重新加载及旧快照示例；不建立网络连接，不写入真实用户配置。
use libfluxa_core::routing::{
    Config, Match, Policy, Rule, Target, Transport,
    runtime::{Layer, Source},
    store::Store,
};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let directory =
        std::env::temp_dir().join(format!("fluxa-binary-example-{}", std::process::id()));
    let store = Store::open(&directory)?;
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
    store.close()?;
    drop(store);
    let reopened = Store::open(&directory)?;
    assert_eq!(
        reopened.snapshot()?.evaluate(&target)?.policy,
        Policy::Direct
    );
    println!("二进制缓存恢复成功；旧快照仍拒绝，新快照直连，示例未建立网络连接。");
    reopened.close()?;
    std::fs::remove_dir_all(directory)?;
    Ok(())
}
