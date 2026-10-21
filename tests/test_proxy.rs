//! 测试文件：tests/test_proxy.rs
//! 测试类型：集成测试、回归测试
//! 测试对象：跨模块契约：proxy
//! 测试内容：
//! - http_rewrites_chunked_upload_and_records_original_request：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - rejection_is_exact_and_upstream_failure_never_falls_back：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - https_uses_same_rewrite_and_capture_pipeline：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - invalid_rules_and_unimplemented_lan_are_rejected_before_listening：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - replacement_expansion_is_bounded_before_allocation：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - traffic_script_runs_on_http_and_https_and_captures_processed_body：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - traffic_scripts_validate_configuration_and_bound_processing：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//!
//! 测试依据：保留原有公开接口契约及全部断言；测试文件原注释保留在下方，L3 必须独立核对。
//!
//! 前置条件：本地测试需文件/回环套接字权限；平台限定及忽略项以实际登记为准。
//!
//! 执行状态：由同名 Markdown 报告记录，不在源码中固化通过结论。

// 使用真实 HTTP/TLS 服务验证迁移后的公共转发流程、改写、日志与拒绝边界。
#[path = "test_fixture_https.rs"]
mod support;
use fluxa_core::{
    error::Error,
    http,
    https::HttpsClient,
    mitm::{MitmProxy, ProxyOptions, authority::Authority, capture::CaptureStore},
    rewrite::Rewrite,
};
use rustls::{
    RootCertStore,
    pki_types::{CertificateDer, pem::PemObject},
};
use std::{sync::Arc, time::Duration};
use tokio::{
    io::{AsyncWriteExt, BufReader},
    net::TcpStream,
    time::{sleep, timeout},
};
fn rules(value: serde_json::Value) -> Vec<Rewrite> {
    serde_json::from_value(value).unwrap()
}
async fn proxy(
    options: ProxyOptions,
    roots: RootCertStore,
    captures: Arc<CaptureStore>,
    ca: Arc<Authority>,
    network: Arc<fluxa_core::outbound::Manager>,
) -> MitmProxy {
    MitmProxy::start_with_options(
        options,
        ca,
        HttpsClient::with_roots(roots)
            .unwrap()
            .with_network(network),
        captures,
    )
    .await
    .unwrap()
}
async fn request(
    port: u16,
    target: u16,
    path: &str,
    extra: &str,
    body: &str,
) -> (http::Head, Vec<u8>) {
    let mut stream = BufReader::new(TcpStream::connect(("127.0.0.1", port)).await.unwrap());
    stream.get_mut().write_all(format!("POST http://localhost:{target}{path} HTTP/1.1\r\nHost: localhost:{target}\r\n{extra}\r\n{body}").as_bytes()).await.unwrap();
    let head = http::read_head(&mut stream).await.unwrap();
    let body = http::read_body(&mut stream, &head, false).await.unwrap();
    (head, body)
}
#[tokio::test]
async fn http_rewrites_chunked_upload_and_records_original_request() {
    let fixture = support::plain_fixture().await;
    let ca = Arc::new(Authority::generate().unwrap());
    let captures = Arc::new(CaptureStore::default());
    let server = proxy(ProxyOptions { capture_bodies: true, rewrites: rules(serde_json::json!([
        {"host":"LOCALHOST","path_prefix":"/echo","action":"replace-response-text","match":"test","value":"changed"},
        {"host":"localhost","path_prefix":"/echo","action":"response-header","header":"X-Rewritten","value":"yes"},
        {"host":"localhost","path_prefix":"/headers","action":"request-header","header":"X-Added","value":"native"}
    ])), ..Default::default() }, RootCertStore::empty(), Arc::clone(&captures), ca, Arc::clone(&fixture.network)).await;
    let (head, body) = request(
        server.port(),
        fixture.port,
        "/echo",
        "Transfer-Encoding: chunked\r\n",
        "4\r\ntest\r\n0\r\n\r\n",
    )
    .await;
    assert_eq!(body, b"changed");
    assert!(
        head.headers
            .iter()
            .any(|h| h.name.eq_ignore_ascii_case("content-length") && h.value == "7")
    );
    assert!(
        head.headers
            .iter()
            .any(|h| h.name == "X-Rewritten" && h.value == "yes")
    );
    timeout(Duration::from_secs(2), async {
        while captures.peek().unwrap().is_none() {
            sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    let capture = captures.peek().unwrap().unwrap();
    assert_eq!(capture.request_body, b"test");
    assert_eq!(capture.response_body, b"changed");
    let (_, body) = request(server.port(), fixture.port, "/headers", "Connection: X-Hop\r\nX-Hop: secret\r\nProxy-Authorization: secret\r\nContent-Length: 0\r\n", "").await;
    let headers: Vec<http::Header> = serde_json::from_slice(&body).unwrap();
    assert!(
        headers
            .iter()
            .any(|h| h.name == "X-Added" && h.value == "native")
    );
    assert!(!headers.iter().any(|h| h.name.eq_ignore_ascii_case("x-hop")
        || h.name.eq_ignore_ascii_case("proxy-authorization")));
    server.stop().await.unwrap();
}
#[tokio::test]
async fn rejection_is_exact_and_upstream_failure_never_falls_back() {
    let fixture = support::plain_fixture().await;
    let ca = Arc::new(Authority::generate().unwrap());
    let server = proxy(
        ProxyOptions {
            rewrites: rules(serde_json::json!([
                {"host":"localhost","path_prefix":"/blocked","action":"reject"},
                {"host":"sub.localhost","path_prefix":"/","action":"reject"}
            ])),
            ..Default::default()
        },
        RootCertStore::empty(),
        Arc::new(CaptureStore::default()),
        Arc::clone(&ca),
        Arc::clone(&fixture.network),
    )
    .await;
    let (head, body) = request(
        server.port(),
        fixture.port,
        "/blocked",
        "Content-Length: 0\r\n",
        "",
    )
    .await;
    assert!(head.first.starts_with("HTTP/1.1 400"));
    assert!(String::from_utf8(body).unwrap().contains("Fluxa"));
    assert!(
        request(
            server.port(),
            fixture.port,
            "/echo",
            "Content-Length: 0\r\n",
            ""
        )
        .await
        .0
        .first
        .starts_with("HTTP/1.1 200")
    );
    server.stop().await.unwrap();
    let reservation = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let closed = reservation.local_addr().unwrap().port();
    drop(reservation);
    let server = proxy(
        ProxyOptions {
            upstream_proxy_port: Some(closed),
            ..Default::default()
        },
        RootCertStore::empty(),
        Arc::new(CaptureStore::default()),
        ca,
        Arc::clone(&fixture.network),
    )
    .await;
    assert!(
        request(
            server.port(),
            fixture.port,
            "/",
            "Content-Length: 0\r\n",
            ""
        )
        .await
        .0
        .first
        .starts_with("HTTP/1.1 502")
    );
    server.stop().await.unwrap();
}
#[tokio::test]
async fn https_uses_same_rewrite_and_capture_pipeline() {
    let fixture = support::fixture().await;
    assert_eq!(
        CertificateDer::from_pem_slice(fixture.pem.as_bytes()).unwrap(),
        fixture.root
    );
    let ca = Arc::new(Authority::generate().unwrap());
    let mut roots = RootCertStore::empty();
    roots
        .add(CertificateDer::from_pem_slice(fixture.pem.as_bytes()).unwrap())
        .unwrap();
    let server = proxy(ProxyOptions { hosts: vec!["localhost".into()], rewrites: rules(serde_json::json!([
        {"host":"localhost","path_prefix":"/echo","action":"replace-response-text","match":"decrypted","value":"rewritten"}
    ])), ..Default::default() }, roots, Arc::new(CaptureStore::default()), Arc::clone(&ca), Arc::clone(&fixture.network)).await;
    let mut roots = RootCertStore::empty();
    roots
        .add(CertificateDer::from(ca.certificate_der().to_vec()))
        .unwrap();
    let response = HttpsClient::with_roots(roots)
        .unwrap()
        .with_network(Arc::clone(&fixture.network))
        .execute_via_proxy(
            "GET",
            &format!("https://localhost:{}/echo", fixture.port),
            &[],
            &[],
            server.port(),
        )
        .await
        .unwrap();
    assert_eq!(response.body, b"rewritten response");
    server.stop().await.unwrap();
}
#[tokio::test]
async fn invalid_rules_and_unimplemented_lan_are_rejected_before_listening() {
    let ca = Arc::new(Authority::generate().unwrap());
    for options in [
        ProxyOptions {
            allow_lan: true,
            ..Default::default()
        },
        ProxyOptions {
            rewrites: rules(serde_json::json!([
                {"host":"localhost","path_prefix":"/","action":"request-header","header":"Host","value":"attacker"}
            ])),
            ..Default::default()
        },
    ] {
        assert!(
            MitmProxy::start_with_options(
                options,
                Arc::clone(&ca),
                HttpsClient::with_roots(RootCertStore::empty()).unwrap(),
                Arc::new(CaptureStore::default())
            )
            .await
            .is_err()
        );
    }
}
#[test]
fn replacement_expansion_is_bounded_before_allocation() {
    let rules = rules(
        serde_json::json!([{"host":"localhost","path_prefix":"/","action":"replace-response-text","match":"a","value":"x".repeat(8192)}]),
    );
    let mut response = fluxa_core::https::Response {
        status: 200,
        headers: vec![http::Header {
            name: "Content-Type".into(),
            value: "text/plain".into(),
        }],
        body: vec![b'a'; 1024 * 1024],
        duration_ms: 0,
    };
    assert_eq!(
        fluxa_core::rewrite::response("http://localhost/", &mut response, &rules),
        Err(Error::ResourceLimit)
    );
    assert_eq!(response.body.len(), 1024 * 1024);
}

#[tokio::test]
async fn traffic_script_runs_on_http_and_https_and_captures_processed_body() {
    let tls = support::fixture().await;
    let plain = support::plain_fixture().await;
    let ca = Arc::new(Authority::generate().unwrap());
    let captures = Arc::new(CaptureStore::default());
    let mut roots = RootCertStore::empty();
    roots.add(tls.root.clone()).unwrap();
    let server = proxy(ProxyOptions {
        hosts: vec!["localhost".into()], capture_bodies: true,
        rewrites: rules(serde_json::json!([
            {"host":"localhost","path_prefix":"/json","action":"response-script","script":"remove-json-keys","json_keys":["ad"]},
            {"host":"localhost","path_prefix":"/json","action":"response-header","header":"X-Processed","value":"yes"}
        ])), ..Default::default()
    }, roots, Arc::clone(&captures), Arc::clone(&ca), Arc::clone(&tls.network)).await;
    let mut roots = RootCertStore::empty();
    roots
        .add(CertificateDer::from(ca.certificate_der().to_vec()))
        .unwrap();
    let client = HttpsClient::with_roots(roots)
        .unwrap()
        .with_network(Arc::clone(&tls.network));
    for url in [
        format!("http://localhost:{}/json", plain.port),
        format!("https://localhost:{}/json", tls.port),
    ] {
        let response = client
            .execute_request("GET", &url, &[], &[], Some(server.port()))
            .await
            .unwrap();
        let value: serde_json::Value = serde_json::from_slice(&response.body).unwrap();
        assert!(value.get("ad").is_none());
        assert_eq!(value["title"], "keep");
        assert_eq!(value["nested"]["ad"], "keep");
        assert_eq!(value["id"].as_u64(), Some(u64::MAX));
        assert!(
            !response
                .headers
                .iter()
                .any(|h| h.name.eq_ignore_ascii_case("etag"))
        );
        assert!(
            response
                .headers
                .iter()
                .any(|h| h.name == "X-Processed" && h.value == "yes")
        );
        timeout(Duration::from_secs(2), async {
            while captures.peek().unwrap().is_none() {
                sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap();
        let record = captures.peek().unwrap().unwrap();
        assert_eq!(record.response_body, response.body);
        assert!(record.error.is_none());
        captures.acknowledge(record.id).unwrap();
    }
    server.stop().await.unwrap();
}

#[test]
fn traffic_scripts_validate_configuration_and_bound_processing() {
    use fluxa_core::{
        http::Header,
        https::Response,
        rewrite,
        traffic_script::{MAX_BODY, Script},
    };
    let keys = vec!["ad".to_owned()];
    assert!(Script::RemoveJsonKeys.validate(&[]).is_err());
    assert!(
        Script::RemoveJsonKeys
            .validate(&["ad".into(), "ad".into()])
            .is_err()
    );
    assert_eq!(
        Script::RemoveJsonKeys.execute(&vec![b' '; MAX_BODY + 1], &keys),
        Err(Error::ResourceLimit)
    );
    assert_eq!(
        Script::RemoveJsonKeys.execute(b"[]", &keys),
        Err(Error::Unsupported)
    );
    assert!(
        Script::RemoveJsonKeys
            .execute(b"invalid JSON", &keys)
            .is_err()
    );
    assert!(serde_json::from_value::<Rewrite>(serde_json::json!({"host":"localhost","path_prefix":"/","action":"response-script","script":"https://example.com/script.js"})).is_err());
    let rule = rules(
        serde_json::json!([{"host":"localhost","path_prefix":"/","action":"response-script","script":"remove-json-keys","json_keys":["ad"]}]),
    );
    rule[0].validate().unwrap();
    for (mime, encoding) in [("text/plain", "identity"), ("application/json", "gzip")] {
        let original = br#"{"ad":1,"keep":2}"#.to_vec();
        let mut response = Response {
            status: 200,
            headers: vec![
                Header {
                    name: "Content-Type".into(),
                    value: mime.into(),
                },
                Header {
                    name: "Content-Encoding".into(),
                    value: encoding.into(),
                },
            ],
            body: original.clone(),
            duration_ms: 0,
        };
        assert_eq!(
            rewrite::response("http://localhost/", &mut response, &rule),
            Err(Error::Unsupported)
        );
        assert_eq!(response.body, original);
    }
    let wrong_stage = rules(
        serde_json::json!([{"host":"localhost","path_prefix":"/","action":"request-header","header":"X-Test","value":"x","script":"remove-json-keys","json_keys":["ad"]}]),
    );
    assert!(wrong_stage[0].validate().is_err());
}
