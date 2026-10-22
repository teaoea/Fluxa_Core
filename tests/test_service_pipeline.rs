//! 测试文件：tests/test_service_pipeline.rs
//! 测试类型：集成测试、回归测试
//! 测试对象：src/http.rs、src/https.rs
//! 测试内容：
//! - service_requires_explicit_route_and_dns：依据现行独立业务约定验证；新来源/模式约定的输入与预期在同名函数明确说明。
//! - configured_rejection_never_dials_and_reconfiguration_recovers：依据现行独立业务约定验证；新来源/模式约定的输入与预期在同名函数明确说明。
//! - unsupported_proxy_is_not_direct：依据现行独立业务约定验证；新来源/模式约定的输入与预期在同名函数明确说明。
//! - cancellation_interrupts_tls_and_plain_requests_and_releases_resources：依据现行独立业务约定验证；新来源/模式约定的输入与预期在同名函数明确说明。
//! - idle_expiry_also_interrupts_managed_tls：依据现行独立业务约定验证；新来源/模式约定的输入与预期在同名函数明确说明。
//!
//! 测试依据：保留原有公开接口契约及全部断言；测试文件原注释保留在下方，L3 必须独立核对。
//!
//! 前置条件：本地测试需文件/回环套接字权限；平台限定及忽略项以实际登记为准。
//!
//! 执行状态：由同名 Markdown 报告记录，不在源码中固化通过结论。

// 服务层与共用连接管线的真实 I/O 验证，不改动系统 DNS 或证书信任。
#[path = "test_fixture_https.rs"]
mod support;
use nodus_core::{
    error::Error,
    https::HttpsClient,
    outbound::Limits,
    routing::{Config, Mode, Policy, Router},
};
use rustls::RootCertStore;
use std::{sync::Arc, time::Duration};
use tokio::time::{sleep, timeout};

fn client(fixture: &support::Fixture) -> HttpsClient {
    let mut roots = RootCertStore::empty();
    roots.add(fixture.root.clone()).unwrap();
    HttpsClient::with_roots(roots)
        .unwrap()
        .with_network(Arc::clone(&fixture.network))
}

#[tokio::test]
async fn service_requires_explicit_route_and_dns() {
    let fixture = support::fixture().await;
    let client = HttpsClient::with_roots(RootCertStore::empty()).unwrap();
    let url = format!("https://localhost:{}/", fixture.port);
    assert!(matches!(
        client.execute("GET", &url, &[], &[]).await,
        Err(Error::InvalidState)
    ));
    client
        .network()
        .configure_routing(Some(Arc::new(
            Router::compile(Config {
                mode: Mode::Direct,
                ..Config::default()
            })
            .unwrap(),
        )))
        .unwrap();
    assert!(client.execute("GET", &url, &[], &[]).await.is_err());
    assert!(client.network().status().unwrap().sessions.is_empty());
}

#[tokio::test]
async fn configured_rejection_never_dials_and_reconfiguration_recovers() {
    let fixture = support::fixture().await;
    let client = client(&fixture);
    fixture
        .network
        .configure_routing(Some(Arc::new(Router::compile(Config::default()).unwrap())))
        .unwrap();
    let url = format!("https://localhost:{}/", fixture.port);
    assert!(matches!(
        client.execute("GET", &url, &[], &[]).await,
        Err(Error::Rejected)
    ));
    assert!(fixture.network.status().unwrap().sessions.is_empty());
    fixture
        .network
        .configure_routing(Some(Arc::new(
            Router::compile(Config {
                mode: Mode::Direct,
                ..Config::default()
            })
            .unwrap(),
        )))
        .unwrap();
    assert_eq!(
        client.execute("GET", &url, &[], &[]).await.unwrap().status,
        200
    );
    assert!(fixture.network.status().unwrap().sessions.is_empty());
}

#[tokio::test]
async fn unsupported_proxy_is_not_direct() {
    let fixture = support::fixture().await;
    fixture
        .network
        .configure_routing(Some(Arc::new(
            Router::compile(Config {
                mode: Mode::Global,
                fallback: Policy::Proxy, // 显式代理兜底，仍必须拒绝未实现代理，不能默认直连。
                outbound: Some("unimplemented".into()),
                ..Config::default()
            })
            .unwrap(),
        )))
        .unwrap();
    assert!(matches!(
        client(&fixture)
            .execute(
                "GET",
                &format!("https://localhost:{}/", fixture.port),
                &[],
                &[]
            )
            .await,
        Err(Error::Unsupported)
    ));
    assert!(fixture.network.status().unwrap().sessions.is_empty());
}

#[tokio::test]
async fn cancellation_interrupts_tls_and_plain_requests_and_releases_resources() {
    let fixture = support::fixture().await;
    let plain = support::plain_fixture().await;
    for url in [
        format!("https://localhost:{}/wait", fixture.port),
        format!("http://localhost:{}/wait", plain.port),
    ] {
        let client = client(&fixture);
        let request =
            tokio::spawn(async move { client.execute_request("GET", &url, &[], &[], None).await });
        timeout(Duration::from_secs(2), async {
            loop {
                if fixture
                    .network
                    .status()
                    .unwrap()
                    .sessions
                    .first()
                    .is_some_and(|session| session.uploaded > 0)
                {
                    break;
                }
                sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap();
        fixture.network.close_all().await.unwrap();
        assert!(matches!(
            timeout(Duration::from_secs(2), request)
                .await
                .unwrap()
                .unwrap(),
            Err(Error::Cancelled)
        ));
        assert!(fixture.network.status().unwrap().sessions.is_empty());
    }
}

#[tokio::test]
async fn idle_expiry_also_interrupts_managed_tls() {
    let fixture = support::fixture().await;
    fixture
        .network
        .configure_limits(Limits {
            maximum_connections: 1,
            idle_timeout_ms: 100,
        })
        .unwrap();
    let result = timeout(
        Duration::from_secs(2),
        client(&fixture).execute(
            "GET",
            &format!("https://localhost:{}/wait", fixture.port),
            &[],
            &[],
        ),
    )
    .await
    .unwrap();
    assert!(matches!(result, Err(Error::Cancelled)));
    assert!(fixture.network.status().unwrap().sessions.is_empty());
}
