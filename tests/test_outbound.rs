//! 测试文件：tests/test_outbound.rs
//! 测试类型：单元测试、迁移回归
//! 测试对象：src/outbound.rs
//! 测试内容：
//! - tcp_duplex_half_close_and_eof_use_real_peer：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - udp_keeps_datagram_boundaries_zero_length_and_rejects_truncation：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - read_timeout_recovers_and_close_cancels_blocked_io：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - policy_and_invalid_targets_do_not_connect：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - session_limit_reclaims_resources_and_never_reuses_handles：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - ipv6_tcp_and_udp_reach_real_loopback_peers：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - tcp_write_timeout_invalidates_connection_to_prevent_duplicate_retry：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - latency_probe_closes_connection_without_application_data：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//!
//! 测试依据：保留原测试的业务/格式/授权/资源约束；《测试文件与报告统一管理原则》，独立语义审查不以实现输出反推预期。
//!
//! 前置条件：本地测试需文件/回环套接字权限；平台限定及忽略项以实际登记为准。
//!
//! 执行状态：由同名 Markdown 报告记录，不在源码中固化通过结论。

// 以下保留迁移前的用例说明；历史执行状态不代表当前版本，当前结果仅见同名报告。
mod public_contract {
    use nodus_core::{
        error::Error,
        outbound::{MAX_BUFFER, Manager, endpoint},
        routing::{Policy, Transport},
    };
    use std::{sync::Arc, time::Duration};
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::{TcpListener, UdpSocket},
        time::{sleep, timeout},
    };

    #[tokio::test]
    async fn tcp_duplex_half_close_and_eof_use_real_peer() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = [0; 4];
            socket.read_exact(&mut request).await.unwrap();
            assert_eq!(request, [0, 1, 2, 255]);
            socket.write_all(b"reply").await.unwrap();
            let mut remaining = Vec::new();
            socket.read_to_end(&mut remaining).await.unwrap();
            assert!(remaining.is_empty());
            socket.shutdown().await.unwrap();
        });
        let manager = Arc::new(Manager::default());
        let id = manager
            .open(address, Transport::Tcp, Policy::Direct, 1000)
            .await
            .unwrap();
        let reader = Arc::clone(&manager);
        let pending = tokio::spawn(async move { reader.read(id, 64, 1000).await });
        sleep(Duration::from_millis(10)).await;
        assert_eq!(manager.write(id, &[0, 1, 2, 255], 1000).await.unwrap(), 4);
        let read = pending.await.unwrap().unwrap();
        assert_eq!(&*read.bytes, b"reply");
        assert!(!read.eof);
        manager.shutdown_write(id, 1000).await.unwrap();
        let eof = manager.read(id, 64, 1000).await.unwrap();
        assert!(eof.eof && eof.bytes.is_empty());
        manager.close(id).await.unwrap();
        assert!(matches!(
            manager.read(id, 64, 1000).await,
            Err(Error::InvalidState)
        ));
        server.await.unwrap();
    }

    #[tokio::test]
    async fn udp_keeps_datagram_boundaries_zero_length_and_rejects_truncation() {
        let socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let address = socket.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let mut buffer = [0; 64];
            for expected in [b"request".as_slice(), b"", b"large", b"again"] {
                let (size, peer) = socket.recv_from(&mut buffer).await.unwrap();
                assert_eq!(&buffer[..size], expected);
                socket
                    .send_to(
                        if expected == b"large" {
                            b"too-large"
                        } else {
                            expected
                        },
                        peer,
                    )
                    .await
                    .unwrap();
            }
        });
        let manager = Manager::default();
        let id = manager
            .open(address, Transport::Udp, Policy::Direct, 1000)
            .await
            .unwrap();
        for data in [b"request".as_slice(), b""] {
            manager.write(id, data, 1000).await.unwrap();
            let read = manager.read(id, 64, 1000).await.unwrap();
            assert_eq!(&*read.bytes, data);
            assert!(!read.eof);
        }
        manager.write(id, b"large", 1000).await.unwrap();
        assert!(matches!(
            manager.read(id, 2, 1000).await,
            Err(Error::ResourceLimit)
        ));
        manager.write(id, b"again", 1000).await.unwrap();
        assert_eq!(&*manager.read(id, 64, 1000).await.unwrap().bytes, b"again");
        assert_eq!(
            manager.shutdown_write(id, 1000).await,
            Err(Error::Unsupported)
        );
        manager.close(id).await.unwrap();
        server.await.unwrap();
    }

    #[tokio::test]
    async fn read_timeout_recovers_and_close_cancels_blocked_io() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let manager = Arc::new(Manager::default());
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
            manager.read(id, 64, 10).await,
            Err(Error::NetworkTimeout)
        ));
        peer.write_all(b"after-timeout").await.unwrap();
        assert_eq!(
            &*manager.read(id, 64, 1000).await.unwrap().bytes,
            b"after-timeout"
        );
        let reader = Arc::clone(&manager);
        let pending = tokio::spawn(async move { reader.read(id, 64, 60000).await });
        sleep(Duration::from_millis(10)).await;
        manager.close(id).await.unwrap();
        assert!(matches!(
            timeout(Duration::from_millis(500), pending)
                .await
                .unwrap()
                .unwrap(),
            Err(Error::Cancelled)
        ));
        assert_eq!(peer.read(&mut [0; 1]).await.unwrap(), 0);
    }

    #[tokio::test]
    async fn policy_and_invalid_targets_do_not_connect() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let manager = Manager::default();
        assert_eq!(
            manager
                .open(address, Transport::Tcp, Policy::Proxy, 1000)
                .await,
            Err(Error::Unsupported)
        );
        assert_eq!(
            manager
                .open(address, Transport::Tcp, Policy::Reject, 1000)
                .await,
            Err(Error::Rejected)
        );
        assert!(
            timeout(Duration::from_millis(30), listener.accept())
                .await
                .is_err()
        );
        for invalid in [
            "example.com:443",
            "127.0.0.1:0",
            "0.0.0.0:80",
            "[::]:80",
            "127.0.0.1",
            "[::1]:65536",
        ] {
            assert!(endpoint(invalid).is_err());
        }
        for duration in [0, 60001, u64::MAX] {
            assert_eq!(
                manager
                    .open(address, Transport::Udp, Policy::Direct, duration)
                    .await,
                Err(Error::InvalidInput)
            );
        }
        let id = manager
            .open(address, Transport::Udp, Policy::Direct, 1000)
            .await
            .unwrap();
        assert!(matches!(
            manager.read(id, 0, 1000).await,
            Err(Error::InvalidInput)
        ));
        assert!(matches!(
            manager.read(id, MAX_BUFFER + 1, 1000).await,
            Err(Error::InvalidInput)
        ));
        assert_eq!(
            manager.write(id, &vec![0; MAX_BUFFER + 1], 1000).await,
            Err(Error::ResourceLimit)
        );
        manager.stop().await.unwrap();
        assert_eq!(
            manager
                .open(address, Transport::Udp, Policy::Direct, 1000)
                .await,
            Err(Error::InvalidState)
        );
    }

    #[tokio::test]
    async fn session_limit_reclaims_resources_and_never_reuses_handles() {
        let manager = Manager::default();
        let address = endpoint("127.0.0.1:9").unwrap();
        let mut ids = Vec::new();
        for _ in 0..64 {
            ids.push(
                manager
                    .open(address, Transport::Udp, Policy::Direct, 1000)
                    .await
                    .unwrap(),
            );
        }
        assert_eq!(
            manager
                .open(address, Transport::Udp, Policy::Direct, 1000)
                .await,
            Err(Error::ResourceLimit)
        );
        manager.close(ids[0]).await.unwrap();
        let newer = manager
            .open(address, Transport::Udp, Policy::Direct, 1000)
            .await
            .unwrap();
        assert!(newer > ids[63]);
        assert_eq!(manager.close(ids[0]).await, Err(Error::InvalidState));
        manager.close_all().await.unwrap();
        manager.close_all().await.unwrap();
        for _ in 0..100 {
            let id = manager
                .open(address, Transport::Udp, Policy::Direct, 1000)
                .await
                .unwrap();
            manager.close(id).await.unwrap();
        }
    }

    #[tokio::test]
    async fn ipv6_tcp_and_udp_reach_real_loopback_peers() {
        let tcp = TcpListener::bind("[::1]:0").await.unwrap();
        let udp = UdpSocket::bind("[::1]:0").await.unwrap();
        let manager = Manager::default();
        let tcp_id = manager
            .open(
                tcp.local_addr().unwrap(),
                Transport::Tcp,
                Policy::Direct,
                1000,
            )
            .await
            .unwrap();
        let (mut peer, _) = tcp.accept().await.unwrap();
        peer.write_all(b"ipv6").await.unwrap();
        assert_eq!(
            &*manager.read(tcp_id, 64, 1000).await.unwrap().bytes,
            b"ipv6"
        );
        let udp_id = manager
            .open(
                udp.local_addr().unwrap(),
                Transport::Udp,
                Policy::Direct,
                1000,
            )
            .await
            .unwrap();
        manager.write(udp_id, b"ipv6-udp", 1000).await.unwrap();
        let mut buffer = [0; 64];
        let (size, destination) = udp.recv_from(&mut buffer).await.unwrap();
        udp.send_to(&buffer[..size], destination).await.unwrap();
        assert_eq!(
            &*manager.read(udp_id, 64, 1000).await.unwrap().bytes,
            b"ipv6-udp"
        );
        manager.close_all().await.unwrap();
    }

    #[tokio::test]
    async fn tcp_write_timeout_invalidates_connection_to_prevent_duplicate_retry() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let manager = Manager::default();
        let id = manager
            .open(
                listener.local_addr().unwrap(),
                Transport::Tcp,
                Policy::Direct,
                1000,
            )
            .await
            .unwrap();
        let (_peer, _) = listener.accept().await.unwrap();
        let bytes = vec![42; MAX_BUFFER];
        let mut timed_out = false;
        for _ in 0..512 {
            match manager.write(id, &bytes, 10).await {
                Ok(_) => {}
                Err(Error::NetworkTimeout) => {
                    timed_out = true;
                    break;
                }
                error => panic!("应写入或超时：{error:?}"),
            }
        }
        assert!(timed_out);
        assert_eq!(
            manager.write(id, b"retry", 1000).await,
            Err(Error::Cancelled)
        );
        manager.close(id).await.unwrap();
    }

    #[tokio::test]
    async fn latency_probe_closes_connection_without_application_data() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buffer = [0; 1];
            assert_eq!(
                timeout(Duration::from_secs(2), socket.read(&mut buffer))
                    .await
                    .unwrap()
                    .unwrap(),
                0
            );
        });
        let manager = Manager::default();
        manager
            .probe_latency("127.0.0.1", address.port(), 1000)
            .await
            .unwrap();
        assert!(matches!(
            manager.probe_latency("127.0.0.1", 0, 1000).await,
            Err(Error::InvalidInput)
        ));
        server.await.unwrap();
    }
}
