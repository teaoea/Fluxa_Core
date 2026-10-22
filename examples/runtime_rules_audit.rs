//! 本地 Rust API 示例：正式规则校验、来源重算与临时调试覆盖，不声明配置调试字段。
use nodus_core::routing::{
    Config, Match, Policy, Rule, Target, Transport,
    runtime::{Layer, ReportVerification, Snapshot, Source},
};
fn rule(policy: Policy) -> Rule {
    Rule {
        protocol: None,
        r#type: Match::Domain,
        value: "example.com".into(),
        policy,
        outbound: None,
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let settings = Config::default();
    let sources = vec![Source {
        id: "local".into(),
        layer: Layer::Local,
        enabled: true,
        rules: vec![rule(Policy::Reject)],
    }];
    let snapshot = Snapshot::generate(settings, sources.clone(), 1)?;
    let loaded = Snapshot::from_yaml(&snapshot.to_yaml()?)?;
    assert_eq!(
        loaded.report_verification(),
        ReportVerification::StructuralOnly
    );
    loaded.verify_against_sources(sources)?;
    let target = Target {
        domain: Some("example.com".into()),
        address: Some("192.0.2.1".into()),
        port: 443,
        transport: Transport::Tcp,
    };
    assert_eq!(loaded.evaluate(&target)?.policy, Policy::Reject);
    println!("正式规则来源重算通过；临时 Debug 必须通过独立授权 IPC，见 debug_ipc 示例");
    Ok(())
}
