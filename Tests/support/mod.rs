//! 真实回环 TLS 服务供 Rust 和 Swift FFI 测试共用；私钥只存在于进程内存。
pub mod dns;
use libfluxa_core::http;
use rcgen::{BasicConstraints, CertificateParams, CertifiedIssuer, IsCa, KeyPair, KeyUsagePurpose};
use rustls::{ServerConfig, pki_types::PrivatePkcs8KeyDer};
use std::sync::Arc;
use tokio::{
    io::{AsyncRead, AsyncWrite, AsyncWriteExt, BufReader},
    net::TcpListener,
    task::{JoinHandle, JoinSet},
};
use tokio_rustls::TlsAcceptor;

// 同一测试夹具由独立协议目标与二进制夹具程序引用，各目标使用所需字段。
#[allow(dead_code)]
pub struct Fixture {
    pub network: Arc<libfluxa_core::outbound::Manager>,
    _dns: dns::Fixture,
    pub port: u16,
    pub root: rustls::pki_types::CertificateDer<'static>,
    pub pem: String,
    pub task: JoinHandle<()>,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.task.abort();
    }
}
pub async fn fixture() -> Fixture {
    fixture_with_options(false, false).await
}
pub async fn fixture_with_options(expired: bool, tls12: bool) -> Fixture {
    fixture_with_provider(expired, tls12, rustls::crypto::ring::default_provider()).await
}
/// 协议互通逐个强制 TLS 1.3 套件，避免只验证默认 AES-128。
// 此函数仅由协议夹具程序调用；共用测试模块的其他目标不使用该入口。
#[allow(dead_code)]
pub async fn fixture_with_suite(suite: u16) -> Fixture {
    let mut provider = rustls::crypto::ring::default_provider();
    provider
        .cipher_suites
        .retain(|v| u16::from(v.suite()) == suite);
    assert_eq!(provider.cipher_suites.len(), 1);
    fixture_with_provider(false, false, provider).await
}
async fn fixture_with_provider(
    expired: bool,
    tls12: bool,
    provider: rustls::crypto::CryptoProvider,
) -> Fixture {
    let mut params = CertificateParams::new(Vec::<String>::new()).unwrap();
    params.is_ca = IsCa::Ca(BasicConstraints::Constrained(0));
    params.key_usages = vec![KeyUsagePurpose::KeyCertSign];
    let ca = CertifiedIssuer::self_signed(params, KeyPair::generate().unwrap()).unwrap();
    let key = KeyPair::generate().unwrap();
    let mut params = CertificateParams::new(vec!["localhost".to_owned()]).unwrap();
    if expired {
        params.not_before = time::OffsetDateTime::now_utc() - time::Duration::days(2);
        params.not_after = time::OffsetDateTime::now_utc() - time::Duration::days(1);
    }
    let leaf = params.signed_by(&key, &ca).unwrap();
    let tls12_versions = [&rustls::version::TLS12];
    let mut config = ServerConfig::builder_with_provider(Arc::new(provider))
        .with_protocol_versions(if tls12 {
            &tls12_versions
        } else {
            rustls::DEFAULT_VERSIONS
        })
        .unwrap()
        .with_no_client_auth()
        .with_single_cert(
            vec![leaf.der().clone()],
            PrivatePkcs8KeyDer::from(key.serialize_der()).into(),
        )
        .unwrap();
    config.alpn_protocols = vec![b"http/1.1".to_vec()];
    let acceptor = TlsAcceptor::from(Arc::new(config));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let task = tokio::spawn(async move {
        let mut sessions = JoinSet::new();
        loop {
            tokio::select! {
                accepted = listener.accept() => {
                    let Ok((socket, _)) = accepted else { break; };
                    let acceptor = acceptor.clone();
                    sessions.spawn(async move {
                        let Ok(tls) = acceptor.accept(socket).await else { return; };
                        respond(tls).await;
                    });
                }
                _ = sessions.join_next(), if !sessions.is_empty() => {}
            }
        }
    });
    let (_dns, network) = configured_network().await;
    Fixture {
        network,
        _dns,
        port,
        root: ca.der().clone(),
        pem: ca.pem(),
        task,
    }
}

async fn respond<S: AsyncRead + AsyncWrite + Unpin>(stream: S) {
    let mut tls = BufReader::new(stream);
    let Ok(head) = http::read_head(&mut tls).await else {
        return;
    };
    let Ok(body) = http::read_body(&mut tls, &head, false).await else {
        return;
    };
    let parts = head.parts();
    let path = parts.get(1).copied().unwrap_or("/");
    if path == "/wait" {
        tokio::time::sleep(std::time::Duration::from_secs(120)).await;
        return;
    }
    let response: Vec<u8> = match path {
                            "/subscription" => { let payload = b"schema-version: 1\nkind: rules\nname: Fixture\nrules: []\n"; let mut response = format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n", payload.len()).into_bytes(); response.extend(payload); response },
                            "/subscription-redirect" => b"HTTP/1.1 302 Found\r\nLocation: /subscription\r\nContent-Length: 0\r\n\r\n".to_vec(),
                            "/subscription-downgrade" => b"HTTP/1.1 302 Found\r\nLocation: http://localhost/\r\nContent-Length: 0\r\n\r\n".to_vec(),
                            "/subscription-loop" => b"HTTP/1.1 302 Found\r\nLocation: /subscription-loop\r\nContent-Length: 0\r\n\r\n".to_vec(),
                            "/subscription-limit" => b"HTTP/1.1 200 OK\r\nContent-Length: 2097153\r\n\r\n".to_vec(),
                            "/subscription-invalid" => b"HTTP/1.1 200 OK\r\nContent-Length: 1\r\n\r\n\xff".to_vec(),
                            "/subscription-error" => b"HTTP/1.1 500 Error\r\nContent-Length: 0\r\n\r\n".to_vec(),
                            "/headers" => { let payload = serde_json::to_vec(&head.headers).unwrap(); let mut response = format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n", payload.len()).into_bytes(); response.extend(payload); response },
                            "/json" => { let payload = br#"{"ad":{"url":"https://example.com/ad"},"title":"keep","id":18446744073709551615,"nested":{"ad":"keep"}}"#; let mut response = format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nETag: old\r\nContent-Length: {}\r\n\r\n", payload.len()).into_bytes(); response.extend(payload); response },
                            "/chunked" => b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n5\r\nhello\r\n6\r\n world\r\n0\r\n\r\n".to_vec(),
                            "/ambiguous" => b"HTTP/1.1 200 OK\r\nContent-Length: 1\r\nTransfer-Encoding: chunked\r\n\r\n0\r\n\r\n".to_vec(),
                            "/oversized" => b"HTTP/1.1 200 OK\r\nContent-Length: 8388609\r\n\r\n".to_vec(),
                            "/redirect" => b"HTTP/1.1 302 Found\r\nLocation: https://example.com/\r\nContent-Length: 0\r\n\r\n".to_vec(),
                            "/head" => b"HTTP/1.1 200 OK\r\nContent-Length: 123\r\n\r\n".to_vec(),
                            "/no-content" => b"HTTP/1.1 204 No Content\r\n\r\n".to_vec(),
                            "/truncated" => b"HTTP/1.1 200 OK\r\nContent-Length: 10\r\n\r\nshort".to_vec(),
                            _ => { let payload = if body.is_empty() { b"decrypted response".to_vec() } else { body }; let mut response = format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: text/plain\r\nX-Fixture: rust-tls\r\nConnection: close\r\n\r\n", payload.len()).into_bytes(); response.extend(payload); response }
                        };
    let _ = tls.get_mut().write_all(&response).await;
    let _ = tls.get_mut().shutdown().await;
}

// 同一支持模块被多个独立测试目标引用；HTTPS 专用目标不需要明文服务。
#[allow(dead_code)]
pub struct PlainFixture {
    pub network: Arc<libfluxa_core::outbound::Manager>,
    _dns: dns::Fixture,
    pub port: u16,
    task: JoinHandle<()>,
}
impl Drop for PlainFixture {
    fn drop(&mut self) {
        self.task.abort();
    }
}
#[allow(dead_code)]
pub async fn plain_fixture() -> PlainFixture {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let task = tokio::spawn(async move {
        let mut sessions = JoinSet::new();
        loop {
            tokio::select! {
                accepted = listener.accept() => {
                    let Ok((socket, _)) = accepted else { break; };
                    sessions.spawn(respond(socket));
                }
                _ = sessions.join_next(), if !sessions.is_empty() => {}
            }
        }
    });
    let (_dns, network) = configured_network().await;
    PlainFixture {
        port,
        task,
        network,
        _dns,
    }
}

async fn configured_network() -> (dns::Fixture, Arc<libfluxa_core::outbound::Manager>) {
    use libfluxa_core::{
        dns::{Config, Resolver, Server},
        outbound::{Limits, Manager},
        routing::{Config as Routing, Mode, Router},
    };
    let dns = dns::fixture().await;
    let resolver = Arc::new(
        Resolver::new(Config {
            servers: vec![Server::Udp {
                address: format!("127.0.0.1:{}", dns.port),
            }],
            timeout_ms: 1000,
            dnssec: false,
            fake_ip: None,
            service: None,
        })
        .unwrap(),
    );
    let manager = Arc::new(Manager::default());
    manager
        .configure_network(
            Some(resolver),
            Some(Arc::new(
                Router::compile(Routing {
                    mode: Mode::Direct,
                    ..Routing::default()
                })
                .unwrap(),
            )),
            Limits::default(),
        )
        .unwrap();
    (dns, manager)
}
