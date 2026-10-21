//! 测试文件：tests/test_protocol.rs
//! 测试类型：集成测试、回归测试
//! 测试对象：src/protocol/mod.rs
//! 测试内容：
//! - strict_configuration_and_supported_combination：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - independent_peer_data_authentication_and_tls13：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//!
//! 测试依据：保留原有公开接口契约及全部断言；测试文件原注释保留在下方，L3 必须独立核对。
//!
//! 前置条件：本地测试需文件/回环套接字权限；平台限定及忽略项以实际登记为准。
//!
//! 执行状态：由同名 Markdown 报告记录，不在源码中固化通过结论。

// 配置与实际独立协议互通测试；对端只用于测试，不链接第三方代理内核。
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use fluxa_core::{
    error::Error,
    protocol::{self, config::Profile},
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
fn profile() -> Profile {
    let public = x25519_dalek::PublicKey::from(&x25519_dalek::StaticSecret::from([7; 32]));
    Profile {
        id: "test".into(),
        uri: format!(
            "vless://78b9e2b4-2d8f-4be1-9fca-8f203dc14a90@127.0.0.1:443?security=reality&type=tcp&encryption=none&flow=xtls-rprx-vision&sni=example.com&pbk={}&sid=12345678&fp=fluxa",
            URL_SAFE_NO_PAD.encode(public.as_bytes())
        ),
    }
}
#[test]
fn strict_configuration_and_supported_combination() {
    let p = profile();
    let n = p.validate().unwrap();
    assert_eq!(n.short_id, [0x12, 0x34, 0x56, 0x78, 0, 0, 0, 0]);
    assert_eq!(n.port, 443);
    for (from, to) in [
        ("fp=fluxa", "fp=unknown"),
        ("type=tcp", "type=ws"),
        ("security=reality", "security=tls"),
        ("flow=xtls-rprx-vision", "flow=other"),
        ("sid=12345678", "sid=z0"),
        ("sni=example.com", "sni=127.0.0.1"),
        ("pbk=", "pbk=A"),
        ("@127.0.0.1:443", "@127.0.0.1:0"),
    ] {
        let mut bad = p.clone();
        bad.uri = bad.uri.replace(from, to);
        assert!(bad.validate().is_err(), "{from}");
    }
    for tail in ["&fp=fluxa", "&unknown=1"] {
        let mut bad = p.clone();
        bad.uri.push_str(tail);
        assert!(bad.validate().is_err());
    }
    assert!(serde_json::from_str::<Profile>(r#"{"id":"x","uri":"x","extra":true}"#).is_err());
    let mut bad = p;
    bad.uri = "x".repeat(4097);
    assert_eq!(bad.validate().err(), Some(Error::InvalidInput));
}
#[tokio::test]
#[ignore = "需要 check-protocol.sh 启动的固定版本独立 REALITY 服务端"]
async fn independent_peer_data_authentication_and_tls13() {
    let path = std::env::var("FLUXA_PROTOCOL_PROFILE").expect("必须提供临时测试配置");
    let fixture_path = std::env::var("FLUXA_PROTOCOL_FIXTURE").unwrap();
    let fixture: serde_json::Value =
        serde_json::from_slice(&std::fs::read(fixture_path).unwrap()).unwrap();
    let profile: Profile = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let node = profile.validate().unwrap();
    let peer = std::net::SocketAddr::new(node.host.parse().unwrap(), node.port);
    let port = fixture["outbound_tcp_port"].as_u64().unwrap() as u16;
    tokio::time::timeout(std::time::Duration::from_secs(30), async {
        let socket = tokio::net::TcpStream::connect(peer).await.unwrap();
        let stream = protocol::connect(socket, &node, "127.0.0.1", port)
            .await
            .unwrap();
        let (mut reader, mut writer) = tokio::io::split(stream);
        let data: Vec<u8> = (0..1024 * 1024).map(|n| (n % 251) as u8).collect();
        let expected = data.clone();
        let sender = async {
            for bytes in data.chunks(65536) {
                writer.write_all(bytes).await.unwrap();
            }
            writer.shutdown().await.unwrap();
        };
        let receiver = async {
            let mut received = vec![0; expected.len()];
            reader.read_exact(&mut received).await.unwrap();
            assert_eq!(received, expected);
        };
        tokio::join!(sender, receiver);
        let udp_port = fixture["outbound_udp_port"].as_u64().unwrap() as u16;
        let socket = tokio::net::TcpStream::connect(peer).await.unwrap();
        let udp = protocol::connect_udp(
            socket,
            &node,
            std::net::SocketAddr::from(([127, 0, 0, 1], udp_port)),
        )
        .await
        .unwrap();
        for payload in [
            vec![0, 255, 1],
            vec![7; 1472],
            vec![9; protocol::datagram_limit()],
        ] {
            assert_eq!(udp.send(&payload).await.unwrap(), payload.len());
            let mut response = vec![0; 8192];
            let length = udp.receive(&mut response).await.unwrap();
            assert_eq!(&response[..length], &payload);
        }
        assert_eq!(udp.send(&[]).await, Err(Error::Unsupported));
        assert_eq!(
            udp.send(&vec![0; protocol::datagram_limit() + 1]).await,
            Err(Error::ResourceLimit)
        );
        for mutation in [0, 1, 2, 3] {
            let mut bad = profile.validate().unwrap();
            match mutation {
                0 => bad.public_key[0] ^= 1,
                1 => bad.short_id[0] ^= 1,
                2 => bad.sni = "invalid.example".into(),
                _ => bad.uuid[0] ^= 1,
            }
            let socket = tokio::net::TcpStream::connect(peer).await.unwrap();
            match protocol::connect(socket, &bad, "127.0.0.1", port).await {
                Err(_) => {}
                Ok(mut stream) => {
                    assert_eq!(mutation, 3, "认证失败必须拒绝连接");
                    stream.write_all(b"must reject").await.unwrap();
                    let mut b = [0; 16];
                    assert!(stream.read(&mut b).await.is_err());
                }
            }
        }
        let socket = tokio::net::TcpStream::connect(peer).await.unwrap();
        let stream = protocol::connect(
            socket,
            &node,
            "127.0.0.1",
            fixture["port"].as_u64().unwrap() as u16,
        )
        .await
        .unwrap();
        let pem = fixture["certificate_pem"].as_str().unwrap();
        let cert = rustls::pki_types::pem::PemObject::from_pem_slice(pem.as_bytes()).unwrap();
        let mut roots = rustls::RootCertStore::empty();
        roots.add(cert).unwrap();
        let config = rustls::ClientConfig::builder_with_provider(std::sync::Arc::new(
            rustls::crypto::ring::default_provider(),
        ))
        .with_protocol_versions(&[&rustls::version::TLS13])
        .unwrap()
        .with_root_certificates(roots)
        .with_no_client_auth();
        let mut tls = tokio_rustls::TlsConnector::from(std::sync::Arc::new(config))
            .connect(
                rustls::pki_types::ServerName::try_from("localhost").unwrap(),
                stream,
            )
            .await
            .unwrap();
        tls.write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
            .await
            .unwrap();
        let mut response = Vec::new();
        tls.read_to_end(&mut response).await.unwrap();
        assert!(response.starts_with(b"HTTP/1.1 200"));
        assert_eq!(
            tls.get_ref().0.vision_direct(),
            (true, true),
            "必须真正进入双向 Vision 裸流"
        );
    })
    .await
    .expect("协议互通超时");
}
