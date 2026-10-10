//! 服务层与共用连接管线的真实 I/O 验证，不改动系统 DNS 或证书信任。
mod support;
use libfluxa_core::{
    error::Error,
    https::HttpsClient,
    outbound::Limits,
    routing::{Config, Mode, Router},
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
        fixture.network.close_all().unwrap();
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
