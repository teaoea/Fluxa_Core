//! 测试文件：tests/test_provider_contract.rs
//! 测试类型：集成测试、公开正文与配置错误契约
//! 测试对象：providers、configuration、outbound、error 的公开边界
//! 测试内容：
//! - remote_conditions_cannot_supply_policy_or_debug_authority：公开预验证不接受正文策略/调试入口。
//! - invalid_provider_configuration_never_creates_runtime：非法主配置拒绝且不创建文件/运行状态。
//!
//! 测试依据：正文只含条件、无 Debug 能力、配置事务先校验后副作用。
//! 前置条件：无外网，不修改 HOME 或系统网络。
use nodus_core::{outbound::Manager, providers, routing::Policy};
#[test]
fn remote_conditions_cannot_supply_policy_or_debug_authority() {
    let result=providers::validate_rules("rules: ['domain,one.example', 'domain,one.example,direct', 'debug,one.example', 'protocol,udp,direct', 'domain-suffix,wide.example', 'process-name,WeChat']",Policy::Reject,None).unwrap();
    assert_eq!(result.accepted, 1);
    assert_eq!(result.ignored, 5);
    assert!(providers::validate_rules("rules: []\ndebug: true", Policy::Reject, None).is_err());
    assert!(
        providers::validate_rules("payload: ['DOMAIN,one.example']", Policy::Direct, None).is_err()
    );
}
#[tokio::test]
async fn invalid_provider_configuration_never_creates_runtime() {
    let manager = std::sync::Arc::new(Manager::default());
    let revision = manager.network_snapshot().unwrap().revision;
    for yaml in [
        "rule-providers: [{}]",
        "rule-providers: [{name: ../bad, url: https://example.com/x, policy: direct, enable: true}]",
        "rule-providers: [{name: one, url: http://example.com/x, policy: direct, enable: true}]",
        "rule-providers: [{name: one, url: https://example.com/x, policy: missing, enable: true}]",
    ] {
        assert!(manager.apply_configuration_async(yaml).await.is_err());
        assert_eq!(manager.network_snapshot().unwrap().revision, revision);
        assert!(!manager.provider_status().unwrap().active);
    }
    manager.stop().await.unwrap();
}
