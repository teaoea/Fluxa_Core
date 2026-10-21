//! 测试文件：tests/test_fake_ip_lifecycle.rs
//! 测试类型：集成测试、回归测试
//! 测试对象：src/dns/fake_ip.rs、src/tun/udp.rs
//! 测试内容：
//! - retired_fake_ip_range_can_expand_and_reopen_tun：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//!
//! 测试依据：保留原有公开接口契约及全部断言；测试文件原注释保留在下方，L3 必须独立核对。
//!
//! 前置条件：本地测试需文件/回环套接字权限；平台限定及忽略项以实际登记为准。
//!
//! 执行状态：由同名 Markdown 报告记录，不在源码中固化通过结论。

// H1 独立进程回归：避免其他测试的存活重叠池改变本案例前置条件。
use fluxa_core::{
    dns::{Config, Resolver, Server, fake_ip},
    outbound::{Limits, Manager},
    tun::udp::{Options, Session},
};
use std::sync::Arc;
/// H1 最小复现：释放旧解析器与会话后，扩大重叠区间仍能创建会话；旧地址不可重新归属。
#[tokio::test]
async fn retired_fake_ip_range_can_expand_and_reopen_tun() {
    let make = |range: &str| {
        Arc::new(
            Resolver::new(Config {
                servers: vec![Server::Udp {
                    address: "127.0.0.1:5353".into(),
                }],
                timeout_ms: 100,
                dnssec: false,
                service: None,
                fake_ip: Some(fake_ip::Config {
                    ipv4_range: range.into(),
                    ipv6_range: "fd00:198:ffff::/64".into(),
                    capacity: 2,
                    exclude: vec![],
                }),
            })
            .unwrap(),
        )
    };
    let old = make("198.18.0.0/16");
    let setup_h1 = |dns: Arc<Resolver>| {
        let manager = Arc::new(Manager::default());
        manager
            .configure_network(Some(dns), None, Limits::default())
            .unwrap();
        let session = Session::new(
            manager.clone(),
            Options {
                dns_endpoints: vec!["192.0.2.53:53".parse().unwrap()],
                ..Options::default()
            },
        )
        .unwrap();
        (manager, session)
    };
    let (manager, session) = setup_h1(old.clone());
    let old_ip = old
        .allocate_fake_ip(fluxa_core::dns::Query {
            name: "retired-h1.test".into(),
            query_type: "A".into(),
        })
        .unwrap()
        .addresses[0]
        .clone();
    session.close().await.unwrap();
    // 关闭后故意长期保留所有旧引用；引用计数不能决定活动资源准入。
    let retained_snapshot = manager.network_snapshot().unwrap();
    assert!(matches!(
        old.allocate_fake_ip(fluxa_core::dns::Query {
            name: "stale.test".into(),
            query_type: "A".into()
        }),
        Err(fluxa_core::error::Error::InvalidState)
    ));
    assert!(matches!(
        retained_snapshot
            .resolver
            .as_ref()
            .unwrap()
            .reverse_fake_ip(&old_ip),
        Err(fluxa_core::error::Error::InvalidState)
    ));
    assert!(matches!(
        session.receive(),
        Err(fluxa_core::error::Error::InvalidState)
    ));
    // 同一 Manager 从只读元数据创建新解析器，调用方不负责恢复旧对象。
    let renewed = Session::new(
        manager.clone(),
        Options {
            dns_endpoints: vec!["192.0.2.53:53".parse().unwrap()],
            ..Options::default()
        },
    )
    .unwrap();
    assert_ne!(
        manager
            .network_snapshot()
            .unwrap()
            .resolver
            .as_ref()
            .unwrap()
            .fake_ip_generation(),
        old.fake_ip_generation()
    );
    renewed.close().await.unwrap();
    let new = make("198.18.0.0/15");
    let (_manager, session) = setup_h1(new.clone());
    let current = new
        .allocate_fake_ip(fluxa_core::dns::Query {
            name: "current-h1.test".into(),
            query_type: "A".into(),
        })
        .unwrap()
        .addresses[0]
        .clone();
    assert_ne!(old_ip, current);
    assert!(new.reverse_fake_ip(&old_ip).unwrap().is_none());
    session.close().await.unwrap();
    // 原进程永久历史名额和对象持有的活动配额均不能随关闭次数单调耗尽。
    let mut retained = Vec::new();
    for index in 0..40 {
        let resolver = make(if index % 2 == 0 {
            "198.18.0.0/16"
        } else {
            "198.18.0.0/15"
        });
        let (manager, session) = setup_h1(resolver.clone());
        let session = Arc::new(session);
        let snapshot = manager.network_snapshot().unwrap();
        if index == 0 {
            // 已发出关闭意图后取消调用方等待，内核清理仍必须继续。
            use std::future::Future;
            let mut closing = Box::pin(session.close());
            std::future::poll_fn(|context| {
                let _ = closing.as_mut().poll(context);
                std::task::Poll::Ready(())
            })
            .await;
            drop(closing);
        }
        let (a, b) = tokio::join!(session.close(), session.close());
        a.unwrap();
        b.unwrap();
        assert!(matches!(
            session.receive(),
            Err(fluxa_core::error::Error::InvalidState)
        ));
        retained.push((resolver, manager, session, snapshot));
    }
    assert_eq!(retained.len(), 40);
    listeners_and_pending_query_close_with_retained_objects().await;
    closed_metadata_stays_budgeted_until_actual_drop().await;
}

/// 已绑定的真实 TCP/UDP 监听和正在等待上游的查询，由内核关闭并等待，不依赖 Service/Resolver 析构。
async fn listeners_and_pending_query_close_with_retained_objects() {
    use fluxa_core::dns::{Query, service};
    use tokio::{
        net::{TcpListener, TcpStream, UdpSocket},
        time::{Duration, timeout},
    };
    let upstream = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let resolver = Arc::new(
        Resolver::new(Config {
            servers: vec![Server::Udp {
                address: upstream.local_addr().unwrap().to_string(),
            }],
            timeout_ms: 5000,
            dnssec: false,
            fake_ip: None,
            service: Some(service::Config {
                listen: "127.0.0.1:0".into(),
                allow_lan: false,
                allowed_clients: vec![],
            }),
        })
        .unwrap(),
    );
    let mut service = resolver.clone().serve().await.unwrap();
    let port = service.status().port;
    let _idle_client = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
    let queried = resolver.clone();
    let query = tokio::spawn(async move {
        queried
            .resolve_real(Query {
                name: "pending.test".into(),
                query_type: "A".into(),
            })
            .await
    });
    let mut packet = [0u8; 4096];
    timeout(Duration::from_secs(2), upstream.recv_from(&mut packet))
        .await
        .unwrap()
        .unwrap();
    timeout(Duration::from_secs(2), resolver.close())
        .await
        .unwrap()
        .unwrap();
    assert!(query.await.unwrap().is_err());
    assert!(!service.status().running);
    let _tcp = TcpListener::bind(("127.0.0.1", port)).await.unwrap();
    let _udp = UdpSocket::bind(("127.0.0.1", port)).await.unwrap();
    service.stop().await.unwrap();
    assert!(resolver.clone().serve().await.is_err());
}

/// 活动配额关闭即回收，但实际保留的配置内存不能借逻辑关闭绕过预算。
async fn closed_metadata_stays_budgeted_until_actual_drop() {
    let make = || {
        let mut address = String::with_capacity(8 * 1024 * 1024);
        address.push_str("127.0.0.1:5353");
        Resolver::new(Config {
            servers: vec![Server::Udp { address }],
            timeout_ms: 100,
            dnssec: false,
            fake_ip: None,
            service: None,
        })
    };
    let retained = Arc::new(make().unwrap());
    retained.close().await.unwrap();
    assert!(matches!(
        make(),
        Err(fluxa_core::error::Error::ResourceBudgetExceeded { .. })
    ));
    drop(retained);
    let replacement = Arc::new(make().unwrap());
    replacement.close().await.unwrap();
}
