//! 测试文件：tests/test_outbound_resources.rs
//! 测试类型：单元测试、迁移回归
//! 测试对象：src/outbound/resources.rs
//! 测试内容：
//! - idle_expiry_cancels_blocked_read_and_releases_peer：依据现行独立业务约定验证；新来源/模式约定的输入与预期在同名函数明确说明。
//! - datagram_activity_and_timeout_have_distinct_idle_semantics：依据现行独立业务约定验证；新来源/模式约定的输入与预期在同名函数明确说明。
//! - quota_reconfiguration_preserves_live_connections_and_isolation：依据现行独立业务约定验证；新来源/模式约定的输入与预期在同名函数明确说明。
//! - atomic_network_replacement_keeps_snapshot_and_live_endpoint：依据现行独立业务约定验证；新来源/模式约定的输入与预期在同名函数明确说明。
//! - in_flight_dns_retains_original_network_snapshot：依据现行独立业务约定验证；新来源/模式约定的输入与预期在同名函数明确说明。
//!
//! 测试依据：保留原测试的业务/格式/授权/资源约束；《测试文件与报告统一管理原则》，独立语义审查不以实现输出反推预期。
//!
//! 前置条件：本地测试需文件/回环套接字权限；平台限定及忽略项以实际登记为准。
//!
//! 执行状态：由同名 Markdown 报告记录，不在源码中固化通过结论。

// 以下保留迁移前的用例说明；历史执行状态不代表当前版本，当前结果仅见同名报告。
mod public_contract {
    use crate as libfluxa_core;
    use libfluxa_core::{
        error::Error,
        outbound::{Family, Limits, Manager, RoutedTarget},
        routing::{Config, Mode, Policy, Router, Target, Transport},
    };
    use std::{sync::Arc, time::Duration};
    use tokio::{
        io::AsyncReadExt,
        net::{TcpListener, UdpSocket},
        time::{sleep, timeout},
    };

    fn limits(maximum_connections: usize, idle_timeout_ms: u64) -> Limits {
        Limits {
            maximum_connections,
            idle_timeout_ms,
        }
    }
    fn routing(mode: Mode) -> Arc<Router> {
        Arc::new(
            Router::compile(Config {
                mode,
                // global 现在由 final/fallback 决定；本用例明确选择代理，保留原未实现代理失败断言。
                fallback: if mode == Mode::Global {
                    Policy::Proxy
                } else {
                    Policy::Reject
                },
                outbound: (mode == Mode::Global).then(|| "unimplemented".into()),
                ..Config::default()
            })
            .unwrap(),
        )
    }

    #[tokio::test]
    async fn idle_expiry_cancels_blocked_read_and_releases_peer() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let manager = Manager::default();
        manager.configure_limits(limits(1, 100)).unwrap();
        let id = manager
            .open(
                listener.local_addr().unwrap(),
                Transport::Tcp,
                Policy::Direct,
                1000,
            )
            .await
            .unwrap();
        let (mut peer, _) = listener.accept().await.unwrap();
        assert!(matches!(
            timeout(Duration::from_secs(2), manager.read(id, 64, 5000))
                .await
                .unwrap(),
            Err(Error::Cancelled)
        ));
        assert!(manager.status().unwrap().sessions.is_empty());
        assert!(matches!(manager.close(id).await, Err(Error::InvalidState)));
        let mut byte = [0];
        assert_eq!(
            timeout(Duration::from_secs(1), peer.read(&mut byte))
                .await
                .unwrap()
                .unwrap(),
            0
        );
        let next = manager
            .open(
                listener.local_addr().unwrap(),
                Transport::Tcp,
                Policy::Direct,
                1000,
            )
            .await
            .unwrap();
        assert_ne!(next, id);
        manager.close(next).await.unwrap();
    }

    #[tokio::test]
    async fn datagram_activity_and_timeout_have_distinct_idle_semantics() {
        let server = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let manager = Manager::default();
        manager.configure_limits(limits(2, 200)).unwrap();
        let id = manager
            .open(
                server.local_addr().unwrap(),
                Transport::Udp,
                Policy::Direct,
                1000,
            )
            .await
            .unwrap();
        let mut buffer = [0; 16];
        for payload in [b"abc".as_slice(), b"", b"defg"] {
            sleep(Duration::from_millis(80)).await;
            manager.write(id, payload, 1000).await.unwrap();
            let (count, peer) = server.recv_from(&mut buffer).await.unwrap();
            server.send_to(&buffer[..count], peer).await.unwrap();
            assert_eq!(&*manager.read(id, 16, 1000).await.unwrap().bytes, payload);
        }
        let status = manager.status().unwrap();
        assert_eq!(status.sessions.len(), 1);
        assert_eq!(status.sessions[0].uploaded, 7);
        assert_eq!(status.sessions[0].downloaded, 7);
        for _ in 0..3 {
            assert!(matches!(
                manager.read(id, 16, 40).await,
                Err(Error::NetworkTimeout)
            ));
        }
        sleep(Duration::from_millis(150)).await;
        assert!(manager.status().unwrap().sessions.is_empty());
    }

    #[tokio::test]
    async fn quota_reconfiguration_preserves_live_connections_and_isolation() {
        let server = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let manager = Manager::default();
        let first_revision = manager.configure_limits(limits(2, 1000)).unwrap();
        let address = server.local_addr().unwrap();
        let first = manager
            .open(address, Transport::Udp, Policy::Direct, 1000)
            .await
            .unwrap();
        let second = manager
            .open(address, Transport::Udp, Policy::Direct, 1000)
            .await
            .unwrap();
        let changed = manager.configure_limits(limits(1, 2000)).unwrap();
        assert_ne!(first_revision, changed);
        let status = manager.status().unwrap();
        assert_eq!(status.sessions.len(), 2);
        assert!(
            status
                .sessions
                .iter()
                .all(|s| s.revision == first_revision && s.idle_timeout_ms == 1000)
        );
        assert!(matches!(
            manager
                .open(address, Transport::Udp, Policy::Direct, 1000)
                .await,
            Err(Error::ResourceLimit)
        ));
        assert!(matches!(
            manager.configure_limits(limits(0, 100)),
            Err(Error::InvalidInput)
        ));
        assert_eq!(manager.status().unwrap().revision, changed);
        manager.close(first).await.unwrap();
        assert!(matches!(
            manager
                .open(address, Transport::Udp, Policy::Direct, 1000)
                .await,
            Err(Error::ResourceLimit)
        ));
        manager.close(second).await.unwrap();
        let next = manager
            .open(address, Transport::Udp, Policy::Direct, 1000)
            .await
            .unwrap();
        assert_eq!(manager.status().unwrap().sessions[0].idle_timeout_ms, 2000);
        let other = Manager::default();
        let different = other
            .open(address, Transport::Udp, Policy::Direct, 1000)
            .await
            .unwrap();
        assert_ne!(next, different);
        assert!(matches!(other.close(next).await, Err(Error::InvalidState)));
        manager.close_all().await.unwrap();
        other.close_all().await.unwrap();
    }

    #[tokio::test]
    async fn atomic_network_replacement_keeps_snapshot_and_live_endpoint() {
        let server = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let manager = Manager::default();
        let first = manager
            .configure_network(None, Some(routing(Mode::Direct)), limits(4, 5000))
            .unwrap();
        let old = manager.network_snapshot().unwrap();
        let request = RoutedTarget {
            target: Target {
                domain: None,
                address: Some("127.0.0.1".into()),
                port: server.local_addr().unwrap().port(),
                transport: Transport::Udp,
            },
            family: Family::Ipv4,
            timeout_ms: 1000,
            mapping_generation: None,
        };
        let opened = manager.connect_configured(request.clone()).await.unwrap();
        let next = manager
            .configure_network(None, Some(routing(Mode::Global)), limits(1, 100))
            .unwrap();
        assert_ne!(first, next);
        assert_eq!(old.revision, first);
        assert_eq!(
            old.router
                .as_ref()
                .unwrap()
                .evaluate(request.target.clone())
                .unwrap()
                .policy,
            Policy::Direct
        );
        assert!(matches!(
            manager.connect_configured(request).await,
            Err(Error::Unsupported)
        ));
        let status = manager.status().unwrap();
        assert_eq!(status.revision, next);
        assert_eq!(status.sessions[0].revision, first);
        manager
            .write(opened.connection.id, b"old", 1000)
            .await
            .unwrap();
        let mut buffer = [0; 16];
        let (size, peer) = server.recv_from(&mut buffer).await.unwrap();
        server.send_to(&buffer[..size], peer).await.unwrap();
        assert_eq!(
            &*manager
                .read(opened.connection.id, 16, 1000)
                .await
                .unwrap()
                .bytes,
            b"old"
        );
        manager.stop().await.unwrap();
        assert!(matches!(manager.status(), Err(Error::InvalidState)));
        assert!(matches!(
            manager.configure_limits(Limits::default()),
            Err(Error::InvalidState)
        ));
    }

    #[tokio::test]
    async fn in_flight_dns_retains_original_network_snapshot() {
        use hickory_resolver::proto::{
            op::{Message, MessageType},
            rr::{RData, Record, rdata::A},
        };
        use libfluxa_core::dns::{Config as DNSConfig, Resolver, Server};
        let upstream = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let dns = Arc::new(
            Resolver::new(DNSConfig {
                servers: vec![Server::Udp {
                    address: upstream.local_addr().unwrap().to_string(),
                }],
                timeout_ms: 1000,
                dnssec: false,
                fake_ip: None,
                service: None,
            })
            .unwrap(),
        );
        let (arrived_tx, arrived_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = tokio::sync::oneshot::channel();
        let upstream_task = tokio::spawn(async move {
            let mut buffer = [0; 4096];
            let (size, peer) = upstream.recv_from(&mut buffer).await.unwrap();
            let query = Message::from_vec(&buffer[..size]).unwrap();
            arrived_tx.send(()).unwrap();
            release_rx.await.unwrap();
            let mut response = Message::query();
            response.metadata.id = query.metadata.id;
            response.metadata.message_type = MessageType::Response;
            response.metadata.recursion_available = true;
            response.add_query(query.queries[0].clone());
            response.add_answer(Record::from_rdata(
                query.queries[0].name.clone(),
                30,
                RData::A(A(std::net::Ipv4Addr::LOCALHOST)),
            ));
            upstream
                .send_to(&response.to_vec().unwrap(), peer)
                .await
                .unwrap();
        });
        let echo = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let manager = Arc::new(Manager::default());
        let original = manager
            .configure_network(Some(dns), Some(routing(Mode::Direct)), limits(4, 5000))
            .unwrap();
        let pending_manager = Arc::clone(&manager);
        let port = echo.local_addr().unwrap().port();
        let pending = tokio::spawn(async move {
            pending_manager
                .connect_configured(RoutedTarget {
                    target: Target {
                        domain: Some("snapshot.example.test".into()),
                        address: None,
                        port,
                        transport: Transport::Udp,
                    },
                    family: Family::Ipv4,
                    timeout_ms: 1000,
                    mapping_generation: None,
                })
                .await
        });
        timeout(Duration::from_secs(1), arrived_rx)
            .await
            .unwrap()
            .unwrap();
        let replacement = manager
            .configure_network(None, Some(routing(Mode::Global)), limits(1, 100))
            .unwrap();
        release_tx.send(()).unwrap();
        let opened = pending.await.unwrap().unwrap();
        let status = manager.status().unwrap();
        assert_eq!(status.revision, replacement);
        assert_eq!(status.sessions[0].revision, original);
        assert_eq!(status.sessions[0].idle_timeout_ms, 5000);
        assert_eq!(opened.decision.policy, Policy::Direct);
        manager.close(opened.connection.id).await.unwrap();
        upstream_task.await.unwrap();
    }
}
