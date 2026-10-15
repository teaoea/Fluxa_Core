//! 测试文件：tests/test_https.rs
//! 测试类型：单元测试、迁移回归
//! 测试对象：src/https.rs
//! 测试内容：
//! - decrypts_https_request_and_response：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - rejects_untrusted_ca_and_wrong_hostname：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - handles_chunked_head_no_content_and_redirect_without_following：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - rejects_ambiguous_oversized_and_truncated_messages：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - rejects_header_injection_credentials_and_wrong_scheme：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - rejects_expired_certificate：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - decrypts_tls12_as_well_as_default_tls13：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//!
//! 测试依据：保留原测试的业务/格式/授权/资源约束；《测试文件与报告统一管理原则》，独立语义审查不以实现输出反推预期。
//!
//! 前置条件：本地测试需文件/回环套接字权限；平台限定及忽略项以实际登记为准。
//!
//! 执行状态：由同名 Markdown 报告记录，不在源码中固化通过结论。

use crate::test_fixture_https as support;
// 以下保留迁移前的用例说明；历史执行状态不代表当前版本，当前结果仅见同名报告。
mod public_contract {
    use crate as libfluxa_core;
    // 真实 TLS 集成验证；不关闭证书验证，不使用外网或保存私钥。
    use super::support;
    use libfluxa_core::{error::Error, http::Header, https::HttpsClient};
    use rustls::RootCertStore;

    fn client(fixture: &support::Fixture) -> HttpsClient {
        let mut roots = RootCertStore::empty();
        roots.add(fixture.root.clone()).unwrap();
        HttpsClient::with_roots(roots)
            .unwrap()
            .with_network(std::sync::Arc::clone(&fixture.network))
    }
    #[tokio::test]
    async fn decrypts_https_request_and_response() {
        let fixture = support::fixture().await;
        let client = client(&fixture);
        let response = client
            .execute(
                "POST",
                &format!("https://localhost:{}/echo?x=1", fixture.port),
                &[Header {
                    name: "X-Test".into(),
                    value: "ffi-ready".into(),
                }],
                b"request body",
            )
            .await
            .unwrap();
        assert_eq!(response.status, 200);
        assert_eq!(response.body, b"request body");
        assert!(response.headers.iter().any(|h| h.name == "X-Fixture"));
        assert!(!fixture.pem.is_empty());
    }
    #[tokio::test]
    async fn rejects_untrusted_ca_and_wrong_hostname() {
        let fixture = support::fixture().await;
        let no_trust = HttpsClient::with_roots(RootCertStore::empty())
            .unwrap()
            .with_network(std::sync::Arc::clone(&fixture.network));
        assert!(matches!(
            no_trust
                .execute(
                    "GET",
                    &format!("https://localhost:{}/", fixture.port),
                    &[],
                    &[]
                )
                .await,
            Err(Error::Tls)
        ));
        assert!(matches!(
            client(&fixture)
                .execute(
                    "GET",
                    &format!("https://127.0.0.1:{}/", fixture.port),
                    &[],
                    &[]
                )
                .await,
            Err(Error::Tls)
        ));
    }
    #[tokio::test]
    async fn handles_chunked_head_no_content_and_redirect_without_following() {
        let fixture = support::fixture().await;
        let client = client(&fixture);
        for (method, path, status, body) in [
            ("GET", "/chunked", 200, b"hello world".as_slice()),
            ("HEAD", "/head", 200, b"".as_slice()),
            ("GET", "/no-content", 204, b"".as_slice()),
            ("GET", "/redirect", 302, b"".as_slice()),
        ] {
            let response = client
                .execute(
                    method,
                    &format!("https://localhost:{}{path}", fixture.port),
                    &[],
                    &[],
                )
                .await
                .unwrap();
            assert_eq!(response.status, status);
            assert_eq!(response.body, body);
        }
    }
    #[tokio::test]
    async fn rejects_ambiguous_oversized_and_truncated_messages() {
        let fixture = support::fixture().await;
        let client = client(&fixture);
        for path in ["/ambiguous", "/oversized", "/truncated"] {
            assert!(
                client
                    .execute(
                        "GET",
                        &format!("https://localhost:{}{path}", fixture.port),
                        &[],
                        &[]
                    )
                    .await
                    .is_err()
            );
        }
    }
    #[tokio::test]
    async fn rejects_header_injection_credentials_and_wrong_scheme() {
        let fixture = support::fixture().await;
        let client = client(&fixture);
        for url in [
            format!("http://localhost:{}/", fixture.port),
            format!("https://user:secret@localhost:{}/", fixture.port),
        ] {
            assert!(matches!(
                client.execute("GET", &url, &[], &[]).await,
                Err(Error::InvalidInput)
            ));
        }
        let url = format!("https://localhost:{}/", fixture.port);
        assert!(matches!(
            client
                .execute(
                    "GET",
                    &url,
                    &[Header {
                        name: "X-Test".into(),
                        value: "yes\r\nHost: attacker".into()
                    }],
                    &[]
                )
                .await,
            Err(Error::InvalidInput)
        ));
        assert!(matches!(
            client.execute("GET\r\n", &url, &[], &[]).await,
            Err(Error::InvalidInput)
        ));
    }

    #[tokio::test]
    async fn rejects_expired_certificate() {
        let fixture = support::fixture_with_options(true, false).await;
        assert!(matches!(
            client(&fixture)
                .execute(
                    "GET",
                    &format!("https://localhost:{}/", fixture.port),
                    &[],
                    &[]
                )
                .await,
            Err(Error::Tls)
        ));
    }
    #[tokio::test]
    async fn decrypts_tls12_as_well_as_default_tls13() {
        let fixture = support::fixture_with_options(false, true).await;
        let response = client(&fixture)
            .execute(
                "GET",
                &format!("https://localhost:{}/", fixture.port),
                &[],
                &[],
            )
            .await
            .unwrap();
        assert_eq!(response.body, b"decrypted response");
    }
}
