//! 测试文件：tests/test_outbound_execution.rs
//! 测试类型：单元测试、真实回环出站集成
//! 测试对象：src/outbound/execution.rs
//! 测试内容：
//! - process_proof_admits_new_flow_without_owning_live_connection：旧源 FD 不能授权新流，已建立流继续完成。
//! - scoped_quic_is_decided_before_real_udp_outbound：真实首包决定出站，普通 UDP/缺首包/错误节点不得直连。
//!
//! 测试依据：QUIC 必须识别报文；域名限定协议按 suffix 匹配；错误禁止默认 DIRECT。
//!
//! 前置条件：本机 DNS/UDP 夹具；不证明 VPN 或远端 QUIC 互通。
use super::*;
#[tokio::test]
async fn scoped_quic_is_decided_before_real_udp_outbound() {
    use crate::{
        dns::{Config as DnsConfig, Server},
        outbound::Family,
        routing::{Config, Match, Rule},
    };
    use std::time::Duration;
    let fixture = crate::test_fixture_https::dns::fixture().await;
    let resolver = std::sync::Arc::new(
        Resolver::new(DnsConfig {
            servers: vec![Server::Udp {
                address: format!("127.0.0.1:{}", fixture.port),
            }],
            timeout_ms: 1000,
            dnssec: false,
            fake_ip: None,
            service: None,
        })
        .unwrap(),
    );
    let peer = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let manager = Manager::default();
    let router = Router::compile(Config {
        rules: vec![Rule {
            r#type: Match::DomainQuic,
            value: "example.test".into(),
            policy: Policy::Direct,
            outbound: None,
            protocol: None,
        }],
        ..Config::default()
    })
    .unwrap();
    let request = || RoutedTarget {
        target: Target {
            domain: Some("connect.example.test".into()),
            address: None,
            port: peer.local_addr().unwrap().port(),
            transport: Transport::Udp,
        },
        timeout_ms: 1000,
        family: Family::Ipv4,
        mapping_generation: None,
    };
    let mut quic = vec![0xc0, 0, 0, 0, 1, 1, 1, 1, 2, 0, 17];
    quic.extend([0; 17]);
    let open = |payload| {
        manager.connect_with_snapshot(
            &router,
            Some(&resolver),
            request(),
            manager.network_snapshot().unwrap(),
            None,
            FlowFacts {
                permission: None,
                datagram: payload,
                source: None,
            },
        )
    };
    assert!(
        matches!(open(None).await, Err(Error::Unsupported)),
        "无首包不能根据端口推断协议"
    );
    assert!(matches!(
        open(Some(b"plain-udp")).await,
        Err(Error::Rejected)
    ));
    let opened = open(Some(&quic)).await.unwrap();
    assert_eq!(opened.decision.policy, Policy::Direct);
    manager
        .write(opened.connection.id, &quic, 1000)
        .await
        .unwrap();
    let mut bytes = [0; 64];
    let (length, _) = tokio::time::timeout(Duration::from_secs(1), peer.recv_from(&mut bytes))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(&bytes[..length], quic);
    manager.close(opened.connection.id).await.unwrap();
    let proxy = Router::compile(Config {
        rules: vec![Rule {
            r#type: Match::DomainQuic,
            value: "example.test".into(),
            policy: Policy::Proxy,
            outbound: Some("missing".into()),
            protocol: None,
        }],
        ..Config::default()
    })
    .unwrap();
    assert!(matches!(
        manager
            .connect_with_snapshot(
                &proxy,
                Some(&resolver),
                request(),
                manager.network_snapshot().unwrap(),
                None,
                FlowFacts {
                    permission: None,
                    datagram: Some(&quic),
                    source: None
                }
            )
            .await,
        Err(Error::Unsupported)
    ));
    assert!(
        tokio::time::timeout(Duration::from_millis(50), peer.recv_from(&mut bytes))
            .await
            .is_err(),
        "缺失节点不得发送或默认直连"
    );
    manager.stop().await.unwrap();
    resolver.close().await.unwrap();
}

#[cfg(target_os = "macos")]
#[tokio::test]
async fn process_proof_admits_new_flow_without_owning_live_connection() {
    use crate::routing::{Config, Match, Rule};
    let peer = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let source = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
    source.connect(peer.local_addr().unwrap()).await.unwrap();
    let flow = crate::platform::process::Flow {
        local: source.local_addr().unwrap(),
        remote: peer.local_addr().unwrap(),
        transport: Transport::Udp,
    };
    let name = std::env::current_exe()
        .unwrap()
        .file_name()
        .unwrap()
        .to_str()
        .unwrap()
        .to_owned();
    let router = Router::compile(Config {
        rules: vec![Rule {
            r#type: Match::ProcessName,
            value: name,
            policy: Policy::Direct,
            outbound: None,
            protocol: None,
        }],
        ..Config::default()
    })
    .unwrap();
    let manager = Manager::default();
    let request = || RoutedTarget {
        target: Target {
            domain: None,
            address: Some("127.0.0.1".into()),
            port: peer.local_addr().unwrap().port(),
            transport: Transport::Udp,
        },
        timeout_ms: 1000,
        family: Family::Ipv4,
        mapping_generation: None,
    };
    let opened = manager
        .connect_with_snapshot(
            &router,
            None,
            request(),
            manager.network_snapshot().unwrap(),
            None,
            FlowFacts {
                permission: None,
                datagram: None,
                source: Some(flow),
            },
        )
        .await
        .unwrap();
    drop(source);
    manager
        .write(opened.connection.id, b"admitted", 1000)
        .await
        .unwrap();
    let mut bytes = [0; 32];
    let (length, _) = tokio::time::timeout(
        std::time::Duration::from_secs(1),
        peer.recv_from(&mut bytes),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(&bytes[..length], b"admitted");
    assert!(
        matches!(
            manager
                .connect_with_snapshot(
                    &router,
                    None,
                    request(),
                    manager.network_snapshot().unwrap(),
                    None,
                    FlowFacts {
                        permission: None,
                        datagram: None,
                        source: Some(flow)
                    }
                )
                .await,
            Err(Error::Rejected)
        ),
        "新流只能重新查 OS，不能沿用旧身份"
    );
    manager.close(opened.connection.id).await.unwrap();
    manager.stop().await.unwrap();
}
