//! 测试文件：tests/test_providers_document.rs
//! 测试类型：单元测试、非法规则权限边界
//! 测试对象：src/providers/document.rs
//! 测试内容：
//! - conditions_inherit_only_provider_policy_and_reject_scope_is_limited：自带策略/范围越权单行忽略，合法条件继承策略。
//! - malformed_structure_is_transaction_error_not_empty_success：重复/未知根字段、别名、非规则文档整体错误。
//!
//! 测试依据：rules 小写格式、正文无策略、REJECT 允许类型与 Debug 隔离。
//! 前置条件：无网络或文件系统。
use super::*;
#[test]
fn conditions_inherit_only_provider_policy_and_reject_scope_is_limited() {
    let yaml = "rules: ['domain,one.example', 'domain,one.example,direct', 'domain-suffix,wide.example', 'domain-keyword,track', 'protocol,quic', 'protocol,quic,one.example', 'protocol,quic,direct', 'port,443,one.example', 'debug,one.example', 'final,direct', 42]";
    let parsed = parse(yaml, Policy::Reject, None, false, &[]).unwrap();
    assert_eq!(parsed.rules.len(), 4);
    assert_eq!(parsed.ignored, 7);
    assert!(
        parsed
            .rules
            .iter()
            .all(|r| r.policy == Policy::Reject && r.outbound.is_none())
    );
    let parsed = parse(yaml, Policy::Proxy, Some("Node".into()), false, &["Node"]).unwrap();
    assert_eq!(parsed.rules.len(), 5);
    assert_eq!(parsed.ignored, 6);
    assert!(
        parsed
            .rules
            .iter()
            .all(|r| r.policy == Policy::Proxy && r.outbound.as_deref() == Some("Node"))
    );
    for policy in [Policy::Direct, Policy::Reject, Policy::Proxy] {
        let outbound = (policy == Policy::Proxy).then(|| "Node".to_owned());
        let parsed = parse(
            "rules: ['process-name,WeChat', 'process-name,WeChat,direct']",
            policy,
            outbound,
            false,
            &["Node"],
        )
        .unwrap();
        assert_eq!(
            parsed.rules.len(),
            0,
            "所有提供者策略都必须忽略远程进程条件"
        );
        assert_eq!(parsed.ignored, 2);
    }
    assert!(
        parse(
            "rules: ['port,0', 'protocol,udp', 'GEOIP,CN', 'geoip,CN']",
            Policy::Direct,
            None,
            false,
            &[]
        )
        .unwrap()
        .rules
        .is_empty()
    );
}
#[test]
fn malformed_structure_is_transaction_error_not_empty_success() {
    for yaml in [
        "rules: [",
        "rules: []\nrules: []",
        "rules: []\ndebug: true",
        "payload: ['DOMAIN,one.example']",
        "rules: &r []",
    ] {
        assert!(
            parse(yaml, Policy::Direct, None, false, &[]).is_err(),
            "{yaml}"
        );
    }
    assert!(
        parse("rules: []", Policy::Direct, None, false, &[])
            .unwrap()
            .rules
            .is_empty()
    );
}
