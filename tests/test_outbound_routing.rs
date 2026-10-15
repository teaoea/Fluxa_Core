//! 测试文件：tests/test_outbound_routing.rs
//! 测试类型：集成测试、回归测试
//! 测试对象：src/dns/mod.rs、src/routing/runtime.rs
//! 测试内容：
//! - known_rejection_proxy_and_missing_dns_never_connect_or_fallback：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - domain_match_skips_ip_rejection_in_both_orders：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - numeric_udp_skips_dns_and_direct_mode_preserves_rejection：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - mixed_address_policies_reject_before_any_candidate_is_dialed：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - fake_ip_route_matches_restored_domain_and_real_cidr_with_generation：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - fake_preallocation_exclusion_does_not_touch_dns：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - routed_dns_deadline_and_close_cancel_before_dial：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - formal_scoped_udp_reject_does_not_break_tcp_or_silently_send_udp：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//!
//! 测试依据：保留原有公开接口契约及全部断言；测试文件原注释保留在下方，L3 必须独立核对。
//!
//! 前置条件：本地测试需文件/回环套接字权限；平台限定及忽略项以实际登记为准。
//!
//! 执行状态：由同名 Markdown 报告记录，不在源码中固化通过结论。

#[path = "test_fixture_dns.rs"]
mod support;
use libfluxa_core::{
    dns::{Config as DNSConfig, Query, Resolver, Server, fake_ip},
    error::Error,
    outbound::{Family, Manager, RoutedTarget},
    routing::{Config, Match, Mode, Policy, Reason, Router, Rule, Target, Transport},
};
use std::{sync::atomic::Ordering, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, UdpSocket},
};
fn dns(port: u16) -> DNSConfig {
    DNSConfig {
        servers: vec![Server::Udp {
            address: format!("127.0.0.1:{port}"),
        }],
        timeout_ms: 1000,
        dnssec: false,
        fake_ip: None,
        service: None,
    }
}
fn rule(kind: Match, value: &str, policy: Policy) -> Rule {
    Rule {
        protocol: None,
        r#type: kind,
        value: value.into(),
        policy,
        outbound: (policy == Policy::Proxy).then(|| "unimplemented".into()),
    }
}
fn router(rules: Vec<Rule>) -> Router {
    Router::compile(Config {
        rules,
        ..Config::default()
    })
    .unwrap()
}
fn target(port: u16) -> RoutedTarget {
    RoutedTarget {
        target: Target {
            domain: Some("connect.example.test".into()),
            address: None,
            port,
            transport: Transport::Tcp,
        },
        timeout_ms: 1000,
        family: Family::Ipv4,
        mapping_generation: None,
    }
}
#[tokio::test]
async fn known_rejection_proxy_and_missing_dns_never_connect_or_fallback() {
    let fixture = support::fixture().await;
    assert!(
        !fixture.pem.is_empty()
            && !fixture.root.is_empty()
            && fixture.dot_port > 0
            && fixture.doh_port > 0
    );
    let resolver = Resolver::new(dns(fixture.port)).unwrap();
    let manager = Manager::default();
    for policy in [Policy::Reject, Policy::Proxy] {
        let router = router(vec![rule(Match::Domain, "connect.example.test", policy)]);
        assert!(
            manager
                .connect_routed(&router, Some(&resolver), target(443))
                .await
                .is_err()
        );
    }
    let global = Router::compile(Config {
        mode: Mode::Global,
        outbound: Some("unimplemented".into()),
        ..Config::default()
    })
    .unwrap();
    assert!(matches!(
        manager.connect_routed(&global, None, target(443)).await,
        Err(Error::InvalidState)
    ));
    let direct = router(vec![rule(
        Match::Domain,
        "connect.example.test",
        Policy::Direct,
    )]);
    assert!(matches!(
        manager.connect_routed(&direct, None, target(443)).await,
        Err(Error::InvalidState)
    ));
    assert_eq!(fixture.count.load(Ordering::SeqCst), 0);
}
#[tokio::test]
async fn domain_match_skips_ip_rejection_in_both_orders() {
    let fixture = support::fixture().await;
    let resolver = Resolver::new(dns(fixture.port)).unwrap();
    let manager = Manager::default();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let deny = rule(Match::IpCidr, "127.0.0.0/8", Policy::Reject);
    let allow = rule(Match::Domain, "connect.example.test", Policy::Direct);
    for rules in [vec![deny.clone(), allow.clone()], vec![allow.clone(), deny]] {
        let opened = manager
            .connect_routed(&router(rules), Some(&resolver), target(port))
            .await
            .unwrap();
        let _peer = listener.accept().await.unwrap();
        assert_eq!(opened.decision.policy, Policy::Direct);
        manager.close(opened.connection.id).await.unwrap();
    }
    let opened = manager
        .connect_routed(&router(vec![allow]), Some(&resolver), target(port))
        .await
        .unwrap();
    assert_eq!(opened.decision.rule_index, Some(0));
    let (mut peer, _) = listener.accept().await.unwrap();
    manager
        .write(opened.connection.id, b"x", 1000)
        .await
        .unwrap();
    let mut byte = [0];
    peer.read_exact(&mut byte).await.unwrap();
    peer.write_all(&byte).await.unwrap();
    assert_eq!(
        &*manager
            .read(opened.connection.id, 1, 1000)
            .await
            .unwrap()
            .bytes,
        b"x"
    );
    manager.close(opened.connection.id).await.unwrap();
}
#[tokio::test]
async fn numeric_udp_skips_dns_and_direct_mode_preserves_rejection() {
    let socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let address = socket.local_addr().unwrap();
    let manager = Manager::default();
    let router = Router::compile(Config {
        mode: Mode::Direct,
        rules: vec![rule(Match::IpCidr, "127.0.0.0/8", Policy::Reject)],
        ..Config::default()
    })
    .unwrap();
    let mut request = target(address.port());
    request.target.domain = None;
    request.target.address = Some(address.ip().to_string());
    request.target.transport = Transport::Udp;
    assert!(matches!(
        manager.connect_routed(&router, None, request.clone()).await,
        Err(Error::Rejected)
    ));
    let router = Router::compile(Config {
        mode: Mode::Direct,
        ..Config::default()
    })
    .unwrap();
    let opened = manager
        .connect_routed(&router, None, request)
        .await
        .unwrap();
    assert_eq!(opened.decision.reason, Reason::Mode);
    manager
        .write(opened.connection.id, b"udp", 1000)
        .await
        .unwrap();
    let mut bytes = [0; 3];
    let (size, peer) = socket.recv_from(&mut bytes).await.unwrap();
    assert_eq!(size, 3);
    socket.send_to(&bytes, peer).await.unwrap();
    assert_eq!(
        &*manager
            .read(opened.connection.id, 3, 1000)
            .await
            .unwrap()
            .bytes,
        b"udp"
    );
    manager.close(opened.connection.id).await.unwrap();
}
#[tokio::test]
async fn mixed_address_policies_reject_before_any_candidate_is_dialed() {
    let fixture = support::fixture().await;
    let resolver = Resolver::new(dns(fixture.port)).unwrap();
    let manager = Manager::default();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let router = Router::compile(Config {
        fallback: Policy::Direct,
        rules: vec![rule(Match::IpCidr, "::1/128", Policy::Reject)],
        ..Config::default()
    })
    .unwrap();
    let mut request = target(listener.local_addr().unwrap().port());
    request.family = Family::Auto;
    assert!(matches!(
        manager
            .connect_routed(&router, Some(&resolver), request)
            .await,
        Err(Error::Rejected)
    ));
    assert_eq!(fixture.count.load(Ordering::SeqCst), 2);
    assert!(
        tokio::time::timeout(Duration::from_millis(20), listener.accept())
            .await
            .is_err()
    );
}
#[tokio::test]
async fn fake_ip_route_matches_restored_domain_and_real_cidr_with_generation() {
    let fixture = support::fixture().await;
    let mut config = dns(fixture.port);
    config.fake_ip = Some(fake_ip::Config {
        ipv4_range: "198.18.0.0/16".into(),
        ipv6_range: "fd00:198:18::/64".into(),
        capacity: 16,
        exclude: vec![],
    });
    let resolver = Resolver::new(config).unwrap();
    let fake = resolver
        .resolve(Query {
            name: "connect.example.test".into(),
            query_type: "A".into(),
        })
        .await
        .unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let manager = Manager::default();
    let mut request = target(listener.local_addr().unwrap().port());
    request.target.domain = None;
    request.target.address = Some(fake.addresses[0].clone());
    let route = router(vec![
        rule(Match::IpCidr, "198.18.0.0/15", Policy::Reject),
        rule(Match::Domain, "connect.example.test", Policy::Direct),
    ]);
    assert!(matches!(
        manager
            .connect_routed(&route, Some(&resolver), request.clone())
            .await,
        Err(Error::InvalidState)
    ));
    request.mapping_generation = resolver.fake_ip_generation();
    let opened = manager
        .connect_routed(&route, Some(&resolver), request.clone())
        .await
        .unwrap();
    assert_eq!(opened.decision.rule_index, Some(1));
    let _peer = listener.accept().await.unwrap();
    manager.close(opened.connection.id).await.unwrap();
    resolver.reset_fake_ip().unwrap();
    assert!(matches!(
        manager
            .connect_routed(&route, Some(&resolver), request)
            .await,
        Err(Error::InvalidState)
    ));
}

#[tokio::test]
async fn fake_preallocation_exclusion_does_not_touch_dns() {
    let fixture = support::fixture().await;
    let mut settings = dns(fixture.port);
    settings.fake_ip = Some(fake_ip::Config {
        ipv4_range: "198.18.0.0/16".into(),
        ipv6_range: "fd00:198:18::/64".into(),
        capacity: 16,
        exclude: vec!["connect.example.test".into()],
    });
    let resolver = Resolver::new(settings).unwrap();
    assert!(matches!(
        resolver.allocate_fake_ip(Query {
            name: "connect.example.test".into(),
            query_type: "A".into()
        }),
        Err(Error::FakeIpUnavailable)
    ));
    assert_eq!(fixture.count.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn routed_dns_deadline_and_close_cancel_before_dial() {
    let fixture = support::fixture().await;
    let resolver = std::sync::Arc::new(Resolver::new(dns(fixture.port)).unwrap());
    let manager = std::sync::Arc::new(Manager::default());
    let router = std::sync::Arc::new(router(vec![rule(
        Match::DomainSuffix,
        "example.test",
        Policy::Direct,
    )]));
    let mut request = target(443);
    request.target.domain = Some("timeout.example.test".into());
    request.timeout_ms = 30;
    assert!(matches!(
        manager
            .connect_routed(&router, Some(&resolver), request.clone())
            .await,
        Err(Error::NetworkTimeout)
    ));
    request.target.domain = Some("timeout.cancel.example.test".into());
    request.timeout_ms = 2000;
    let before = fixture.count.load(Ordering::SeqCst);
    let active = manager.clone();
    let task = tokio::spawn(async move {
        active
            .connect_routed(&router, Some(&resolver), request)
            .await
    });
    tokio::time::timeout(Duration::from_millis(500), async {
        while fixture.count.load(Ordering::SeqCst) == before {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    manager.close_all().await.unwrap();
    assert!(matches!(task.await.unwrap(), Err(Error::Cancelled)));
}

#[tokio::test]
async fn formal_scoped_udp_reject_does_not_break_tcp_or_silently_send_udp() {
    let fixture = support::fixture().await;
    let resolver = Resolver::new(dns(fixture.port)).unwrap();
    let manager = Manager::default();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let mut scoped = rule(Match::Domain, "connect.example.test", Policy::Reject);
    scoped.protocol = Some(Transport::Udp);
    let route = router(vec![
        scoped,
        rule(Match::Domain, "connect.example.test", Policy::Direct),
    ]);
    let opened = manager
        .connect_routed(
            &route,
            Some(&resolver),
            target(listener.local_addr().unwrap().port()),
        )
        .await
        .unwrap();
    let (mut peer, _) = listener.accept().await.unwrap();
    manager
        .write(opened.connection.id, b"scope", 1000)
        .await
        .unwrap();
    let mut body = [0; 5];
    peer.read_exact(&mut body).await.unwrap();
    assert_eq!(&body, b"scope");
    manager.close(opened.connection.id).await.unwrap();
    let udp = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let mut request = target(udp.local_addr().unwrap().port());
    request.target.transport = Transport::Udp;
    let before_dns = fixture.count.load(Ordering::SeqCst);
    assert!(matches!(
        manager
            .connect_routed(&route, Some(&resolver), request)
            .await,
        Err(Error::Rejected)
    ));
    assert_eq!(fixture.count.load(Ordering::SeqCst), before_dns);
    assert!(
        tokio::time::timeout(Duration::from_millis(20), udp.recv_from(&mut body))
            .await
            .is_err()
    );
}
