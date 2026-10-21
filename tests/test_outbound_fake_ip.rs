//! 测试文件：tests/test_outbound_fake_ip.rs
//! 测试类型：集成测试、回归测试
//! 测试对象：src/dns/mod.rs、src/routing/runtime.rs
//! 测试内容：
//! - both_fake_families_restore_domain_and_use_real_dns_cache：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - reset_and_reconfiguration_reject_old_tokens_but_keep_existing_connections：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - unknown_mapping_and_rejected_policies_do_not_send_dns_or_connect：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - fake_mapping_does_not_bypass_dnssec_validation：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//!
//! 测试依据：保留原有公开接口契约及全部断言；测试文件原注释保留在下方，L3 必须独立核对。
//!
//! 前置条件：本地测试需文件/回环套接字权限；平台限定及忽略项以实际登记为准。
//!
//! 执行状态：由同名 Markdown 报告记录，不在源码中固化通过结论。

#[path = "test_fixture_dns.rs"]
mod support;
use fluxa_core::{
    dns::{Config, Query, Resolver, Server, fake_ip},
    error::Error,
    outbound::{FakeIPTarget, Family, Manager},
    routing::{Policy, Transport},
};
use std::{sync::atomic::Ordering, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};
fn config(port: u16) -> Config {
    Config {
        servers: vec![Server::Udp {
            address: format!("127.0.0.1:{port}"),
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
    }
}
async fn target(resolver: &Resolver, port: u16, ipv6: bool) -> FakeIPTarget {
    let answer = resolver
        .resolve(Query {
            name: "connect.example.test".into(),
            query_type: if ipv6 { "AAAA" } else { "A" }.into(),
        })
        .await
        .unwrap();
    assert_eq!(answer.validation, "synthetic");
    let address = &answer.addresses[0];
    FakeIPTarget {
        address: if ipv6 {
            format!("[{address}]:{port}")
        } else {
            format!("{address}:{port}")
        },
        mapping_generation: resolver.fake_ip_generation().unwrap(),
        transport: Transport::Tcp,
        policy: Policy::Direct,
        timeout_ms: 1000,
        family: Family::Ipv4,
    }
}
#[tokio::test]
async fn both_fake_families_restore_domain_and_use_real_dns_cache() {
    let fixture = support::fixture().await;
    assert!(
        !fixture.pem.is_empty()
            && !fixture.root.is_empty()
            && fixture.dot_port > 0
            && fixture.doh_port > 0
    );
    let resolver = Resolver::new(config(fixture.port)).unwrap();
    let manager = Manager::default();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    for ipv6 in [false, true] {
        let request = target(&resolver, port, ipv6).await;
        let opened = manager.connect_fake_ip(&resolver, request).await.unwrap();
        assert_eq!(opened.address, format!("127.0.0.1:{port}"));
        let (mut peer, _) = listener.accept().await.unwrap();
        manager.write(opened.id, b"x", 1000).await.unwrap();
        let mut byte = [0];
        peer.read_exact(&mut byte).await.unwrap();
        peer.write_all(&byte).await.unwrap();
        assert_eq!(
            &*manager.read(opened.id, 1, 1000).await.unwrap().bytes,
            b"x"
        );
        manager.close(opened.id).await.unwrap();
    }
    assert_eq!(fixture.count.load(Ordering::SeqCst), 1);
}
#[tokio::test]
async fn reset_and_reconfiguration_reject_old_tokens_but_keep_existing_connections() {
    let fixture = support::fixture().await;
    let settings = config(fixture.port);
    let resolver = Resolver::new(settings.clone()).unwrap();
    let manager = Manager::default();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let original = target(&resolver, port, false).await;
    let opened = manager
        .connect_fake_ip(&resolver, original.clone())
        .await
        .unwrap();
    let (mut peer, _) = listener.accept().await.unwrap();
    resolver.reset_fake_ip().unwrap();
    assert!(matches!(
        manager.connect_fake_ip(&resolver, original.clone()).await,
        Err(Error::InvalidState)
    ));
    peer.write_all(b"live").await.unwrap();
    assert_eq!(
        &*manager.read(opened.id, 4, 1000).await.unwrap().bytes,
        b"live"
    );
    manager.close(opened.id).await.unwrap();
    let next = target(&resolver, port, false).await;
    assert_ne!(next.address, original.address);
    let replacement = Resolver::new(settings).unwrap();
    let replaced = target(&replacement, port, false).await;
    assert_eq!(replaced.address, original.address);
    assert_ne!(replaced.mapping_generation, original.mapping_generation);
    assert!(matches!(
        manager.connect_fake_ip(&replacement, original).await,
        Err(Error::InvalidState)
    ));
    let id = manager
        .connect_fake_ip(&replacement, replaced)
        .await
        .unwrap()
        .id;
    let _peer = listener.accept().await.unwrap();
    manager.close(id).await.unwrap();
}
#[tokio::test]
async fn unknown_mapping_and_rejected_policies_do_not_send_dns_or_connect() {
    let fixture = support::fixture().await;
    let resolver = Resolver::new(config(fixture.port)).unwrap();
    let manager = Manager::default();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let mut request = target(&resolver, port, false).await;
    for policy in [Policy::Reject, Policy::Proxy] {
        request.policy = policy;
        assert!(
            manager
                .connect_fake_ip(&resolver, request.clone())
                .await
                .is_err()
        );
    }
    request.policy = Policy::Direct;
    request.address = format!("198.18.0.99:{port}");
    assert!(matches!(
        manager.connect_fake_ip(&resolver, request.clone()).await,
        Err(Error::InvalidState)
    ));
    request.address = format!("127.0.0.1:{port}");
    assert!(matches!(
        manager.connect_fake_ip(&resolver, request).await,
        Err(Error::InvalidState)
    ));
    assert_eq!(fixture.count.load(Ordering::SeqCst), 0);
    assert!(
        tokio::time::timeout(Duration::from_millis(30), listener.accept())
            .await
            .is_err()
    );
}
#[path = "test_fixture_dnssec.rs"]
mod signed_support;
#[tokio::test]
async fn fake_mapping_does_not_bypass_dnssec_validation() {
    let fixture = signed_support::fixture().await;
    let mut settings = config(fixture.port);
    settings.dnssec = true;
    let resolver = Resolver::with_trust_anchors(settings, fixture.anchors.clone()).unwrap();
    let manager = Manager::default();
    let answer = resolver
        .resolve(Query {
            name: "bogus.".into(),
            query_type: "A".into(),
        })
        .await
        .unwrap();
    let request = FakeIPTarget {
        address: format!("{}:443", answer.addresses[0]),
        mapping_generation: resolver.fake_ip_generation().unwrap(),
        transport: Transport::Tcp,
        policy: Policy::Direct,
        timeout_ms: 1000,
        family: Family::Ipv4,
    };
    assert!(matches!(
        manager.connect_fake_ip(&resolver, request).await,
        Err(Error::DnsValidation)
    ));
}
