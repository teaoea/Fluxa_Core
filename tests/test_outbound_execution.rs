//! 测试文件：tests/test_outbound_execution.rs
//! 测试类型：单元测试、真实回环出站集成
//! 测试对象：src/outbound/execution.rs
//! 测试内容：
//! - udp_node_connect_failure_retries_before_datagram_send：UDP代理承载建连失败才尝试显式direct，不复制业务报文。
//! - cancel_during_node_connect_never_reaches_direct：节点阶段取消及时退出，无下一候选拨号。
//! - destination_connect_failure_does_not_try_next_node：请求接收方失败不触发节点自动连接重试。
//! - ordered_node_failures_use_only_explicit_direct：真实拒绝/超时按顺序尝试，显式直连才可用，reject终止；列表中任一节点未配置（含首个有效、后续缺失）时拨号前整体报 Unsupported。
//! - successful_direct_stream_never_replays_on_failure：成功选中后读写不切换其他候选。
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
                connection_id: None,
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
                    connection_id: None,
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
                connection_id: None,
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
                        connection_id: None,
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

fn mixed_profile(name: &str, port: u16) -> crate::protocol::config::Profile {
    crate::protocol::config::Profile {
        id: name.into(),
        uri: format!(
            "vless://00000000-0000-0000-0000-000000000001@127.0.0.1:{port}?security=reality&type=tcp&encryption=none&flow=xtls-rprx-vision&sni=localhost&pbk=AQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQE&sid=01&fp=chrome133"
        ),
    }
}
fn mixed_request(port: u16) -> RoutedTarget {
    RoutedTarget {
        target: Target {
            domain: None,
            address: Some("127.0.0.1".into()),
            port,
            transport: Transport::Tcp,
        },
        timeout_ms: 1200,
        family: Family::Ipv4,
        mapping_generation: None,
    }
}
fn mixed_router(value: &str) -> std::sync::Arc<Router> {
    std::sync::Arc::new(
        Router::compile(crate::routing::Config {
            fallback: Policy::Proxy,
            outbound: Some(value.into()),
            ..Default::default()
        })
        .unwrap(),
    )
}
#[tokio::test]
async fn ordered_node_failures_use_only_explicit_direct() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let target = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let target_port = target.local_addr().unwrap().port();
    let closed = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = closed.local_addr().unwrap().port();
    drop(closed);
    let stalled = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let stalled_port = stalled.local_addr().unwrap().port();
    let accepted = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let seen = accepted.clone();
    let peer = tokio::spawn(async move {
        let (mut socket, _) = stalled.accept().await.unwrap();
        seen.store(true, std::sync::atomic::Ordering::SeqCst);
        let mut bytes = [0; 4096];
        let _ = socket.read(&mut bytes).await;
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    });
    let manager = Manager::default();
    let router = mixed_router("(one,two,direct)");
    manager
        .configure_network_with_outbounds(
            None,
            Some(router),
            Default::default(),
            vec![
                mixed_profile("one", port),
                mixed_profile("two", stalled_port),
            ],
        )
        .unwrap();
    let (log_tx, log_rx) = std::sync::mpsc::sync_channel(64);
    manager.set_log_output(log_tx).unwrap();
    manager
        .set_log_level(crate::configuration::LogFilter::Info)
        .unwrap();
    let opened = manager
        .connect_configured(mixed_request(target_port))
        .await
        .unwrap();
    let events: Vec<_> = log_rx.try_iter().collect();
    assert!(
        events
            .iter()
            .any(|v| v.event == crate::logging::Event::NodeRetry)
    );
    assert!(
        events
            .iter()
            .all(|v| v.context.connection_id == Some(opened.connection.connection_id))
    );
    assert_eq!(
        events
            .iter()
            .filter(|v| v.event == crate::logging::Event::ConnectionCreated)
            .count(),
        1
    );
    assert!(accepted.load(std::sync::atomic::Ordering::SeqCst));
    assert_eq!(opened.decision.policy, Policy::Direct);
    assert_eq!(opened.decision.outbound, None);
    let (mut remote, _) = target.accept().await.unwrap();
    manager
        .write(opened.connection.id, b"single", 1000)
        .await
        .unwrap();
    let mut bytes = [0; 6];
    remote.read_exact(&mut bytes).await.unwrap();
    assert_eq!(&bytes, b"single");
    remote.write_all(b"reply").await.unwrap();
    assert_eq!(
        manager
            .read(opened.connection.id, 16, 1000)
            .await
            .unwrap()
            .bytes
            .as_slice(),
        b"reply"
    );
    manager.close(opened.connection.id).await.unwrap();
    peer.abort();
    let _ = peer.await;
    // (one,missing)：首个候选是已配置节点、后续候选不存在；必须在任何拨号前整体报错，不能只用有效部分。
    for chain in [
        "(one,reject)",
        "(one,three)",
        "(missing,direct)",
        "(one,missing)",
    ] {
        manager
            .configure_network_with_outbounds(
                None,
                Some(mixed_router(chain)),
                Default::default(),
                vec![mixed_profile("one", port), mixed_profile("three", port)],
            )
            .unwrap();
        let result = manager.connect_configured(mixed_request(target_port)).await;
        if chain.ends_with("reject)") {
            assert!(matches!(result, Err(Error::Rejected)));
        } else if chain.contains("missing") {
            assert!(matches!(result, Err(Error::Unsupported)));
        } else {
            assert!(matches!(result, Err(Error::Network)));
        }
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(30), target.accept())
                .await
                .is_err()
        );
    }
    manager.close_all().await.unwrap();
}
#[tokio::test]
async fn successful_direct_stream_never_replays_on_failure() {
    use tokio::io::AsyncReadExt;
    let target = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = target.local_addr().unwrap().port();
    let next = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let next_port = next.local_addr().unwrap().port();
    let manager = Manager::default();
    manager
        .configure_network_with_outbounds(
            None,
            Some(mixed_router("(direct,next)")),
            Default::default(),
            vec![mixed_profile("next", next_port)],
        )
        .unwrap();
    let opened = manager
        .connect_configured(mixed_request(port))
        .await
        .unwrap();
    assert_eq!(opened.decision.policy, Policy::Direct);
    let (mut socket, _) = target.accept().await.unwrap();
    manager
        .write(opened.connection.id, b"once", 1000)
        .await
        .unwrap();
    let mut bytes = [0; 4];
    socket.read_exact(&mut bytes).await.unwrap();
    drop(socket);
    assert!(
        manager
            .read(opened.connection.id, 16, 1000)
            .await
            .unwrap()
            .eof
    );
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(50), next.accept())
            .await
            .is_err()
    );
    manager.close(opened.connection.id).await.unwrap();
    manager.close_all().await.unwrap();
}

#[tokio::test]
async fn destination_connect_failure_does_not_try_next_node() {
    let closed = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = closed.local_addr().unwrap().port();
    drop(closed);
    let next = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let manager = Manager::default();
    manager
        .configure_network_with_outbounds(
            None,
            Some(mixed_router("(direct,next)")),
            Default::default(),
            vec![mixed_profile("next", next.local_addr().unwrap().port())],
        )
        .unwrap();
    assert!(
        manager
            .connect_configured(mixed_request(port))
            .await
            .is_err()
    );
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(50), next.accept())
            .await
            .is_err()
    );
    manager.close_all().await.unwrap();
}

#[tokio::test]
async fn udp_node_connect_failure_retries_before_datagram_send() {
    let peer = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let closed = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = closed.local_addr().unwrap().port();
    drop(closed);
    let manager = Manager::default();
    manager
        .configure_network_with_outbounds(
            None,
            Some(mixed_router("(one,direct)")),
            Default::default(),
            vec![mixed_profile("one", port)],
        )
        .unwrap();
    let mut request = mixed_request(peer.local_addr().unwrap().port());
    request.target.transport = Transport::Udp;
    let opened = manager.connect_configured(request).await.unwrap();
    assert_eq!(opened.decision.policy, Policy::Direct);
    manager
        .write(opened.connection.id, b"one-datagram", 1000)
        .await
        .unwrap();
    let mut bytes = [0; 32];
    let (len, from) = tokio::time::timeout(
        std::time::Duration::from_secs(1),
        peer.recv_from(&mut bytes),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(&bytes[..len], b"one-datagram");
    peer.send_to(b"udp-reply", from).await.unwrap();
    assert_eq!(
        manager
            .read(opened.connection.id, 32, 1000)
            .await
            .unwrap()
            .bytes
            .as_slice(),
        b"udp-reply"
    );
    assert!(
        tokio::time::timeout(
            std::time::Duration::from_millis(30),
            peer.recv_from(&mut bytes)
        )
        .await
        .is_err()
    );
    manager.close(opened.connection.id).await.unwrap();
    manager.close_all().await.unwrap();
}
#[tokio::test]
async fn cancel_during_node_connect_never_reaches_direct() {
    let node = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = node.local_addr().unwrap().port();
    let target = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let target_port = target.local_addr().unwrap().port();
    let manager = std::sync::Arc::new(Manager::default());
    manager
        .configure_network_with_outbounds(
            None,
            Some(mixed_router("(one,direct)")),
            Default::default(),
            vec![mixed_profile("one", port)],
        )
        .unwrap();
    let clone = manager.clone();
    let task =
        tokio::spawn(async move { clone.connect_configured(mixed_request(target_port)).await });
    let (socket, _) = tokio::time::timeout(std::time::Duration::from_secs(1), node.accept())
        .await
        .unwrap()
        .unwrap();
    manager.close_all().await.unwrap();
    assert!(matches!(
        tokio::time::timeout(std::time::Duration::from_secs(1), task)
            .await
            .unwrap()
            .unwrap(),
        Err(Error::Cancelled)
    ));
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(30), target.accept())
            .await
            .is_err()
    );
    drop(socket);
}
