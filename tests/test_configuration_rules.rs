//! 测试文件：tests/test_configuration_rules.rs
//! 测试类型：单元测试、规则输入契约
//! 测试对象：src/configuration/rules.rs
//! 测试内容：
//! - process_rule_syntax_is_exact_case_preserving_and_not_wildcard：按新配置契约独立验证正反边界。
//! - final_is_unique_last_and_required：兜底唯一且末尾必填。
//! - scoped_rules_and_unscoped_reject：组合条件合法、全域协议仅拦截。
//! - malformed_rules_are_not_partially_accepted：非法类型、值域、字段数及引用整体拒绝。
//!
//! 测试依据：用户已确认的主配置标量规则与协议约束；不验证尚未接入的匹配优先级。
//!
//! 前置条件：无网络或系统资源需求。
use super::*;
fn check(rules: &[&str]) -> Result<()> {
    validate(
        &rules.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
        &HashSet::from(["PROXY"]),
    )
}
#[test]
fn final_is_unique_last_and_required() {
    assert!(check(&["final,direct"]).is_ok());
    for rules in [
        vec![],
        vec!["domain,example.com,direct"],
        vec!["final,direct", "domain,example.com,reject"],
        vec!["final,direct", "final,PROXY"],
        vec!["final,"],
    ] {
        assert!(check(&rules).is_err());
    }
}
#[test]
fn scoped_rules_and_unscoped_reject() {
    for rule in [
        "port,80,direct",
        "port,53,example.com,direct",
        "protocol,tcp,example.com,PROXY",
        "protocol,quic,reject",
        "domain,example.com,direct",
        "domain-suffix,example.com,reject",
        "domain-keyword,example,direct",
        "ip-cidr,2001:db8::/32,PROXY",
        "geoip,CN,direct",
    ] {
        assert!(check(&[rule, "final,reject"]).is_ok(), "{rule}");
    }
    for rule in [
        "protocol,tcp,direct",
        "protocol,quic,PROXY",
        "protocol,udp,direct",
    ] {
        assert!(check(&[rule, "final,direct"]).is_err());
    }
}
#[test]
fn malformed_rules_are_not_partially_accepted() {
    for rule in [
        "DOMAIN,example.com,direct",
        "debug,example.com,direct",
        "port,0,direct",
        "port,65536,direct",
        "protocol,http,example.com,direct",
        "port,53,*.example.com,direct",
        "domain,1.1.1.1,direct",
        "domain,example.com,missing",
        "domain,example.com,proxy,PROXY",
        "ip-cidr,broken,direct",
        "geoip,China,direct",
        "domain,,direct",
    ] {
        assert!(check(&[rule, "final,direct"]).is_err(), "{rule}");
    }
}

#[test]
fn process_rule_syntax_is_exact_case_preserving_and_not_wildcard() {
    for name in ["WeChat", "wechat", "App Helper"] {
        assert!(check(&[&format!("process-name,{name},direct"), "final,reject"]).is_ok());
    }
    for name in ["We*Chat", "/Applications/WeChat", "bad\nname"] {
        assert!(check(&[&format!("process-name,{name},direct"), "final,reject"]).is_err());
    }
}
