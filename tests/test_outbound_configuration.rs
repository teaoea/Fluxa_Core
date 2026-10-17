//! 测试文件：tests/test_outbound_configuration.rs
//! 测试类型：单元测试、原子事务、并发和预算生命周期
//! 测试对象：src/outbound/configuration.rs
//! 测试内容：
//! - concurrent_candidates_have_one_commit_winner：同基线并发候选只能提交一个，失败方不覆盖。
//! - failed_preparation_preserves_previous_configuration：非法和未接入能力均不得部分提交。
//! - stale_preparation_cannot_overwrite_newer_configuration：旧候选不得覆盖已提交新版本。
//! - stopped_manager_rejects_prepared_candidate：关闭优先于候选提交。
//! - replacement_releases_unreferenced_old_snapshot：不保留旧快照历史，外部旧引用仍保留真实预算。
//! - partial_updates_invalidate_summary_but_retain_budget：分模块变更撤销完整性声明且节点预算不提前释放。
//! - debug_overlay_survives_main_commit：配置提交不能撤销专用授权的调试规则。
//!
//! 测试依据：配置失败保留旧状态、新请求读取新快照、Debug 最高及内存实际占用预算约定。
//!
//! 前置条件：纯内存操作，无网络、VPN、文件或系统设置副作用。
use super::*;
use crate::{
    configuration::Document,
    routing::{Policy, Target, Transport},
};
fn target() -> Target {
    Target {
        domain: None,
        address: Some("192.0.2.1".into()),
        port: 443,
        transport: Transport::Tcp,
    }
}
fn policy(manager: &Manager) -> Policy {
    manager
        .network_snapshot()
        .unwrap()
        .router
        .as_ref()
        .unwrap()
        .evaluate(target())
        .unwrap()
        .policy
}
#[test]
fn failed_preparation_preserves_previous_configuration() {
    let manager = Manager::default();
    let revision = manager
        .apply_configuration("rules: ['final,reject']")
        .unwrap();
    for (yaml, expected) in [
        ("rules: ['final,unknown']", Error::InvalidInput),
        (
            "rules: ['final,direct']\ndns: {enable: true, server: ['system']}",
            Error::Unsupported,
        ),
        (
            "rules: ['final,direct']\ndns: {enable: true, server: ['1.1.1.1']}",
            Error::Unsupported,
        ),
        (
            "rules: ['final,direct']\nsystem-proxy: {port: 1054}",
            Error::Unsupported,
        ),
        (
            "rules: ['final,direct']\nlog-level: info",
            Error::Unsupported,
        ),
        (
            "rules: ['final,direct']\nip-strategy: prefer-ipv6",
            Error::Unsupported,
        ),
        (
            "rules: ['final,direct']\nrule-providers: [{name: remote, url: 'https://example.com/rules.yaml', policy: direct, enable: true}]",
            Error::Unsupported,
        ),
    ] {
        assert_eq!(manager.apply_configuration(yaml), Err(expected));
        assert_eq!(manager.configuration_status().unwrap().revision, revision);
        assert_eq!(policy(&manager), Policy::Reject);
    }
}
#[test]
fn stale_preparation_cannot_overwrite_newer_configuration() {
    let manager = Manager::default();
    let base = manager
        .apply_configuration("rules: ['final,reject']")
        .unwrap();
    let candidate = Document::from_yaml("rules: ['final,direct']")
        .unwrap()
        .prepare()
        .unwrap();
    let next = manager
        .apply_configuration("rules: ['final,reject']")
        .unwrap();
    assert_eq!(
        manager.commit_configuration(base, candidate),
        Err(Error::InvalidState)
    );
    assert_eq!(manager.configuration_status().unwrap().revision, next);
    assert_eq!(policy(&manager), Policy::Reject);
}
#[tokio::test]
async fn stopped_manager_rejects_prepared_candidate() {
    let manager = Manager::default();
    let base = manager
        .apply_configuration("rules: ['final,reject']")
        .unwrap();
    let candidate = Document::from_yaml("rules: ['final,direct']")
        .unwrap()
        .prepare()
        .unwrap();
    manager.stop().await.unwrap();
    assert_eq!(
        manager.commit_configuration(base, candidate),
        Err(Error::InvalidState)
    );
    assert_eq!(policy(&manager), Policy::Reject);
}
#[test]
fn replacement_releases_unreferenced_old_snapshot() {
    let manager = Manager::default();
    manager
        .apply_configuration("rules: ['final,reject']")
        .unwrap();
    let old = manager.network_snapshot().unwrap();
    let old_document = Arc::downgrade(old.configuration.as_ref().unwrap());
    let old_index = Arc::downgrade(old.router.as_ref().unwrap());
    manager
        .apply_configuration("rules: ['final,direct']")
        .unwrap();
    assert_eq!(policy(&manager), Policy::Direct);
    assert_eq!(
        old.router
            .as_ref()
            .unwrap()
            .evaluate(target())
            .unwrap()
            .policy,
        Policy::Reject
    );
    assert!(old_document.upgrade().is_some());
    drop(old);
    assert!(old_document.upgrade().is_none());
    assert!(old_index.upgrade().is_none());
}
#[test]
fn partial_updates_invalidate_summary_but_retain_budget() {
    let manager = Manager::default();
    manager
        .apply_configuration("rules: ['final,reject']")
        .unwrap();
    let document = Arc::downgrade(
        manager
            .network_snapshot()
            .unwrap()
            .configuration
            .as_ref()
            .unwrap(),
    );
    manager
        .configure_limits(Limits {
            maximum_connections: 1,
            ..Limits::default()
        })
        .unwrap();
    assert!(
        manager
            .configuration_status()
            .unwrap()
            .configuration
            .is_none()
    );
    assert!(
        document.upgrade().is_some(),
        "仍共享已准备对象的快照保留配置预算"
    );
    manager
        .configure_network(None, None, Limits::default())
        .unwrap();
    assert!(document.upgrade().is_none());
}
#[test]
fn debug_overlay_survives_main_commit() {
    let manager = Manager::default();
    manager
        .apply_configuration("rules: ['final,reject']")
        .unwrap();
    let session = Arc::new(crate::routing::debug::Session::new().unwrap());
    manager
        .replace_debug(
            session,
            vec![crate::routing::debug::DebugRule {
                // 调试对象的具体字段由现有公开规则模型构造，不能通过 YAML 注入。
                rule: crate::routing::Rule {
                    r#type: crate::routing::Match::Domain,
                    value: "example.com".into(),
                    policy: Policy::Direct,
                    outbound: None,
                    protocol: None,
                },
                protocol: None,
                lifetime_seconds: Some(60),
            }],
            None,
        )
        .unwrap();
    manager
        .apply_configuration("rules: ['final,reject']")
        .unwrap();
    let mut request = target();
    request.address = None;
    request.domain = Some("example.com".into());
    assert_eq!(
        manager
            .network_snapshot()
            .unwrap()
            .router
            .as_ref()
            .unwrap()
            .evaluate(request)
            .unwrap()
            .policy,
        Policy::Direct
    );
}

#[test]
fn concurrent_candidates_have_one_commit_winner() {
    let manager = Arc::new(Manager::default());
    let base = manager
        .apply_configuration("rules: ['final,reject']")
        .unwrap();
    let barrier = Arc::new(std::sync::Barrier::new(2));
    let workers: Vec<_> = ["rules: ['final,direct']", "rules: ['final,reject']"]
        .into_iter()
        .map(|yaml| {
            let candidate = Document::from_yaml(yaml).unwrap().prepare().unwrap();
            let manager = manager.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                manager.commit_configuration(base, candidate)
            })
        })
        .collect();
    let results: Vec<_> = workers
        .into_iter()
        .map(|worker| worker.join().unwrap())
        .collect();
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|result| **result == Err(Error::InvalidState))
            .count(),
        1
    );
    assert_eq!(
        manager.configuration_status().unwrap().revision,
        *results
            .iter()
            .find_map(|result| result.as_ref().ok())
            .unwrap()
    );
}
