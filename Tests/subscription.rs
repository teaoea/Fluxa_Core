//! 验证订阅实际 HTTPS 下载、重定向、安全边界和共用有界分帧。
mod support;
use libfluxa_core::{error::Error, https::HttpsClient, subscription};
#[tokio::test]
async fn fetches_https_and_rejects_invalid_content() {
    let fixture = support::fixture().await;
    let mut roots = rustls::RootCertStore::empty();
    roots.add(fixture.root.clone()).unwrap();
    let client = HttpsClient::with_roots(roots)
        .unwrap()
        .with_network(fixture.network.clone());
    let url = format!("https://localhost:{}", fixture.port);
    for path in ["/subscription", "/subscription-redirect"] {
        let response = subscription::fetch(&client, &format!("{url}{path}"))
            .await
            .unwrap();
        assert_eq!(
            response.body,
            b"schema-version: 1\nkind: rules\nname: Fixture\nrules: []\n"
        );
    }
    for path in [
        "/subscription-downgrade",
        "/subscription-loop",
        "/subscription-limit",
        "/subscription-invalid",
        "/subscription-error",
    ] {
        assert!(
            subscription::fetch(&client, &format!("{url}{path}"))
                .await
                .is_err()
        );
    }
    for address in [
        "http://localhost/",
        "https://user:secret@localhost/",
        "https://localhost/#fragment",
    ] {
        assert!(subscription::fetch(&client, address).await.is_err());
    }
    assert_eq!(
        client
            .execute_limited(&format!("{url}/chunked"), 5)
            .await
            .err(),
        Some(Error::ResourceLimit)
    );
}
#[tokio::test]
async fn body_limits_precede_allocation_for_all_framings() {
    use libfluxa_core::http::{Head, Header, read_body_limited};
    use tokio::io::BufReader;
    let head = |name: &str, value: &str| Head {
        first: "HTTP/1.1 200 OK".into(),
        headers: vec![Header {
            name: name.into(),
            value: value.into(),
        }],
    };
    let fixed = head("Content-Length", "2097153");
    let mut reader = BufReader::new(&b""[..]);
    assert_eq!(
        read_body_limited(&mut reader, &fixed, true, 2 * 1024 * 1024)
            .await
            .err(),
        Some(Error::ResourceLimit)
    );
    let chunked = head("Transfer-Encoding", "chunked");
    let mut reader = BufReader::new(&b"200001\r\n"[..]);
    assert_eq!(
        read_body_limited(&mut reader, &chunked, true, 2 * 1024 * 1024)
            .await
            .err(),
        Some(Error::ResourceLimit)
    );
    let eof = Head {
        first: fixed.first,
        headers: vec![],
    };
    let mut reader = BufReader::new(&b"abcdef"[..]);
    assert_eq!(
        read_body_limited(&mut reader, &eof, true, 5).await.err(),
        Some(Error::ResourceLimit)
    );
}
