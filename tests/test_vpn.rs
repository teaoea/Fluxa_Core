//! 测试文件：tests/test_vpn.rs
//! 测试类型：单元测试、隔离资产准备和生命周期回归
//! 测试对象：src/vpn.rs、providers、configuration、outbound
//! 测试内容：
//! - local_candidate_is_not_activation_and_does_not_replace_live_rules：候选规则隔离且不授予接管权限。
//! - nonexistent_default_node_fails_without_candidate_or_direct_fallback：缺失兜底节点报错且不发布候选。
//! - invalid_input_and_unsupported_modes_fail_without_partial_candidate：严格拒绝及失败状态。
//! - host_sources_are_read_only_and_keep_policy：宿主文件编译，无下载或磁盘修改。
//! - failure_of_second_source_never_publishes_partial_candidate：多源失败不暴露部分候选。
//! - close_releases_slot_with_old_reference_alive：重复关闭与旧引用不占业务槽。
//! - slots_are_bounded_before_background_work：两候选准入及关闭恢复。
//! - geoip_failure_is_error_not_disabled_success：显式 GeoIP 失败不能冒充禁用。
//! - total_deadline_rejects_invalid_duration_before_work：期限边界输入拒绝；超时执行路径本轮未验证。
//!
//! 测试依据：VPN 启动规范、无节点授权和生命周期约束；CandidateReady 不代表 Preflight/Running。
//!
//! 前置条件：本机回环 HTTPS，测试内串行；不改变 HOME、系统网络、VPN 或登录会话。
use super::*;
use crate::routing::{Policy, Target, Transport};
static GATE: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
async fn completed(candidate: &Preparation) -> Status {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let status = candidate.status().unwrap();
            if status.phase != Phase::Preparing {
                return status;
            }
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
    })
    .await
    .unwrap()
}
fn client(f: &crate::test_fixture_https::Fixture) -> HttpsClient {
    let mut roots = rustls::RootCertStore::empty();
    roots.add(f.root.clone()).unwrap();
    HttpsClient::with_roots(roots).unwrap()
}
fn remote(port: u16, extra: &str) -> String {
    format!(
        "tun: {{enable: true, name: nodus0}}\nrule-providers:\n  - {{name: one, url: 'https://localhost:{port}/provider-one', policy: reject, enable: true}}\n{extra}rules: ['domain,localhost,direct', 'final,direct']"
    )
}
fn policy(manager: &Manager, host: &str) -> Policy {
    manager
        .network_snapshot()
        .unwrap()
        .router
        .as_ref()
        .unwrap()
        .evaluate(Target {
            domain: Some(host.into()),
            address: None,
            port: 443,
            transport: Transport::Tcp,
        })
        .unwrap()
        .policy
}
#[tokio::test]
async fn local_candidate_is_not_activation_and_does_not_replace_live_rules() {
    let _gate = GATE.lock().await;
    let live = Manager::default();
    let revision = live
        .apply_configuration("rules: ['domain,ads.example,reject','final,direct']")
        .unwrap();
    let candidate =
        Preparation::begin("tun: {enable: true, name: nodus0}\nrules: ['final,direct']").unwrap();
    let status = completed(&candidate).await;
    assert_eq!(status.phase, Phase::CandidateReady);
    assert!(!status.activation_permitted);
    assert_eq!(
        status.configuration.unwrap().node_count,
        0,
        "无节点仍需后续本地二次确认"
    );
    assert_eq!(live.network_snapshot().unwrap().revision, revision);
    assert_eq!(policy(&live, "ads.example"), Policy::Reject);
    assert_eq!(
        policy(&candidate.inner.manager, "ads.example"),
        Policy::Direct
    );
    candidate.close().await.unwrap();
    assert_eq!(candidate.status().unwrap().phase, Phase::Closed);
    live.stop().await.unwrap();
}
#[tokio::test]
async fn invalid_input_and_unsupported_modes_fail_without_partial_candidate() {
    let _gate = GATE.lock().await;
    for (yaml, code) in [
        ("rules: ['final,direct']\npermission: true", 2002),
        ("rules: ['final,direct']\nmode: direct\nmode: rule", 2003),
        ("rules: ['final,direct']\nsystem-proxy: {port: 1080}", 1021),
        ("rules: ['final,direct']\nproxies: [{name: broken}]", 1001),
    ] {
        let candidate = Preparation::begin(yaml).unwrap();
        let status = completed(&candidate).await;
        assert_eq!(status.phase, Phase::Failed);
        assert_eq!(status.error_info.unwrap().code.value(), code);
        assert!(!status.activation_permitted);
        assert!(status.configuration.is_none());
        candidate.close().await.unwrap();
    }
}
#[tokio::test]
async fn host_sources_are_read_only_and_keep_policy() {
    let _gate = GATE.lock().await;
    let root = std::env::temp_dir().join(format!(
        "nodus-vpn-readonly-{}",
        crate::lifecycle::identifier().unwrap()
    ));
    std::fs::create_dir(&root).unwrap();
    let config = root.join("config.yaml");
    let rules = root.join("one.yaml");
    std::fs::write(&config, remote(1, "")).unwrap();
    std::fs::write(&rules, "rules: ['domain,ads.example']").unwrap();
    let resources = crate::storage::Resources {
        rules: vec![crate::storage::NamedFile {
            name: "one".into(),
            path: rules.clone(),
        }],
        ..Default::default()
    };
    let candidate = Preparation::begin_file(config.to_str().unwrap(), resources).unwrap();
    assert_eq!(completed(&candidate).await.phase, Phase::CandidateReady);
    assert_eq!(
        policy(&candidate.inner.manager, "ads.example"),
        Policy::Reject
    );
    assert!(candidate.inner.manager.provider_status().unwrap().read_only);
    assert_eq!(
        std::fs::read_to_string(&rules).unwrap(),
        "rules: ['domain,ads.example']"
    );
    assert_eq!(std::fs::read_dir(&root).unwrap().count(), 2);
    candidate.close().await.unwrap();
    std::fs::remove_dir_all(root).unwrap();
}
#[tokio::test]
async fn failure_of_second_source_never_publishes_partial_candidate() {
    let _gate = GATE.lock().await;
    let fixture = crate::test_fixture_https::fixture().await;
    fixture.provider.bodies.lock().unwrap().insert(
        "/provider-one".into(),
        "rules: ['domain,ads.example']".into(),
    );
    fixture
        .provider
        .bodies
        .lock()
        .unwrap()
        .insert("/provider-two".into(), "not_rules: []".into());
    let config = remote(
        fixture.port,
        &format!(
            "  - {{name: two, url: 'https://localhost:{}/provider-two', policy: direct, enable: true}}\n",
            fixture.port
        ),
    );
    let candidate = Preparation::begin_with_client(&config, Some(client(&fixture))).unwrap();
    let status = completed(&candidate).await;
    assert_eq!(status.phase, Phase::Failed);
    assert!(status.configuration.is_none());
    assert!(
        candidate
            .inner
            .manager
            .network_snapshot()
            .unwrap()
            .router
            .is_none()
    );
    candidate.close().await.unwrap();
}
#[tokio::test]
async fn close_releases_slot_with_old_reference_alive() {
    let _gate = GATE.lock().await;
    let fixture = crate::test_fixture_https::fixture().await;
    let candidate =
        Preparation::begin_with_client("rules: ['final,reject']", Some(client(&fixture))).unwrap();
    let retained = candidate.inner.manager.clone();
    tokio::time::timeout(Duration::from_secs(3), async {
        let (a, b) = tokio::join!(candidate.close(), candidate.close());
        a.unwrap();
        b.unwrap();
    })
    .await
    .unwrap();
    assert_eq!(candidate.status().unwrap().phase, Phase::Closed);
    assert!(!candidate.status().unwrap().activation_permitted);
    assert!(!retained.provider_status().unwrap().active);
    let first = Preparation::begin("rules: ['final,reject']").unwrap();
    let second = Preparation::begin("rules: ['final,reject']").unwrap();
    first.close().await.unwrap();
    second.close().await.unwrap();
}
#[tokio::test]
async fn slots_are_bounded_before_background_work() {
    let _gate = GATE.lock().await;
    let first = Preparation::begin("rules: ['final,direct']").unwrap();
    let second = Preparation::begin("rules: ['final,direct']").unwrap();
    assert!(matches!(
        Preparation::begin("rules: ['final,direct']"),
        Err(Error::ResourceBudgetExceeded { .. })
    ));
    first.close().await.unwrap();
    let third = Preparation::begin("rules: ['final,reject']").unwrap();
    second.close().await.unwrap();
    third.close().await.unwrap();
}
#[tokio::test]
async fn geoip_failure_is_error_not_disabled_success() {
    let _gate = GATE.lock().await;
    let fixture = crate::test_fixture_https::fixture().await;
    let yaml = format!(
        "geoip: {{enabled: true, url: 'https://localhost:{}/invalid-mmdb'}}\nrules: ['domain,localhost,direct','final,direct']",
        fixture.port
    );
    let candidate = Preparation::begin_with_client(&yaml, Some(client(&fixture))).unwrap();
    let status = completed(&candidate).await;
    assert_eq!(status.phase, Phase::Failed);
    assert!(status.error_info.is_some());
    assert!(!status.activation_permitted);
    candidate.close().await.unwrap();
}

#[tokio::test]
async fn total_deadline_rejects_invalid_duration_before_work() {
    let _gate = GATE.lock().await;
    assert!(matches!(
        Preparation::begin_with_deadline("rules: ['final,reject']", None, Duration::ZERO),
        Err(Error::InvalidInput)
    ));
    assert!(matches!(
        Preparation::begin_with_deadline(
            "rules: ['final,reject']",
            None,
            PREPARATION_TIMEOUT + Duration::from_secs(1)
        ),
        Err(Error::InvalidInput)
    ));
}

#[tokio::test]
async fn nonexistent_default_node_fails_without_candidate_or_direct_fallback() {
    let _gate = GATE.lock().await;
    for yaml in ["rules: ['final,missing-node']", "rules: ['final,PROXY']"] {
        let candidate = Preparation::begin(yaml).unwrap();
        let status = completed(&candidate).await;
        assert_eq!(status.phase, Phase::Failed);
        assert_eq!(status.error_info.unwrap().code.value(), 1001);
        assert!(!status.activation_permitted);
        assert!(status.configuration.is_none());
        assert!(
            candidate
                .inner
                .manager
                .network_snapshot()
                .unwrap()
                .router
                .is_none()
        );
        candidate.close().await.unwrap();
    }
}
