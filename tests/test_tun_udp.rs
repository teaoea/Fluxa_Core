//! 测试文件：tests/test_tun_udp.rs
//! 测试类型：单元测试、迁移回归
//! 测试对象：src/tun/udp.rs
//! 测试内容：
//! - real_ipv4_ipv6_multiple_datagrams_and_empty_payload：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - reject_missing_configuration_proxy_and_invalid_packets_do_not_send：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - bounded_flows_idle_reuse_backpressure_and_close_release_resources：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - new_flows_use_new_rules_existing_flows_keep_snapshot：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - options_and_return_packet_limits_are_strict：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - fake_ip_requires_live_resolver_generation_and_never_dials_synthetic_address：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - oversize_response_is_an_error_and_drop_cancels_blocked_output：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//!
//! 测试内容：fragmented_returns_preserve_datagrams_and_close_discards_pending。
//! - completed_workers_are_reaped_without_aborting_active_tasks：验证运行期任务回收、有界登记及活动任务连续性。
//! - registered_task_limit_includes_pending_cancellation_and_recovers：验证运行期任务回收、有界登记及活动任务连续性。
//! - udp_flow_churn_reaps_before_registering_new_worker：验证运行期任务回收、有界登记及活动任务连续性。
//! - completed_worker_panic_is_observed_once：验证运行期任务回收、有界登记及活动任务连续性。
//!
//! 测试内容：pmtu_udp_feedback_fragments_next_datagram_and_close_revokes_control。
//! 测试依据：保留原测试的业务/格式/授权/资源约束；《测试文件与报告统一管理原则》，独立语义审查不以实现输出反推预期。
//!
//! 前置条件：本地测试需文件/回环套接字权限；平台限定及忽略项以实际登记为准。
//!
//! 执行状态：由同名 Markdown 报告记录，不在源码中固化通过结论。

use crate::test_fixture_https::dns as support;
// 以下保留迁移前的用例说明；历史执行状态不代表当前版本，当前结果仅见同名报告。
mod public_contract {
    use nodus_core::{
        error::Error,
        outbound::{Limits, Manager},
        routing::{Config, Policy, Router},
        tun::{
            parse,
            udp::{Options, Session, encode},
        },
    };
    use std::{net::SocketAddr, sync::Arc, time::Duration};
    use tokio::net::UdpSocket;
    fn manager(policy: Policy) -> Arc<Manager> {
        let manager = Arc::new(Manager::default());
        manager
            .configure_network(
                None,
                Some(Arc::new(
                    Router::compile(Config {
                        fallback: policy,
                        outbound: (policy == Policy::Proxy).then(|| "missing".into()),
                        ..Config::default()
                    })
                    .unwrap(),
                )),
                Limits::default(),
            )
            .unwrap();
        manager
    }
    async fn next(session: &Session) -> nodus_core::error::Result<nodus_core::tun::udp::Packet> {
        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                if let Some(packet) = session.receive()? {
                    return Ok(packet);
                }
                tokio::time::sleep(Duration::from_millis(2)).await;
            }
        })
        .await
        .unwrap()
    }
    #[tokio::test]
    async fn real_ipv4_ipv6_multiple_datagrams_and_empty_payload() {
        for ip in ["127.0.0.1", "::1"] {
            let peer =
                UdpSocket::bind(format!("{}:0", if ip.contains(':') { "[::1]" } else { ip }))
                    .await
                    .unwrap();
            let destination = peer.local_addr().unwrap();
            let source: SocketAddr = if destination.is_ipv4() {
                "192.0.2.2:3456"
            } else {
                "[2001:db8::2]:3456"
            }
            .parse()
            .unwrap();
            let session = Session::new(manager(Policy::Direct), Options::default()).unwrap();
            for body in [b"abc".as_slice(), &[], &[0, 255, 128, 1, 2]] {
                let packet = encode(source, destination, body, 1500).unwrap();
                session.submit(&packet.bytes, None).await.unwrap();
                let mut received = [0; 1500];
                let (size, address) =
                    tokio::time::timeout(Duration::from_secs(1), peer.recv_from(&mut received))
                        .await
                        .unwrap()
                        .unwrap();
                assert_eq!(&received[..size], body);
                // 两次不同响应均应形成返回包，不限定每次请求只有一个响应。
                for response in [body, b"second".as_slice()] {
                    peer.send_to(response, address).await.unwrap();
                    let packet = next(&session).await.unwrap();
                    let info = parse(&packet.bytes, 1500).unwrap();
                    assert_eq!(info.source, destination.ip().to_string());
                    assert_eq!(info.destination, source.ip().to_string());
                    assert_eq!(
                        (info.source_port, info.destination_port),
                        (destination.port(), source.port())
                    );
                    assert_eq!(
                        &packet.bytes[packet.bytes.len() - info.payload_bytes..],
                        response
                    );
                }
            }
            session.close().await.unwrap();
            session.close().await.unwrap();
            assert!(matches!(session.receive(), Err(Error::InvalidState)));
        }
    }
    #[tokio::test]
    async fn reject_missing_configuration_proxy_and_invalid_packets_do_not_send() {
        let socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let packet = encode(
            "192.0.2.1:12".parse().unwrap(),
            socket.local_addr().unwrap(),
            b"test",
            1500,
        )
        .unwrap();
        for policy in [Policy::Reject, Policy::Proxy] {
            let session = Session::new(manager(policy), Options::default()).unwrap();
            assert!(session.submit(&packet.bytes, None).await.is_err());
        }
        let session = Session::new(Arc::new(Manager::default()), Options::default()).unwrap();
        assert_eq!(
            session.submit(&packet.bytes, None).await,
            Err(Error::InvalidState)
        );
        let session = Session::new(manager(Policy::Direct), Options::default()).unwrap();
        for cut in 0..packet.bytes.len() {
            assert!(session.submit(&packet.bytes[..cut], None).await.is_err());
        }
        let mut corrupt = packet.bytes.to_vec();
        let last = corrupt.len() - 1;
        corrupt[last] ^= 1;
        assert_eq!(
            session.submit(&corrupt, None).await,
            Err(Error::InvalidInput)
        );
        let mut buffer = [0; 64];
        assert!(
            tokio::time::timeout(Duration::from_millis(20), socket.recv_from(&mut buffer))
                .await
                .is_err()
        );
    }
    #[tokio::test]
    async fn bounded_flows_idle_reuse_backpressure_and_close_release_resources() {
        let socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let destination = socket.local_addr().unwrap();
        let manager = manager(Policy::Direct);
        let session = Session::new(
            Arc::clone(&manager),
            Options {
                maximum_flows: 1,
                output_capacity: 1,
                idle_timeout_ms: 100,
                ..Options::default()
            },
        )
        .unwrap();
        let first = encode("192.0.2.1:12".parse().unwrap(), destination, b"1", 1500).unwrap();
        let second = encode("192.0.2.1:13".parse().unwrap(), destination, b"2", 1500).unwrap();
        session.submit(&first.bytes, None).await.unwrap();
        assert_eq!(
            session.submit(&second.bytes, None).await,
            Err(Error::ResourceLimit)
        );
        let mut buffer = [0; 64];
        let (_, peer) = tokio::time::timeout(Duration::from_secs(2), socket.recv_from(&mut buffer))
            .await
            .unwrap()
            .unwrap();
        for n in 0..8 {
            socket.send_to(&[n], peer).await.unwrap();
        }
        tokio::time::sleep(Duration::from_millis(30)).await;
        for n in 0..8 {
            let packet = next(&session).await.unwrap();
            assert_eq!(*packet.bytes.last().unwrap(), n);
        }
        tokio::time::sleep(Duration::from_millis(150)).await;
        session.submit(&second.bytes, None).await.unwrap();
        session.close().await.unwrap();
        assert_eq!(
            session.submit(&first.bytes, None).await,
            Err(Error::InvalidState)
        );
        // 已关闭的会话只回收自己的连接，不停止共用出站管理器。
        let other = Session::new(manager, Options::default()).unwrap();
        other.submit(&first.bytes, None).await.unwrap();
        other.close().await.unwrap();
    }
    #[tokio::test]
    async fn new_flows_use_new_rules_existing_flows_keep_snapshot() {
        let socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let manager = manager(Policy::Direct);
        let session = Session::new(Arc::clone(&manager), Options::default()).unwrap();
        let packet = encode(
            "192.0.2.1:12".parse().unwrap(),
            socket.local_addr().unwrap(),
            b"x",
            1500,
        )
        .unwrap();
        session.submit(&packet.bytes, None).await.unwrap();
        manager
            .configure_network(
                None,
                Some(Arc::new(Router::compile(Config::default()).unwrap())),
                Limits::default(),
            )
            .unwrap();
        session.submit(&packet.bytes, None).await.unwrap();
        let packet = encode(
            "192.0.2.1:13".parse().unwrap(),
            socket.local_addr().unwrap(),
            b"x",
            1500,
        )
        .unwrap();
        assert_eq!(
            session.submit(&packet.bytes, None).await,
            Err(Error::Rejected)
        );
    }
    #[test]
    fn options_and_return_packet_limits_are_strict() {
        for options in [
            Options {
                mtu: 1279,
                ..Options::default()
            },
            Options {
                maximum_flows: 33,
                ..Options::default()
            },
            Options {
                output_capacity: 65,
                ..Options::default()
            },
            Options {
                timeout_ms: 0,
                ..Options::default()
            },
        ] {
            assert!(matches!(
                Session::new(manager(Policy::Direct), options),
                Err(Error::InvalidInput)
            ));
        }
        assert!(
            encode(
                "127.0.0.1:1".parse().unwrap(),
                "[::1]:2".parse().unwrap(),
                &[],
                1500
            )
            .is_err()
        );
        assert!(
            encode(
                "127.0.0.1:1".parse().unwrap(),
                "127.0.0.1:2".parse().unwrap(),
                &vec![0; 1500],
                1500
            )
            .is_err()
        );
    }

    use super::support;
    #[tokio::test]
    async fn fake_ip_requires_live_resolver_generation_and_never_dials_synthetic_address() {
        use nodus_core::dns::{Config as DnsConfig, Query, Resolver, Server, fake_ip};
        let fixture = support::fixture().await;
        let resolver = Arc::new(
            Resolver::new(DnsConfig {
                servers: vec![Server::Udp {
                    address: format!("127.0.0.1:{}", fixture.port),
                }],
                timeout_ms: 1000,
                dnssec: false,
                service: None,
                fake_ip: Some(fake_ip::Config {
                    ipv4_range: "198.18.0.0/16".into(),
                    ipv6_range: "fd00:198:18::/64".into(),
                    capacity: 16,
                    exclude: vec![],
                }),
            })
            .unwrap(),
        );
        let socket = UdpSocket::bind("[::1]:0").await.unwrap();
        let manager = manager(Policy::Direct);
        manager
            .configure_network(
                Some(Arc::clone(&resolver)),
                Some(Arc::new(
                    Router::compile(Config {
                        fallback: Policy::Direct,
                        ..Config::default()
                    })
                    .unwrap(),
                )),
                Limits::default(),
            )
            .unwrap();
        let session = Session::new(Arc::clone(&manager), Options::default()).unwrap();
        let address = resolver
            .resolve(Query {
                name: "connect.example.test".into(),
                query_type: "A".into(),
            })
            .await
            .unwrap()
            .addresses[0]
            .parse()
            .unwrap();
        let destination = SocketAddr::new(address, socket.local_addr().unwrap().port());
        let packet = encode(
            "192.0.2.1:12".parse().unwrap(),
            destination,
            b"mapped",
            1500,
        )
        .unwrap();
        assert_eq!(
            session.submit(&packet.bytes, None).await,
            Err(Error::InvalidState)
        );
        let generation = resolver.fake_ip_generation().unwrap();
        session
            .submit(&packet.bytes, Some(generation))
            .await
            .unwrap();
        let mut buffer = [0; 64];
        let (size, peer) =
            tokio::time::timeout(Duration::from_secs(2), socket.recv_from(&mut buffer))
                .await
                .unwrap()
                .unwrap();
        assert_eq!(&buffer[..size], b"mapped");
        socket.send_to(b"reply", peer).await.unwrap();
        assert_eq!(
            parse(&next(&session).await.unwrap().bytes, 1500)
                .unwrap()
                .source,
            address.to_string()
        );
        session.close().await.unwrap();
        resolver.reset_fake_ip().unwrap();
        let session = Session::new(manager, Options::default()).unwrap();
        assert!(
            session
                .submit(&packet.bytes, Some(generation))
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn oversize_response_is_an_error_and_drop_cancels_blocked_output() {
        let socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let destination = socket.local_addr().unwrap();
        let packet = encode("192.0.2.1:12".parse().unwrap(), destination, b"x", 1280).unwrap();
        let session = Session::new(
            manager(Policy::Direct),
            Options {
                mtu: 1280,
                output_capacity: 1,
                ..Options::default()
            },
        )
        .unwrap();
        session.submit(&packet.bytes, None).await.unwrap();
        let mut buffer = [0; 64];
        let (_, peer) = tokio::time::timeout(Duration::from_secs(2), socket.recv_from(&mut buffer))
            .await
            .unwrap()
            .unwrap();
        socket.send_to(&vec![0; 9000], peer).await.unwrap();
        assert!(matches!(next(&session).await, Err(Error::ResourceLimit)));
        session.close().await.unwrap();
        for _ in 0..40 {
            let session = Arc::new(
                Session::new(
                    manager(Policy::Direct),
                    Options {
                        output_capacity: 1,
                        ..Options::default()
                    },
                )
                .unwrap(),
            );
            session.submit(&packet.bytes, None).await.unwrap();
            let (_, peer) =
                tokio::time::timeout(Duration::from_secs(2), socket.recv_from(&mut buffer))
                    .await
                    .unwrap()
                    .unwrap();
            for _ in 0..4 {
                socket.send_to(b"blocked", peer).await.unwrap();
            }
            tokio::time::sleep(Duration::from_millis(1)).await;
            let querying = Arc::clone(&session);
            let task = tokio::spawn(async move {
                for _ in 0..100 {
                    let _ = querying.receive();
                    tokio::task::yield_now().await;
                }
            });
            session.close().await.unwrap();
            task.await.unwrap();
            drop(session);
            tokio::task::yield_now().await;
        }
    }
}

#[tokio::test]
async fn fragmented_returns_preserve_datagrams_and_close_discards_pending() {
    use super::*;
    use crate::{
        outbound::Limits,
        routing::{Config, Policy, Router},
    };
    use std::time::Duration;
    for ipv6 in [false, true] {
        let peer = tokio::net::UdpSocket::bind(if ipv6 { "[::1]:0" } else { "127.0.0.1:0" })
            .await
            .unwrap();
        let source = if ipv6 {
            "[2001:db8::1]:2222"
        } else {
            "192.0.2.1:2222"
        }
        .parse()
        .unwrap();
        let manager = Arc::new(Manager::default());
        manager
            .configure_network(
                None,
                Some(Arc::new(
                    Router::compile(Config {
                        fallback: Policy::Direct,
                        ..Config::default()
                    })
                    .unwrap(),
                )),
                Limits::default(),
            )
            .unwrap();
        let session = Session::new(
            manager.clone(),
            Options {
                mtu: 1280,
                output_capacity: 1,
                ..Options::default()
            },
        )
        .unwrap();
        let request = encode(source, peer.local_addr().unwrap(), b"large", 1280).unwrap();
        session.submit(&request.bytes, None).await.unwrap();
        let mut buffer = [0; 64];
        let (_, address) =
            tokio::time::timeout(Duration::from_secs(2), peer.recv_from(&mut buffer))
                .await
                .unwrap()
                .unwrap();
        let payload: Vec<u8> = (0..8001).map(|i| (i % 251) as u8).collect();
        peer.send_to(&payload, address).await.unwrap();
        peer.send_to(b"next-datagram", address).await.unwrap();
        let mut table = crate::tun::ip::Reassembly::default();
        let mut received = Vec::new();
        tokio::time::timeout(Duration::from_secs(3), async {
            while received.len() != 2 {
                if let Some(packet) = session.receive().unwrap() {
                    assert!(packet.bytes.len() <= 1280);
                    if let Some(full) = table
                        .submit(&packet.bytes, 1280, None, std::time::Instant::now())
                        .unwrap()
                    {
                        let info = crate::tun::parse(full.bytes(), 9000).unwrap();
                        received
                            .push(full.bytes()[full.bytes().len() - info.payload_bytes..].to_vec());
                    }
                } else {
                    tokio::task::yield_now().await;
                }
            }
        })
        .await
        .unwrap();
        assert_eq!(received, vec![payload.clone(), b"next-datagram".to_vec()]);
        peer.send_to(&payload, address).await.unwrap();
        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                if session.receive().unwrap().is_some() {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        assert!(!session.inner.receiver.lock().unwrap().pending.is_empty());
        let (a, b) = tokio::join!(session.close(), session.close());
        a.unwrap();
        b.unwrap();
        let output = session.inner.receiver.lock().unwrap();
        assert!(output.pending.is_empty());
        assert!(output.channel.is_empty());
        assert!(matches!(session.receive(), Err(Error::InvalidState)));
    }
}

mod task_registration_contract {
    use super::super::*;
    use std::{
        sync::atomic::{AtomicUsize, Ordering},
        time::Duration,
    };
    use tokio::{net::UdpSocket, sync::oneshot};
    async fn finished(handle: &tokio::task::AbortHandle) {
        tokio::time::timeout(Duration::from_secs(2), async {
            while !handle.is_finished() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
    }
    fn session() -> Session {
        Session::new(Arc::new(Manager::default()), Options::default()).unwrap()
    }
    #[tokio::test]
    async fn completed_workers_are_reaped_without_aborting_active_tasks() {
        let owner = session();
        let active = owner.spawn(std::future::pending()).unwrap();
        // 远多于登记上限的完成总量不应累积为进程生命周期内的永久额度。
        for _ in 0..512 {
            let task = owner.spawn(async {}).unwrap();
            assert!(owner.inner.tasks.lock().unwrap().len() <= 2);
            finished(&task).await;
            // AbortHandle 的完成标记可能先于 JoinSet 的完成通知可见；
            // 等待真实回收边界，仍保留上面的登记数量断言和两秒资源释放期限。
            tokio::time::timeout(Duration::from_secs(2), async {
                loop {
                    owner.reap_tasks().unwrap();
                    if owner.inner.tasks.lock().unwrap().len() == 1 {
                        break;
                    }
                    tokio::task::yield_now().await;
                }
            })
            .await
            .unwrap();
            assert!(!active.is_finished());
        }
        assert!(owner.receive().unwrap().is_none());
        assert_eq!(owner.inner.tasks.lock().unwrap().len(), 1);
        owner.close().await.unwrap();
        assert!(active.is_finished());
        assert!(owner.inner.tasks.lock().unwrap().is_empty());
        assert!(matches!(owner.spawn(async {}), Err(Error::InvalidState)));
    }
    struct Held(Arc<AtomicUsize>);
    impl Held {
        fn new(count: &Arc<AtomicUsize>) -> Self {
            count.fetch_add(1, Ordering::SeqCst);
            Self(count.clone())
        }
    }
    impl Drop for Held {
        fn drop(&mut self) {
            self.0.fetch_sub(1, Ordering::SeqCst);
        }
    }
    #[tokio::test]
    async fn registered_task_limit_includes_pending_cancellation_and_recovers() {
        let owner = session();
        let count = Arc::new(AtomicUsize::new(0));
        let mut tasks = Vec::new();
        let mut release = Vec::new();
        for _ in 0..MAX_SESSION_TASKS {
            let (sender, receiver) = oneshot::channel::<()>();
            let held = Held::new(&count);
            tasks.push(
                owner
                    .spawn(async move {
                        let _held = held;
                        let _ = receiver.await;
                    })
                    .unwrap(),
            );
            release.push(sender);
        }
        let rejected = Held::new(&count);
        let authority = owner.authority();
        let operation = authority.enter(authority.identity()).unwrap();
        assert!(matches!(
            owner.spawn(async move {
                // 模拟真实 TCP Context；拒绝时需析构同一会话的 Operation，不能重入锁死。
                let _operation = operation;
                let _held = rejected;
            }),
            Err(Error::ResourceLimit)
        ));
        assert_eq!(count.load(Ordering::SeqCst), MAX_SESSION_TASKS);
        assert_eq!(owner.inner.tasks.lock().unwrap().len(), MAX_SESSION_TASKS);
        release.remove(0).send(()).unwrap();
        finished(&tasks[0]).await;
        let replacement = owner.spawn(std::future::pending()).unwrap();
        assert_eq!(owner.inner.tasks.lock().unwrap().len(), MAX_SESSION_TASKS);
        for task in &tasks[1..] {
            assert!(!task.is_finished());
        }
        // abort 请求不等于任务完成；回收只消费真正完成项，不能提前凭请求归还名额。
        replacement.abort();
        assert_eq!(owner.inner.tasks.lock().unwrap().len(), MAX_SESSION_TASKS);
        finished(&replacement).await;
        let next = owner.spawn(std::future::pending()).unwrap();
        assert_eq!(owner.inner.tasks.lock().unwrap().len(), MAX_SESSION_TASKS);
        let (a, b) = tokio::join!(owner.close(), owner.close());
        a.unwrap();
        b.unwrap();
        assert_eq!(count.load(Ordering::SeqCst), 0);
        assert!(next.is_finished());
        assert!(owner.inner.tasks.lock().unwrap().is_empty());
    }
    #[tokio::test]
    async fn udp_flow_churn_reaps_before_registering_new_worker() {
        let manager = Arc::new(Manager::default());
        manager
            .configure_network(
                None,
                Some(Arc::new(
                    crate::routing::Router::compile(crate::routing::Config {
                        fallback: crate::routing::Policy::Direct,
                        ..crate::routing::Config::default()
                    })
                    .unwrap(),
                )),
                crate::outbound::Limits::default(),
            )
            .unwrap();
        let owner = Session::new(
            manager,
            Options {
                maximum_flows: 1,
                idle_timeout_ms: 50,
                ..Options::default()
            },
        )
        .unwrap();
        let peer = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let destination = peer.local_addr().unwrap();
        let active = owner.spawn(std::future::pending()).unwrap();
        for index in 0..48_u16 {
            let source = SocketAddr::new("192.0.2.1".parse().unwrap(), 10000 + index);
            let packet = encode(source, destination, b"ok", 1500).unwrap();
            owner.submit(&packet.bytes, None).await.unwrap();
            // 必须在 receive 之前检查，否则接收侧回收会掩盖 UDP 登记入口仍泄漏的错误。
            assert!(owner.inner.tasks.lock().unwrap().len() <= 2);
            let mut bytes = [0; 2];
            let (_, address) =
                tokio::time::timeout(Duration::from_secs(2), peer.recv_from(&mut bytes))
                    .await
                    .unwrap()
                    .unwrap();
            peer.send_to(&bytes, address).await.unwrap();
            tokio::time::timeout(Duration::from_secs(2), async {
                loop {
                    if owner.receive().unwrap().is_some() {
                        break;
                    }
                    tokio::task::yield_now().await;
                }
            })
            .await
            .unwrap();
            let task = owner
                .inner
                .state
                .lock()
                .unwrap()
                .flows
                .values()
                .next()
                .unwrap()
                .task
                .clone();
            finished(&task).await;
            assert!(!active.is_finished());
        }
        owner.close().await.unwrap();
        assert!(owner.inner.tasks.lock().unwrap().is_empty());
    }
    #[tokio::test]
    async fn completed_worker_panic_is_observed_once() {
        let owner = session();
        let (tx, rx) = std::sync::mpsc::sync_channel(64);
        crate::error::diagnostics::set_output(Some(tx)).unwrap();
        let task = owner
            .spawn(async {
                panic!("注入工作任务故障，验证受控诊断");
            })
            .unwrap();
        finished(&task).await;
        // is_finished 与 JoinSet 的完成通知发布不是同一个时刻；必须等实际 join 回收。
        // 保留两秒硬期限，不能用一次轮询未取到完成项推断诊断被吞没。
        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                assert!(owner.receive().unwrap().is_none());
                if owner.inner.tasks.lock().unwrap().is_empty() {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        let records: Vec<_> = rx.try_iter().collect();
        assert_eq!(
            records
                .iter()
                .filter(|v| v.code == crate::error::Error::Internal.code())
                .count(),
            1
        );
        assert!(owner.receive().unwrap().is_none());
        assert!(rx.try_recv().is_err());
        crate::error::diagnostics::set_output(None).unwrap();
        assert!(owner.inner.tasks.lock().unwrap().is_empty());
        owner.close().await.unwrap();
    }
}

/// 实际 UDP Socket 回环；ICMP 只调整被引用流的返回路径，不调整到上游的系统 Socket。
#[tokio::test]
async fn pmtu_udp_feedback_fragments_next_datagram_and_close_revokes_control() {
    use crate::{
        outbound::{Limits, Manager},
        routing::{Config, Policy, Router},
        tun::{
            icmp,
            udp::{Options, Session},
        },
    };
    use std::{
        sync::Arc,
        time::{Duration, Instant},
    };
    for ip in ["127.0.0.1", "::1"] {
        let socket = tokio::net::UdpSocket::bind(std::net::SocketAddr::new(ip.parse().unwrap(), 0))
            .await
            .unwrap();
        let server = socket.local_addr().unwrap();
        let client: std::net::SocketAddr = if server.is_ipv4() {
            "192.0.2.1:12345"
        } else {
            "[2001:db8::1]:12345"
        }
        .parse()
        .unwrap();
        let manager = Arc::new(Manager::default());
        manager
            .configure_network(
                None,
                Some(Arc::new(
                    Router::compile(Config {
                        fallback: Policy::Direct,
                        ..Config::default()
                    })
                    .unwrap(),
                )),
                Limits::default(),
            )
            .unwrap();
        let session = Session::new(manager, Options::default()).unwrap();
        let request = crate::tun::packet::encode_udp(client, server, b"hello", 1500).unwrap();
        session.submit(&request.bytes, None).await.unwrap();
        let mut b = [0; 32];
        let (_, peer) = socket.recv_from(&mut b).await.unwrap();
        socket.send_to(&[7; 1400], peer).await.unwrap();
        let next = async {
            loop {
                if let Some(p) = session.receive().unwrap() {
                    break p;
                }
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
        };
        let first = tokio::time::timeout(Duration::from_secs(2), next)
            .await
            .unwrap();
        let quote = icmp::quote(&first.bytes).unwrap();
        // 通过与宿主会话相同的 owner 入口验证 UDP 近期引用；解析器另有独立 ICMP 向量。
        let inner = &session;
        let notice = icmp::Notice {
            quote,
            kind: icmp::Kind::TooBig(if server.is_ipv4() { 576 } else { 1280 }),
        };
        inner.control(notice, None).unwrap();
        session.submit(&request.bytes, None).await.unwrap();
        socket.recv_from(&mut b).await.unwrap();
        socket.send_to(&[8; 1400], peer).await.unwrap();
        let mut reassembly = crate::tun::ip::Reassembly::default();
        let deadline = Instant::now() + Duration::from_secs(2);
        let mut fragments = 0;
        loop {
            assert!(Instant::now() < deadline);
            if let Some(packet) = session.receive().unwrap() {
                fragments += 1;
                assert!(packet.bytes.len() <= if server.is_ipv4() { 576 } else { 1280 });
                if let Some(full) = reassembly
                    .submit(&packet.bytes, 1500, None, Instant::now())
                    .unwrap()
                {
                    let bytes = full.bytes();
                    assert_eq!(&bytes[if server.is_ipv4() { 28 } else { 48 }..], &[8; 1400]);
                    break;
                }
            } else {
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
        }
        assert_eq!(fragments, if server.is_ipv4() { 3 } else { 2 });
        session.close().await.unwrap();
        assert!(inner.control(notice, None).is_err());
    }
}
