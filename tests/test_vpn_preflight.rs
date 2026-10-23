//! 测试文件：tests/test_vpn_preflight.rs
//! 测试类型：单元测试、真实回环 TLS/DNS 与故障回归
//! 测试对象：src/vpn/preflight.rs 及共用路由、HTTPS、DNS 的前置检查入口
//! 测试内容：
//! - strict_input_rejects_before_admission：输入上限、非法探测 URL 和配置伪造拒绝。
//! - real_https_is_not_full_permission_or_authorization：真实 TLS/HTTP 成功仍不授予接管资格。
//! - reject_policy_never_contacts_probe：REJECT 不探测、不绕过策略。
//! - errors_redirects_and_rewrites_cannot_fake_success：HTTP 故障、重定向及复写不能伪造成功。
//! - wrong_tls_root_fails_closed：目标证书验证失败拒绝，不关闭校验。
//! - every_dns_upstream_is_checked_without_hosts_or_fake_ip：逐个实测上游，hosts/Fake-IP 不冒充上游。
//! - one_good_dns_server_does_not_hide_failure：任一必要上游失败阻止探测。
//! - missing_proxy_coverage_is_not_success：正式规则、主配置及插件代理复写均需覆盖，禁用复写不计必要链路。
//! - listening_proxy_is_not_a_working_chain：监听端口存在不等于协议链路可用。
//! - explicit_geoip_failure_is_not_disabled：显式库失败不报告 Disabled。
//! - cancellation_and_deadline_stop_probes：中途取消、期限及后续事务隔离。
//! - compiled_requirements_respect_mode_and_sources：只从有效快照提取模式、默认与规则出站。
//! - read_only_rule_resources_are_enforced_and_unchanged：只读规则参与探测决策，不下载或改写资源。
//!
//! 测试依据：VPN启动技术规范、只读资源所有权、生命周期完成边界；不以测试输出反推预期。
//!
//! 前置条件：串行共享事务槽、本机回环 DNS/TLS、测试根证书；不接管 VPN，不修改系统信任、路由或 DNS。
use super::super::transaction::{Phase, SystemState, TEST_GATE as GATE};
use super::*;
use std::sync::atomic::Ordering;
fn client(f: &crate::test_fixture_https::Fixture) -> HttpsClient {
    let mut roots = rustls::RootCertStore::empty();
    roots.add(f.root.clone()).unwrap();
    HttpsClient::with_roots(roots).unwrap()
}
fn start(owner: &Controller, yaml: &str, target: String, tls: HttpsClient) -> u64 {
    begin(
        owner,
        yaml,
        Resources::default(),
        &[target],
        Duration::from_secs(8),
        Some(tls),
    )
    .unwrap()
}
async fn finished(owner: &Controller, id: u64) -> Report {
    tokio::time::timeout(Duration::from_secs(12), async {
        loop {
            if matches!(
                owner.status(id).unwrap().phase,
                Phase::Failed | Phase::RollbackFailed | Phase::Stopped
            ) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(owner.status(id).unwrap().system, SystemState::Unchanged);
    owner.preflight_status(id).unwrap().unwrap()
}
fn outcome(report: &Report, check: Check) -> Outcome {
    report
        .findings
        .iter()
        .find(|f| f.check == check)
        .unwrap()
        .outcome
}
fn proxy(name: &str, port: u16) -> serde_json::Value {
    serde_json::json!({"name":name,"type":"vless","server":"127.0.0.1","port":port,"uuid":"00000000-0000-0000-0000-000000000000","flow":"xtls-rprx-vision","security":"reality","sni":"example.com","pbk":"AQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQE","sid":"01"})
}
#[tokio::test]
async fn strict_input_rejects_before_admission() {
    let _gate = GATE.lock().await;
    let owner = Controller::default();
    for target in [
        "http://example.com/",
        "https://user:password@example.com/",
        "https://example.com/?secret=x",
        "https://example.com/#x",
        "https://example.com:0/",
    ] {
        assert_eq!(
            owner.begin_preflight(
                "rules: ['final,direct']",
                Resources::default(),
                &[target.into()],
                Duration::from_secs(1)
            ),
            Err(Error::InvalidInput)
        );
    }
    assert_eq!(
        owner.begin_preflight(
            "rules: ['final,direct']",
            Resources::default(),
            &[],
            Duration::from_secs(1)
        ),
        Err(Error::InvalidInput)
    );
    assert_eq!(
        owner.begin_preflight(
            "rules: ['final,direct']",
            Resources::default(),
            &vec!["https://example.com/".into(); 33],
            Duration::from_secs(1)
        ),
        Err(Error::ResourceLimit)
    );
    let resources = Resources {
        script: Some(crate::storage::NamedFile {
            name: "x".repeat(9 * 1024 * 1024),
            path: "/private/tmp/not-read-by-preflight.js".into(),
        }),
        ..Default::default()
    };
    let error = owner
        .begin_preflight(
            "rules: ['final,direct']",
            resources,
            &["https://example.com/".into()],
            Duration::from_secs(1),
        )
        .unwrap_err();
    assert_eq!(
        error.public_info().code,
        crate::error::ErrorCode::ResourceBudgetExceeded
    );
    let f = crate::test_fixture_https::fixture().await;
    let id = start(
        &owner,
        "rules: ['final,direct']\npermission: true",
        format!("https://localhost:{}/provider-probe", f.port),
        client(&f),
    );
    let report = finished(&owner, id).await;
    assert_eq!(outcome(&report, Check::Configuration), Outcome::Failed);
    assert_eq!(f.provider.requests.load(Ordering::SeqCst), 0);
}
#[tokio::test]
async fn real_https_is_not_full_permission_or_authorization() {
    let _gate = GATE.lock().await;
    let f = crate::test_fixture_https::fixture().await;
    let owner = Controller::default();
    let id = start(
        &owner,
        "rules: ['final,direct']",
        format!("https://localhost:{}/", f.port),
        client(&f),
    );
    let report = finished(&owner, id).await;
    assert!(report.network_checks_passed);
    assert!(!report.activation_permitted);
    assert_eq!(report.transaction, id);
    assert!(
        !serde_json::to_string(&report)
            .unwrap()
            .contains("localhost")
    );
    assert_eq!(report.probes_completed, 1);
    assert_eq!(outcome(&report, Check::Permission), Outcome::Unavailable);
    assert_eq!(outcome(&report, Check::ResourceBudget), Outcome::Partial);
    assert_eq!(
        outcome(&report, Check::LocalAuthorization),
        Outcome::AuthorizationRequired
    );
    assert_eq!(report.geoip_status.unwrap().level, "warn");
    assert_eq!(owner.status(id).unwrap().failure, Some(Error::Unsupported));
    assert_eq!(
        owner.status(id).unwrap().failure_phase,
        Some(Phase::Preflight)
    );
}
#[tokio::test]
async fn reject_policy_never_contacts_probe() {
    let _gate = GATE.lock().await;
    let f = crate::test_fixture_https::fixture().await;
    let owner = Controller::default();
    let id = start(
        &owner,
        "rules: ['final,reject']",
        format!("https://localhost:{}/provider-probe", f.port),
        client(&f),
    );
    let report = finished(&owner, id).await;
    assert!(!report.network_checks_passed);
    assert_eq!(owner.status(id).unwrap().failure, Some(Error::Rejected));
    assert_eq!(f.provider.requests.load(Ordering::SeqCst), 0);
}
#[tokio::test]
async fn errors_redirects_and_rewrites_cannot_fake_success() {
    let _gate = GATE.lock().await;
    let f = crate::test_fixture_https::fixture().await;
    let owner = Controller::default();
    for path in [
        "/subscription-error",
        "/redirect",
        "/truncated",
        "/oversized",
    ] {
        let target = format!("https://localhost:{}{path}", f.port);
        let yaml=serde_json::json!({"rules":["final,direct"],"rewrites":[{"url":target,"action":"respond","response":{"status":200,"body":{"text":"fake"}}}]}).to_string();
        let id = start(&owner, &yaml, target, client(&f));
        let report = finished(&owner, id).await;
        assert_eq!(outcome(&report, Check::RulesAndAssets), Outcome::Passed);
        assert_eq!(outcome(&report, Check::RequiredOutbounds), Outcome::Failed);
        assert!(!report.network_checks_passed);
        assert_eq!(report.probes_completed, 0);
    }
}
#[tokio::test]
async fn wrong_tls_root_fails_closed() {
    let _gate = GATE.lock().await;
    let f = crate::test_fixture_https::fixture().await;
    let other = crate::test_fixture_https::fixture().await;
    let owner = Controller::default();
    let id = start(
        &owner,
        "rules: ['final,direct']",
        format!("https://localhost:{}/", f.port),
        client(&other),
    );
    let report = finished(&owner, id).await;
    assert!(!report.network_checks_passed);
    assert_eq!(owner.status(id).unwrap().failure, Some(Error::Tls));
}
#[tokio::test]
async fn every_dns_upstream_is_checked_without_hosts_or_fake_ip() {
    let _gate = GATE.lock().await;
    let f = crate::test_fixture_https::fixture().await;
    let one = crate::test_fixture_https::dns::fixture().await;
    let two = crate::test_fixture_https::dns::fixture().await;
    let owner = Controller::default();
    let yaml = format!(
        "rules: ['final,direct']\ndns:\n  enable: true\n  server: ['127.0.0.1:{}','127.0.0.1:{}']\n  hosts: {{connect.example.test: 127.0.0.1}}\n  fake-ip: {{enable: true, ipv4-range: 198.18.0.0/24, ipv6-range: 'fd00::/120'}}\n",
        one.port, two.port
    );
    let id = start(
        &owner,
        &yaml,
        format!("https://connect.example.test:{}/", f.port),
        client(&f),
    );
    let report = finished(&owner, id).await;
    assert_eq!(report.dns_upstreams_checked, 2, "{report:?}");
    assert!(report.network_checks_passed);
    assert!(one.count.load(Ordering::SeqCst) > 0);
    assert!(two.count.load(Ordering::SeqCst) > 0);
}
#[tokio::test]
async fn one_good_dns_server_does_not_hide_failure() {
    let _gate = GATE.lock().await;
    let f = crate::test_fixture_https::fixture().await;
    let one = crate::test_fixture_https::dns::fixture().await;
    let dead = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let port = dead.local_addr().unwrap().port();
    drop(dead);
    let yaml = format!(
        "rules: ['final,direct']\ndns: {{enable: true, timeout: 1, server: ['127.0.0.1:{}','127.0.0.1:{port}']}}",
        one.port
    );
    let owner = Controller::default();
    let id = start(
        &owner,
        &yaml,
        format!("https://connect.example.test:{}/", f.port),
        client(&f),
    );
    let report = finished(&owner, id).await;
    assert!(one.count.load(Ordering::SeqCst) > 0);
    assert_eq!(outcome(&report, Check::Dns), Outcome::Failed);
    assert!(!report.network_checks_passed);
    assert_eq!(report.probes_completed, 0);
}
#[tokio::test]
async fn missing_proxy_coverage_is_not_success() {
    let _gate = GATE.lock().await;
    let f = crate::test_fixture_https::fixture().await;
    let owner = Controller::default();
    let path = std::env::temp_dir().join(format!(
        "nodus-preflight-plugin-{}.yaml",
        crate::lifecycle::identifier().unwrap()
    ));
    let rewrite = serde_json::json!({"url":"https://proxy.example/**","action":"proxy(required)"});
    std::fs::write(
        &path,
        serde_json::json!({"rewrites":[rewrite.clone()]}).to_string(),
    )
    .unwrap();
    for origin in 0..4 {
        let mut configuration =
            serde_json::json!({"proxies":[proxy("required",1)],"rules":["final,direct"]});
        let mut resources = Resources::default();
        match origin {
            0 => {
                configuration["rules"] =
                    serde_json::json!(["domain,proxy.example,required", "final,direct"])
            }
            1 => configuration["rewrites"] = serde_json::json!([rewrite.clone()]),
            2 => {
                configuration["plugin-providers"] = serde_json::json!([{"name":"one","category":"test","url":"https://example.invalid/plugin","enabled":true}]);
                resources.plugins.push(crate::storage::PluginFiles {
                    name: "one".into(),
                    id: 1,
                    path: path.clone(),
                    script: None,
                });
            }
            _ => {
                configuration["rewrites"] = serde_json::json!([{"url":"https://proxy.example/**","action":"proxy(required)","enable":false}])
            }
        }
        let id = begin(
            &owner,
            &configuration.to_string(),
            resources,
            &[format!("https://localhost:{}/", f.port)],
            Duration::from_secs(8),
            Some(client(&f)),
        )
        .unwrap();
        let report = finished(&owner, id).await;
        assert_eq!(report.probes_completed, 1, "{report:?}");
        assert_eq!(report.required_outbounds, usize::from(origin < 3));
        assert_eq!(report.verified_outbounds, 0);
        assert_eq!(report.network_checks_passed, origin == 3);
    }
    std::fs::remove_file(path).unwrap();
}
#[tokio::test]
async fn listening_proxy_is_not_a_working_chain() {
    let _gate = GATE.lock().await;
    let f = crate::test_fixture_https::fixture().await;
    let owner = Controller::default();
    let server = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = server.local_addr().unwrap().port();
    let peer = tokio::spawn(async move {
        let (stream, _) = server.accept().await.unwrap();
        drop(stream);
    });
    let yaml = serde_json::json!({"proxies":[proxy("required",port)],"rules":["final,required"]})
        .to_string();
    let id = start(
        &owner,
        &yaml,
        format!("https://localhost:{}/", f.port),
        client(&f),
    );
    let report = finished(&owner, id).await;
    peer.await.unwrap();
    assert!(!report.network_checks_passed);
    assert_eq!(report.verified_outbounds, 0);
    assert_ne!(owner.status(id).unwrap().failure, None);
}
#[tokio::test]
async fn explicit_geoip_failure_is_not_disabled() {
    let _gate = GATE.lock().await;
    let f = crate::test_fixture_https::fixture().await;
    let owner = Controller::default();
    let id = start(
        &owner,
        "rules: ['final,direct']\ngeoip: {enabled: true, url: 'https://example.com/data.mmdb', update-time: '00:00'}",
        format!("https://localhost:{}/", f.port),
        client(&f),
    );
    let report = finished(&owner, id).await;
    assert!(!report.network_checks_passed);
    assert!(report.geoip_status.is_none());
    assert_eq!(outcome(&report, Check::Geoip), Outcome::Failed);
    let path = std::env::temp_dir().join(format!(
        "nodus-preflight-geoip-{}.mmdb",
        crate::lifecycle::identifier().unwrap()
    ));
    for (bytes, valid) in [
        (crate::test_fixture_https::geoip_data::fixture(), true),
        (b"invalid mmdb".to_vec(), false),
    ] {
        std::fs::write(&path, &bytes).unwrap();
        let resources = Resources {
            geoip: Some(path.clone()),
            ..Default::default()
        };
        let id = begin(&owner,"rules: ['final,direct']\ngeoip: {enabled: true, url: 'https://example.com/data.mmdb', update-time: '00:00'}",resources,&[format!("https://localhost:{}/",f.port)],Duration::from_secs(5),Some(client(&f))).unwrap();
        let report = finished(&owner, id).await;
        assert_eq!(report.network_checks_passed, valid);
        if valid {
            assert_eq!(report.geoip_status.unwrap().state, "ready");
        } else {
            assert!(report.geoip_status.is_none());
            assert_eq!(outcome(&report, Check::Geoip), Outcome::Failed);
        }
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
    }
    std::fs::remove_file(path).unwrap();
}
#[tokio::test]
async fn cancellation_and_deadline_stop_probes() {
    let _gate = GATE.lock().await;
    let f = crate::test_fixture_https::fixture().await;
    f.provider.delay.store(true, Ordering::SeqCst);
    let owner = Controller::default();
    for cancel in [true, false] {
        let id = begin(
            &owner,
            "rules: ['final,direct']",
            Resources::default(),
            &[format!("https://localhost:{}/provider-probe", f.port)],
            Duration::from_millis(100),
            Some(client(&f)),
        )
        .unwrap();
        if cancel {
            tokio::time::timeout(Duration::from_secs(2), async {
                while f.provider.requests.load(Ordering::SeqCst) == 0 {
                    tokio::time::sleep(Duration::from_millis(1)).await;
                }
            })
            .await
            .unwrap();
            assert_eq!(
                owner.begin_preflight(
                    "rules: ['final,direct']",
                    Resources::default(),
                    &["https://example.com/".into()],
                    Duration::from_secs(1)
                ),
                Err(Error::InvalidState)
            );
            owner.stop(id).await.unwrap();
        }
        let report = finished(&owner, id).await;
        assert!(!report.activation_permitted);
        assert!(!report.network_checks_passed);
        assert_eq!(
            owner.status(id).unwrap().failure,
            Some(if cancel {
                Error::Cancelled
            } else {
                Error::NetworkTimeout
            })
        );
    }
}
#[tokio::test]
async fn compiled_requirements_respect_mode_and_sources() {
    let _gate = GATE.lock().await;
    for (mode, expected) in [
        ("rule", vec!["first", "last"]),
        ("direct", vec![]),
        ("global", vec!["last"]),
    ] {
        let text=serde_json::json!({"mode":mode,"proxies":[proxy("first",1),proxy("last",2),proxy("unused",3)],"rules":["domain,one.example,first","final,last"]}).to_string();
        let document = Arc::new(Document::from_yaml(&text).unwrap());
        let prepared = document
            .prepare_transport(None, crate::configuration::PreparationPurpose::VpnCandidate)
            .unwrap();
        let actual = prepared.router.preflight_outbounds().unwrap();
        assert_eq!(actual, expected.into_iter().map(String::from).collect());
    }
}

#[tokio::test]
async fn read_only_rule_resources_are_enforced_and_unchanged() {
    let _gate = GATE.lock().await;
    let f = crate::test_fixture_https::fixture().await;
    let owner = Controller::default();
    let root = std::env::temp_dir().join(format!(
        "nodus-preflight-assets-{}",
        crate::lifecycle::identifier().unwrap()
    ));
    std::fs::create_dir(&root).unwrap();
    let path = root.join("rules.yaml");
    let yaml = "rules: ['final,direct']\nrule-providers: [{name: one, url: 'https://example.invalid/rules', policy: reject, enable: true}]";
    for (rules, passed) in [
        ("rules: ['domain,other.example']", true),
        ("rules: ['domain,localhost']", false),
    ] {
        // 写入只属于测试宿主，受测内核仅获读取授权。
        std::fs::write(&path, rules).unwrap();
        let resources = Resources {
            rules: vec![crate::storage::NamedFile {
                name: "one".into(),
                path: path.clone(),
            }],
            ..Default::default()
        };
        let id = begin(
            &owner,
            yaml,
            resources,
            &[format!("https://localhost:{}/", f.port)],
            Duration::from_secs(5),
            Some(client(&f)),
        )
        .unwrap();
        let report = finished(&owner, id).await;
        assert_eq!(report.network_checks_passed, passed);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), rules);
        assert_eq!(std::fs::read_dir(&root).unwrap().count(), 1);
    }
    std::fs::remove_dir_all(root).unwrap();
}
