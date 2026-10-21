//! 测试文件：tests/test_dns_service.rs
//! 测试类型：单元测试、迁移回归
//! 测试对象：src/dns/service.rs
//! 测试内容：
//! - fixed_conflict_is_not_silently_moved_and_ephemeral_pair_agrees：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - service_protocols_cache_negative_large_truncation_tcp_and_stop：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - service_dnssec_proof_fake_ip_ad_flags_and_edns_version：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - service_acl_rate_malformed_cancel_idle_and_ipv6：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - service_config_rejects_public_clients_and_implicit_lan_listen：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - service_failed_bind_drop_and_global_quota_release：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//!
//! 测试依据：保留原测试的业务/格式/授权/资源约束；《测试文件与报告统一管理原则》，独立语义审查不以实现输出反推预期。
//!
//! 前置条件：本地测试需文件/回环套接字权限；平台限定及忽略项以实际登记为准。
//!
//! 执行状态：由同名 Markdown 报告记录，不在源码中固化通过结论。

#[cfg(test)]
mod binding_tests {
    use super::super::*;
    #[tokio::test]
    async fn fixed_conflict_is_not_silently_moved_and_ephemeral_pair_agrees() {
        let occupied = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let address = occupied.local_addr().unwrap();
        assert!(bind_listeners(address).await.is_err());
        for _ in 0..100 {
            let (tcp, udp) = bind_listeners("127.0.0.1:0".parse().unwrap())
                .await
                .unwrap();
            assert_eq!(tcp.local_addr().unwrap(), udp.local_addr().unwrap());
        }
    }
}

use crate::test_fixture_dnssec as signed;
use crate::test_fixture_https::dns as support;
// 以下保留迁移前的用例说明；历史执行状态不代表当前版本，当前结果仅见同名报告。
mod public_contract {
    // 对实际 UDP/TCP 监听端口测试，不用模拟网络替代协议和停止行为。
    use super::signed;
    use super::support;
    use fluxa_core::dns::{
        Config, Resolver, Server,
        service::{Config as ListenConfig, Service},
    };
    use hickory_resolver::proto::{
        op::{Edns, Message, Query as WireQuery, ResponseCode},
        rr::{Name, RecordType},
    };
    use std::{
        sync::{Arc, atomic::Ordering},
        time::Duration,
    };
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::{TcpStream, UdpSocket},
    };
    static SERIAL: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
    fn config(port: u16) -> Config {
        Config {
            servers: vec![Server::Udp {
                address: format!("127.0.0.1:{port}"),
            }],
            timeout_ms: 500,
            dnssec: false,
            fake_ip: None,
            service: None,
        }
    }
    fn listen(address: &str) -> ListenConfig {
        ListenConfig {
            listen: address.into(),
            allow_lan: false,
            allowed_clients: vec![],
        }
    }
    fn question(name: &str, kind: RecordType) -> Message {
        let mut q = Message::query();
        q.metadata.id = 0x1234;
        q.metadata.recursion_desired = true;
        q.add_query(WireQuery::query(Name::from_ascii(name).unwrap(), kind));
        q
    }
    async fn udp(address: &str, q: &Message) -> Message {
        let socket = UdpSocket::bind(if address.starts_with('[') {
            "[::1]:0"
        } else {
            "127.0.0.1:0"
        })
        .await
        .unwrap();
        socket.send_to(&q.to_vec().unwrap(), address).await.unwrap();
        let mut bytes = [0; 65535];
        let (len, _) = tokio::time::timeout(Duration::from_secs(3), socket.recv_from(&mut bytes))
            .await
            .unwrap()
            .unwrap();
        Message::from_vec(&bytes[..len]).unwrap()
    }
    async fn tcp(socket: &mut TcpStream, q: &Message) -> Message {
        let bytes = q.to_vec().unwrap();
        socket
            .write_all(&(bytes.len() as u16).to_be_bytes())
            .await
            .unwrap();
        socket.write_all(&bytes).await.unwrap();
        let len = tokio::time::timeout(Duration::from_secs(3), socket.read_u16())
            .await
            .unwrap()
            .unwrap();
        let mut bytes = vec![0; usize::from(len)];
        socket.read_exact(&mut bytes).await.unwrap();
        Message::from_vec(&bytes).unwrap()
    }
    #[tokio::test]
    async fn service_protocols_cache_negative_large_truncation_tcp_and_stop() {
        let _gate = SERIAL.lock().await;
        let upstream = support::fixture().await;
        assert!(upstream.pem.starts_with("-----BEGIN"));
        let mut roots = rustls::RootCertStore::empty();
        roots.add(upstream.root.clone()).unwrap();
        let trust = rustls::ClientConfig::builder_with_provider(Arc::new(
            rustls::crypto::ring::default_provider(),
        ))
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_root_certificates(roots)
        .with_no_client_auth();
        for server in [
            Server::Udp {
                address: format!("127.0.0.1:{}", upstream.port),
            },
            Server::Tcp {
                address: format!("127.0.0.1:{}", upstream.port),
            },
            Server::Dot {
                address: format!("127.0.0.1:{}", upstream.dot_port),
                server_name: "localhost".into(),
            },
            Server::Doh {
                url: format!("https://localhost:{}/dns-query", upstream.doh_port),
                bootstrap: format!("127.0.0.1:{}", upstream.doh_port),
            },
        ] {
            let mut settings = config(upstream.port);
            settings.servers = vec![server];
            let resolver = Arc::new(Resolver::with_tls_config(settings, trust.clone()).unwrap());
            let mut service = Service::start(listen("127.0.0.1:0"), resolver)
                .await
                .unwrap();
            let address = service.status().address;
            let q = question("alias.example.test.", RecordType::A);
            let first = udp(&address, &q).await;
            assert_eq!(first.metadata.id, q.metadata.id);
            assert_eq!(first.answers.len(), 2);
            assert!(!first.metadata.authentic_data);
            let count = upstream.count.load(Ordering::SeqCst);
            udp(&address, &q).await;
            assert_eq!(upstream.count.load(Ordering::SeqCst), count);
            let negative = udp(&address, &question("nxdomain.example.test.", RecordType::A)).await;
            assert_eq!(negative.metadata.response_code, ResponseCode::NXDomain);
            assert!(
                negative
                    .authorities
                    .iter()
                    .any(|r| r.record_type() == RecordType::SOA)
            );
            let large = question("large.example.test.", RecordType::TXT);
            assert!(udp(&address, &large).await.metadata.truncation);
            let mut stream = TcpStream::connect(&address).await.unwrap();
            assert!(!tcp(&mut stream, &large).await.metadata.truncation);
            assert_eq!(tcp(&mut stream, &q).await.answers.len(), 2);
            let refused = udp(&address, &question("example.test.", RecordType::ANY)).await;
            assert_eq!(refused.metadata.response_code, ResponseCode::Refused);
            assert!(service.status().queries >= 7);
            service.stop().await.unwrap();
            assert!(!service.status().running);
            let mut closed = [0; 1];
            assert_eq!(
                tokio::time::timeout(Duration::from_secs(1), stream.read(&mut closed))
                    .await
                    .unwrap()
                    .unwrap(),
                0
            );
            tokio::net::TcpListener::bind(&address).await.unwrap();
            UdpSocket::bind(&address).await.unwrap();
        }
    }
    #[tokio::test]
    async fn service_dnssec_proof_fake_ip_ad_flags_and_edns_version() {
        let _gate = SERIAL.lock().await;
        let upstream = signed::fixture().await;
        let mut settings = config(upstream.port);
        settings.dnssec = true;
        let resolver =
            Arc::new(Resolver::with_trust_anchors(settings, upstream.anchors.clone()).unwrap());
        let mut service = Service::start(listen("127.0.0.1:0"), resolver)
            .await
            .unwrap();
        let mut q = question("valid.", RecordType::A);
        let mut edns = Edns::new();
        edns.set_dnssec_ok(true);
        q.edns = Some(edns);
        let result = udp(&service.status().address, &q).await;
        assert!(result.metadata.authentic_data);
        q.queries[0].name = Name::from_ascii("bogus.").unwrap();
        let bad = udp(&service.status().address, &q).await;
        assert_eq!(bad.metadata.response_code, ResponseCode::ServFail);
        assert!(!bad.metadata.authentic_data);
        q.queries[0].name = Name::from_ascii("nxdomain.").unwrap();
        let negative = udp(&service.status().address, &q).await;
        assert_eq!(negative.metadata.response_code, ResponseCode::NXDomain);
        assert!(negative.metadata.authentic_data);
        q.edns.as_mut().unwrap().set_version(1);
        let version = udp(&service.status().address, &q).await;
        assert_eq!(u16::from(version.metadata.response_code), 16);
        assert_eq!(version.edns.unwrap().version(), 0);
        service.stop().await.unwrap();
        let mut settings = config(9);
        settings.fake_ip = Some(fluxa_core::dns::fake_ip::Config {
            ipv4_range: "198.18.0.0/24".into(),
            ipv6_range: "fd00:198:18::/64".into(),
            capacity: 2,
            exclude: vec![],
        });
        settings.dnssec = true;
        let resolver = Arc::new(Resolver::new(settings).unwrap());
        let mut service = Service::start(listen("127.0.0.1:0"), resolver)
            .await
            .unwrap();
        let mut q = question("fake.test.", RecordType::A);
        q.metadata.authentic_data = true;
        q.edns = Some(Edns::new());
        q.edns.as_mut().unwrap().set_dnssec_ok(true);
        let result = udp(&service.status().address, &q).await;
        assert_eq!(result.answers[0].data.to_string(), "198.18.0.1");
        assert!(!result.metadata.authentic_data);
        service.stop().await.unwrap();
    }
    #[tokio::test]
    async fn service_acl_rate_malformed_cancel_idle_and_ipv6() {
        let _gate = SERIAL.lock().await;
        let mut settings = config(9);
        settings.fake_ip = Some(fluxa_core::dns::fake_ip::Config {
            ipv4_range: "198.18.0.0/24".into(),
            ipv6_range: "fd00:198:18::/64".into(),
            capacity: 2,
            exclude: vec![],
        });
        let resolver = Arc::new(Resolver::new(settings).unwrap());
        let mut deny = listen("127.0.0.1:0");
        deny.allowed_clients = vec!["10.0.0.0/8".into()];
        let mut service = Service::start(deny, resolver.clone()).await.unwrap();
        let socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        socket
            .send_to(
                &question("fake.test.", RecordType::A).to_vec().unwrap(),
                service.status().address,
            )
            .await
            .unwrap();
        assert!(
            tokio::time::timeout(Duration::from_millis(100), socket.recv_from(&mut [0; 512]))
                .await
                .is_err()
        );
        assert!(service.status().rejected > 0);
        service.stop().await.unwrap();
        let mut service = Service::start(listen("127.0.0.1:0"), resolver.clone())
            .await
            .unwrap();
        let address = service.status().address;
        socket.send_to(&[0x12, 0x34, 0], &address).await.unwrap();
        let mut bytes = [0; 512];
        let (len, _) = socket.recv_from(&mut bytes).await.unwrap();
        let malformed = Message::from_vec(&bytes[..len]).unwrap();
        assert_eq!(malformed.metadata.response_code, ResponseCode::FormErr);
        assert_eq!(malformed.metadata.id, 0x1234);
        for _ in 0..40 {
            socket
                .send_to(
                    &question("fake.test.", RecordType::A).to_vec().unwrap(),
                    &address,
                )
                .await
                .unwrap();
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(service.status().rejected >= 8);
        assert!(service.status().queries <= 32);
        let mut idle = TcpStream::connect(&address).await.unwrap();
        tokio::time::timeout(Duration::from_secs(1), service.stop())
            .await
            .unwrap()
            .unwrap();
        let closed = idle.read(&mut [0; 1]).await;
        assert!(matches!(closed, Ok(0)) || closed.is_err());
        let mut service = Service::start(listen("[::1]:0"), resolver).await.unwrap();
        let ipv6 = udp(
            &service.status().address,
            &question("fake.test.", RecordType::AAAA),
        )
        .await;
        assert!(
            ipv6.answers
                .iter()
                .any(|r| r.record_type() == RecordType::AAAA)
        );
        service.stop().await.unwrap();
    }
    #[test]
    fn service_config_rejects_public_clients_and_implicit_lan_listen() {
        assert!(listen("0.0.0.0:53").validate().is_err());
        for cidr in [
            "0.0.0.0/0",
            "8.8.8.0/24",
            "192.168.1.1/24",
            "::/0",
            "2001:4860::/32",
            "invalid",
        ] {
            let mut cfg = listen("0.0.0.0:0");
            cfg.allow_lan = true;
            cfg.allowed_clients = vec![cidr.into()];
            assert!(cfg.validate().is_err());
        }
        let mut cfg = listen("0.0.0.0:0");
        cfg.allow_lan = true;
        cfg.allowed_clients = vec!["192.168.1.0/24".into(), "fd00::/8".into()];
        cfg.validate().unwrap();
    }

    #[tokio::test]
    async fn service_failed_bind_drop_and_global_quota_release() {
        let _serial = SERIAL.lock().await;
        let fixture = support::fixture().await;
        let occupied = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let address = occupied.local_addr().unwrap();
        let mut cfg = config(fixture.port);
        cfg.service = Some(listen(&address.to_string()));
        assert!(Arc::new(Resolver::new(cfg).unwrap()).serve().await.is_err());
        let tcp = tokio::net::TcpListener::bind(address).await.unwrap();
        drop(tcp);
        drop(occupied);
        let mut services = Vec::new();
        for _ in 0..4 {
            let mut cfg = config(fixture.port);
            cfg.service = Some(listen("127.0.0.1:0"));
            services.push(Arc::new(Resolver::new(cfg).unwrap()).serve().await.unwrap());
        }
        let mut cfg = config(fixture.port);
        cfg.service = Some(listen("127.0.0.1:0"));
        let resolver = Arc::new(Resolver::new(cfg).unwrap());
        assert!(matches!(
            resolver.clone().serve().await,
            Err(fluxa_core::error::Error::ResourceLimit)
        ));
        let dropped = services.pop().unwrap();
        let address = dropped.status().address;
        drop(dropped);
        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                if let Ok(socket) = UdpSocket::bind(&address).await {
                    drop(socket);
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        let mut replacement = resolver.serve().await.unwrap();
        replacement.stop().await.unwrap();
        for mut service in services {
            service.stop().await.unwrap();
        }
    }
}
