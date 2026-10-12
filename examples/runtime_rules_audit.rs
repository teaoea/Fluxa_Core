//! 本地 Rust API 示例：正式规则校验、来源重算与临时调试覆盖，不声明配置调试字段。
use libfluxa_core::routing::{
    Config, Match, Policy, Reason, Router, Rule, Target, Transport,
    debug::{DebugRule, Session},
    runtime::{Layer, ReportVerification, Snapshot, Source},
};
fn rule(policy: Policy) -> Rule {
    Rule {
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
    // 仅本地可信宿主可执行此示例；网络控制端必须先完成认证和调试权限检查。
    let session = Session::new()?;
    session.replace(
        vec![DebugRule {
            rule: rule(Policy::Direct),
            protocol: Some(Transport::Tcp),
            lifetime_seconds: Some(60),
        }],
        None,
    )?;
    let router = Router::from_runtime(std::sync::Arc::new(loaded))?.with_debug(&session)?;
    assert_eq!(router.evaluate(target)?.reason, Reason::DebugRule);
    session.clear()?;
    assert!(!session.status()?.active);
    session.close()?;
    println!("正式规则来源重算及 API 临时调试示例通过；未连接真实网络");
    Ok(())
}
