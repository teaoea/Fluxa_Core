//! 测试文件：tests/test_mitm.rs
//! 测试类型：集成测试、回归测试
//! 测试对象：src/mitm/mod.rs
//! 测试内容：
//! - intercepts_real_tls_and_captures_both_bodies：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - unselected_host_keeps_original_tls_certificate：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - rejects_upstream_and_downstream_untrusted_certificates：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - disabled_body_capture_preserves_lengths_without_payloads：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - stop_closes_active_connections_and_releases_listener：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - restores_ca_and_rejects_mismatched_or_non_ca_identity：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - rejects_cross_host_request_and_accepts_chunked_request_body：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - rejects_wildcards_and_preserves_head_and_redirect：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//!
//! 测试依据：保留原有公开接口契约及全部断言；测试文件原注释保留在下方，L3 必须独立核对。
//!
//! 前置条件：本地测试需文件/回环套接字权限；平台限定及忽略项以实际登记为准。
//!
//! 执行状态：由同名 Markdown 报告记录，不在源码中固化通过结论。

// 真实双向 TLS 与 CONNECT 集成验证；不关闭证书检查、不修改系统信任。
#[path = "test_fixture_https.rs"]
mod support;
use fluxa_core::{
    error::Error,
    http::{self, Header},
    https::HttpsClient,
    mitm::{MitmProxy, authority::Authority, capture::CaptureStore},
};
use rustls::{
    ClientConfig, RootCertStore,
    pki_types::{CertificateDer, ServerName, pem::PemObject},
};
use std::{sync::Arc, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt, BufReader},
    net::TcpStream,
    time::{sleep, timeout},
};
use tokio_rustls::TlsConnector;
fn client(certificate: CertificateDer<'static>, fixture: &support::Fixture) -> HttpsClient {
    let mut roots = RootCertStore::empty();
    roots.add(certificate).unwrap();
    HttpsClient::with_roots(roots)
        .unwrap()
        .with_network(Arc::clone(&fixture.network))
}
fn ca_client(authority: &Authority, fixture: &support::Fixture) -> HttpsClient {
    client(
        CertificateDer::from(authority.certificate_der().to_vec()),
        fixture,
    )
}
async fn captured(captures: &CaptureStore) -> fluxa_core::mitm::capture::Capture {
    timeout(Duration::from_secs(2), async {
        loop {
            if let Some(capture) = captures.peek().unwrap() {
                break capture;
            }
            sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap()
}
async fn raw_client(
    port: u16,
    target_port: u16,
    authority: &Authority,
) -> tokio_rustls::client::TlsStream<BufReader<TcpStream>> {
    let mut socket = BufReader::new(TcpStream::connect(("127.0.0.1", port)).await.unwrap());
    socket
        .get_mut()
        .write_all(
            format!(
                "CONNECT localhost:{target_port} HTTP/1.1\r\nHost: localhost:{target_port}\r\n\r\n"
            )
            .as_bytes(),
        )
        .await
        .unwrap();
    let head = http::read_head(&mut socket).await.unwrap();
    assert!(head.first.starts_with("HTTP/1.1 200"));
    let mut roots = RootCertStore::empty();
    roots
        .add(CertificateDer::from(authority.certificate_der().to_vec()))
        .unwrap();
    let config =
        ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
            .with_safe_default_protocol_versions()
            .unwrap()
            .with_root_certificates(roots)
            .with_no_client_auth();
    TlsConnector::from(Arc::new(config))
        .connect(
            ServerName::try_from("localhost").unwrap().to_owned(),
            socket,
        )
        .await
        .unwrap()
}
#[tokio::test]
async fn intercepts_real_tls_and_captures_both_bodies() {
    let fixture = support::fixture().await;
    let authority = Arc::new(Authority::generate().unwrap());
    let captures = Arc::new(CaptureStore::default());
    let proxy = MitmProxy::start(
        0,
        &["LOCALHOST.".into()],
        Arc::clone(&authority),
        client(
            CertificateDer::from_pem_slice(fixture.pem.as_bytes()).unwrap(),
            &fixture,
        ),
        Arc::clone(&captures),
        true,
    )
    .await
    .unwrap();
    let response = ca_client(&authority, &fixture)
        .execute_via_proxy(
            "POST",
            &format!("https://localhost:{}/echo", fixture.port),
            &[Header {
                name: "X-Test".into(),
                value: "mitm".into(),
            }],
            b"decrypted request and response",
            proxy.port(),
        )
        .await
        .unwrap();
    assert_eq!(response.body, b"decrypted request and response");
    let record = captured(&captures).await;
    assert!(record.intercepted);
    assert_eq!(record.status, Some(200));
    assert_eq!(record.method, "POST");
    assert_eq!(record.request_body, response.body);
    assert_eq!(record.response_body, response.body);
    assert!(record.error.is_none());
    assert!(captures.acknowledge(record.id + 1).is_err());
    assert_eq!(captures.peek().unwrap().unwrap().id, record.id);
    captures.acknowledge(record.id).unwrap();
    assert!(captures.peek().unwrap().is_none());
    proxy.stop().await.unwrap();
}
#[tokio::test]
async fn unselected_host_keeps_original_tls_certificate() {
    let fixture = support::fixture().await;
    let captures = Arc::new(CaptureStore::default());
    let proxy = MitmProxy::start(
        0,
        &["sub.localhost".into()],
        Arc::new(Authority::generate().unwrap()),
        client(fixture.root.clone(), &fixture),
        Arc::clone(&captures),
        true,
    )
    .await
    .unwrap();
    let response = client(fixture.root.clone(), &fixture)
        .execute_via_proxy(
            "GET",
            &format!("https://localhost:{}/", fixture.port),
            &[],
            &[],
            proxy.port(),
        )
        .await
        .unwrap();
    assert_eq!(response.status, 200);
    assert!(!captured(&captures).await.intercepted);
    proxy.stop().await.unwrap();
}
#[tokio::test]
async fn rejects_upstream_and_downstream_untrusted_certificates() {
    let fixture = support::fixture().await;
    let authority = Arc::new(Authority::generate().unwrap());
    let captures = Arc::new(CaptureStore::default());
    let proxy = MitmProxy::start(
        0,
        &["localhost".into()],
        Arc::clone(&authority),
        HttpsClient::with_roots(RootCertStore::empty())
            .unwrap()
            .with_network(Arc::clone(&fixture.network)),
        Arc::clone(&captures),
        true,
    )
    .await
    .unwrap();
    assert!(
        ca_client(&authority, &fixture)
            .execute_via_proxy(
                "GET",
                &format!("https://localhost:{}/", fixture.port),
                &[],
                &[],
                proxy.port()
            )
            .await
            .is_err()
    );
    assert!(captured(&captures).await.error.is_some());
    proxy.stop().await.unwrap();
    let proxy = MitmProxy::start(
        0,
        &["localhost".into()],
        authority,
        client(fixture.root.clone(), &fixture),
        Arc::new(CaptureStore::default()),
        true,
    )
    .await
    .unwrap();
    assert!(
        client(fixture.root.clone(), &fixture)
            .execute_via_proxy(
                "GET",
                &format!("https://localhost:{}/", fixture.port),
                &[],
                &[],
                proxy.port()
            )
            .await
            .is_err()
    );
    proxy.stop().await.unwrap();
}
#[tokio::test]
async fn disabled_body_capture_preserves_lengths_without_payloads() {
    let fixture = support::fixture().await;
    let authority = Arc::new(Authority::generate().unwrap());
    let captures = Arc::new(CaptureStore::default());
    let proxy = MitmProxy::start(
        0,
        &["localhost".into()],
        Arc::clone(&authority),
        client(fixture.root.clone(), &fixture),
        Arc::clone(&captures),
        false,
    )
    .await
    .unwrap();
    ca_client(&authority, &fixture)
        .execute_via_proxy(
            "POST",
            &format!("https://localhost:{}/", fixture.port),
            &[],
            b"secret",
            proxy.port(),
        )
        .await
        .unwrap();
    let record = captured(&captures).await;
    assert!(record.request_body.is_empty() && record.response_body.is_empty());
    assert_eq!(record.request_body_length, 6);
    assert_eq!(record.response_body_length, 6);
    proxy.stop().await.unwrap();
}
#[tokio::test]
async fn stop_closes_active_connections_and_releases_listener() {
    let fixture = support::fixture().await;
    let proxy = MitmProxy::start(
        0,
        &["localhost".into()],
        Arc::new(Authority::generate().unwrap()),
        client(fixture.root.clone(), &fixture),
        Arc::new(CaptureStore::default()),
        false,
    )
    .await
    .unwrap();
    let port = proxy.port();
    let mut socket = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
    sleep(Duration::from_millis(10)).await;
    proxy.stop().await.unwrap();
    let mut byte = [0];
    let result = timeout(Duration::from_secs(1), socket.read(&mut byte))
        .await
        .unwrap();
    assert!(matches!(result, Ok(0) | Err(_)));
    assert!(TcpStream::connect(("127.0.0.1", port)).await.is_err());
}
#[test]
fn restores_ca_and_rejects_mismatched_or_non_ca_identity() {
    let first = Authority::generate().unwrap();
    let other = Authority::generate().unwrap();
    let restored = Authority::restore(first.certificate_pem(), &first.private_key_pem()).unwrap();
    assert_eq!(restored.certificate_der(), first.certificate_der());
    assert!(Authority::restore(first.certificate_pem(), &other.private_key_pem()).is_err());
    let leaf = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
    assert!(Authority::restore(&leaf.cert.pem(), &leaf.signing_key.serialize_pem()).is_err());
    assert!(CertificateDer::from_pem_slice(first.certificate_pem().as_bytes()).is_ok());
}
#[tokio::test]
async fn rejects_cross_host_request_and_accepts_chunked_request_body() {
    let fixture = support::fixture().await;
    let authority = Arc::new(Authority::generate().unwrap());
    let captures = Arc::new(CaptureStore::default());
    let proxy = MitmProxy::start(
        0,
        &["localhost".into()],
        Arc::clone(&authority),
        client(fixture.root.clone(), &fixture),
        Arc::clone(&captures),
        true,
    )
    .await
    .unwrap();
    let mut tls = raw_client(proxy.port(), fixture.port, &authority).await;
    tls.write_all(b"GET / HTTP/1.1\r\nHost: attacker.example\r\n\r\n")
        .await
        .unwrap();
    let mut byte = [0];
    let _ = timeout(Duration::from_secs(1), tls.read(&mut byte))
        .await
        .unwrap();
    let record = captured(&captures).await;
    assert!(record.error.is_some());
    captures.acknowledge(record.id).unwrap();
    let tls = raw_client(proxy.port(), fixture.port, &authority).await;
    let mut tls = BufReader::new(tls);
    tls.get_mut().write_all(format!("POST / HTTP/1.1\r\nHost: localhost:{}\r\nTransfer-Encoding: chunked\r\n\r\n4\r\ntest\r\n0\r\n\r\n", fixture.port).as_bytes()).await.unwrap();
    let head = http::read_head(&mut tls).await.unwrap();
    let body = http::read_body(&mut tls, &head, false).await.unwrap();
    assert_eq!(body, b"test");
    proxy.stop().await.unwrap();
}
#[tokio::test]
async fn rejects_wildcards_and_preserves_head_and_redirect() {
    let fixture = support::fixture().await;
    let authority = Arc::new(Authority::generate().unwrap());
    assert!(matches!(
        MitmProxy::start(
            0,
            &["*.localhost".into()],
            Arc::clone(&authority),
            client(fixture.root.clone(), &fixture),
            Arc::new(CaptureStore::default()),
            false
        )
        .await,
        Err(Error::InvalidInput)
    ));
    let proxy = MitmProxy::start(
        0,
        &["localhost".into()],
        Arc::clone(&authority),
        client(fixture.root.clone(), &fixture),
        Arc::new(CaptureStore::default()),
        true,
    )
    .await
    .unwrap();
    let client = ca_client(&authority, &fixture);
    let head = client
        .execute_via_proxy(
            "HEAD",
            &format!("https://localhost:{}/head", fixture.port),
            &[],
            &[],
            proxy.port(),
        )
        .await
        .unwrap();
    assert!(head.body.is_empty());
    assert!(
        head.headers
            .iter()
            .any(|h| h.name.eq_ignore_ascii_case("content-length") && h.value == "123")
    );
    let redirect = client
        .execute_via_proxy(
            "GET",
            &format!("https://localhost:{}/redirect", fixture.port),
            &[],
            &[],
            proxy.port(),
        )
        .await
        .unwrap();
    assert_eq!(redirect.status, 302);
    proxy.stop().await.unwrap();
}
