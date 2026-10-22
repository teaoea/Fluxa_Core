//! 正式域名 + 传输协议 AND 示例；只执行决策，不建立真实连接。
use nodus_core::routing::{Config, Policy, Router, Rule, Target, Transport};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 既有 JSON / YAML 没有 protocol 时保持原行为；新增字段仅允许 tcp、udp。
    let rules: Vec<Rule> = serde_json::from_str(
        r#"[
        {"type":"DOMAIN-SUFFIX","value":"example.net","protocol":"udp","policy":"reject"},
        {"type":"DOMAIN","value":"api.example.net","policy":"direct"}
    ]"#,
    )?;
    let router = Router::compile(Config {
        rules,
        ..Config::default()
    })?;
    for (transport, expected) in [
        (Transport::Tcp, Policy::Direct),
        (Transport::Udp, Policy::Reject),
    ] {
        let decision = router.evaluate(Target {
            domain: Some("api.example.net".into()),
            address: None,
            port: 443,
            transport,
        })?;
        assert_eq!(decision.policy, expected);
        println!("{transport:?} => {:?}", decision.policy);
    }
    Ok(())
}
