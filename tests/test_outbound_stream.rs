//! 测试文件：tests/test_outbound_stream.rs
//! 测试类型：单元测试、迁移回归
//! 测试对象：src/outbound/stream.rs
//! 测试内容：
//! - retained_stream_does_not_own_socket_after_session_close：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//!
//! 测试依据：保留原测试的业务/格式/授权/资源约束；《测试文件与报告统一管理原则》，独立语义审查不以实现输出反推预期。
//!
//! 前置条件：本地测试需文件/回环套接字权限；平台限定及忽略项以实际登记为准。
//!
//! 执行状态：由同名 Markdown 报告记录，不在源码中固化通过结论。

#[cfg(test)]
mod lifecycle_tests {
    use super::super::*;
    /// 外部持有流对象与旧快照，关闭仍应释放真实 TCP 并禁止恢复读写。
    #[tokio::test]
    async fn retained_stream_does_not_own_socket_after_session_close() {
        use crate::{
            dns::{Config, Resolver, Server, fake_ip},
            routing::{Config as Routes, Mode, Router},
            tun::udp::{Options, Session},
        };
        use tokio::{
            io::{AsyncReadExt, AsyncWriteExt},
            net::TcpListener,
            time::{Duration, timeout},
        };
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let manager = Arc::new(Manager::default());
        let resolver = Arc::new(
            Resolver::new(Config {
                servers: vec![Server::Udp {
                    address: "127.0.0.1:5353".into(),
                }],
                timeout_ms: 100,
                dnssec: false,
                service: None,
                fake_ip: Some(fake_ip::Config {
                    ipv4_range: "198.19.96.0/24".into(),
                    ipv6_range: "fd00:cafe:198::/64".into(),
                    capacity: 2,
                    exclude: vec![],
                }),
            })
            .unwrap(),
        );
        manager
            .configure_network(
                Some(resolver.clone()),
                Some(Arc::new(
                    Router::compile(Routes {
                        mode: Mode::Direct,
                        ..Routes::default()
                    })
                    .unwrap(),
                )),
                crate::outbound::Limits::default(),
            )
            .unwrap();
        let options = Options {
            dns_endpoints: vec!["192.0.2.53:53".parse().unwrap()],
            ..Options::default()
        };
        let session = Session::new(manager.clone(), options.clone()).unwrap();
        let snapshot = manager.network_snapshot().unwrap();
        let mut stream = manager
            .connect_stream("127.0.0.1", listener.local_addr().unwrap().port(), None)
            .await
            .unwrap();
        let (mut remote, _) = listener.accept().await.unwrap();
        stream.write_all(b"x").await.unwrap();
        let mut bytes = [0];
        assert_eq!(remote.read(&mut bytes).await.unwrap(), 1);
        timeout(Duration::from_secs(2), session.close())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            timeout(Duration::from_secs(2), remote.read(&mut bytes))
                .await
                .unwrap()
                .unwrap(),
            0
        );
        assert!(stream.write_all(b"revoked").await.is_err());
        assert!(snapshot.resolver.as_ref().unwrap().clear().is_err());
        let next = Session::new(manager.clone(), options).unwrap();
        next.close().await.unwrap();
        assert!(session.receive().is_err());
    }
}
