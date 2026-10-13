//! 完整 IP 包进入同一 Rust 会话；真实 DNS 上游与 UDP 对端验证映射衔接。
#[path = "support/dns.rs"]
mod support;
use hickory_resolver::proto::{
    op::{Message, Query, ResponseCode},
    rr::{Name, RecordType},
};
use libfluxa_core::{
    dns::{Config, Resolver, Server, fake_ip},
    error::Error,
    outbound::{Limits, Manager},
    routing::{Config as Routing, Policy, Router},
    tun::{
        parse,
        udp::{Options, Session, encode},
    },
};
use std::{
    net::SocketAddr,
    sync::{Arc, atomic::Ordering},
    time::Duration,
};
use tokio::net::UdpSocket;
fn resolver(port: u16, fake: bool) -> Arc<Resolver> {
    Arc::new(
        Resolver::new(Config {
            servers: vec![Server::Udp {
                address: format!("127.0.0.1:{port}"),
            }],
            timeout_ms: 100,
            dnssec: false,
            service: None,
            fake_ip: fake.then(|| fake_ip::Config {
                ipv4_range: "198.18.0.0/16".into(),
                ipv6_range: "fd00:198:18::/64".into(),
                capacity: 8,
                exclude: vec!["excluded.test".into()],
            }),
        })
        .unwrap(),
    )
}
fn setup(resolver: Arc<Resolver>, capacity: usize) -> (Arc<Manager>, Session) {
    let manager = Arc::new(Manager::default());
    manager
        .configure_network(
            Some(resolver),
            Some(Arc::new(
                Router::compile(Routing {
                    fallback: Policy::Direct,
                    ..Routing::default()
                })
                .unwrap(),
            )),
            Limits::default(),
        )
        .unwrap();
    let session = Session::new(
        manager.clone(),
        Options {
            dns_endpoints: vec![
                "198.19.0.53:53".parse().unwrap(),
                "[fd00:198:19::53]:53".parse().unwrap(),
            ],
            output_capacity: capacity,
            ..Options::default()
        },
    )
    .unwrap();
    (manager, session)
}
fn request(name: &str, kind: RecordType, ipv6: bool) -> libfluxa_core::tun::udp::Packet {
    let mut message = Message::query();
    message.metadata.id = 0x1234;
    message.metadata.recursion_desired = true;
    message.add_query(Query::query(Name::from_ascii(name).unwrap(), kind));
    encode(
        if ipv6 {
            "[2001:db8::1]:12345"
        } else {
            "192.0.2.1:12345"
        }
        .parse()
        .unwrap(),
        if ipv6 {
            "[fd00:198:19::53]:53"
        } else {
            "198.19.0.53:53"
        }
        .parse()
        .unwrap(),
        &message.to_vec().unwrap(),
        1500,
    )
    .unwrap()
}
fn answer(session: &Session) -> Message {
    let packet = session.receive().unwrap().unwrap();
    let info = parse(&packet.bytes, 1500).unwrap();
    assert_eq!((info.source_port, info.destination_port), (53, 12345));
    Message::from_vec(&packet.bytes[packet.bytes.len() - info.payload_bytes..]).unwrap()
}
#[tokio::test]
async fn synthetic_ipv4_ipv6_packets_own_mapping_and_no_upstream() {
    let upstream = support::fixture().await;
    let dns = resolver(upstream.port, true);
    let (_, session) = setup(dns.clone(), 2);
    for (kind, ipv6) in [(RecordType::A, false), (RecordType::AAAA, true)] {
        session
            .submit(&request("connect.example.test", kind, ipv6).bytes, None)
            .await
            .unwrap();
        let response = answer(&session);
        assert_eq!(response.metadata.id, 0x1234);
        assert_eq!(response.metadata.response_code, ResponseCode::NoError);
        let ip = response.answers[0].data.to_string();
        assert_eq!(
            dns.restore_fake_ip(&ip, dns.fake_ip_generation().unwrap())
                .unwrap(),
            "connect.example.test"
        );
    }
    assert_eq!(upstream.count.load(Ordering::SeqCst), 0);
}
#[tokio::test]
async fn fake_packet_resolves_real_and_returns_original_synthetic_endpoint() {
    let upstream = support::fixture().await;
    let dns = resolver(upstream.port, true);
    let (_, session) = setup(dns.clone(), 2);
    session
        .submit(
            &request("connect.example.test", RecordType::A, false).bytes,
            None,
        )
        .await
        .unwrap();
    let ip = answer(&session).answers[0].data.to_string();
    // Auto 当前优先 IPv6，必须实际验证该候选返回而不是只检查 IPv4。
    let peer = UdpSocket::bind("[::1]:0").await.unwrap();
    let port = peer.local_addr().unwrap().port();
    let packet = encode(
        "192.0.2.1:3456".parse().unwrap(),
        format!("{ip}:{port}").parse().unwrap(),
        b"owned",
        1500,
    )
    .unwrap();
    session.submit(&packet.bytes, None).await.unwrap();
    let mut body = [0; 64];
    let (size, source) = tokio::time::timeout(Duration::from_secs(1), peer.recv_from(&mut body))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(&body[..size], b"owned");
    peer.send_to(b"return", source).await.unwrap();
    let response = tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            if let Some(p) = session.receive().unwrap() {
                break p;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_eq!(parse(&response.bytes, 1500).unwrap().source, ip);
    dns.reset_fake_ip().unwrap();
    let new_packet = encode(
        "192.0.2.1:3457".parse().unwrap(),
        format!("{ip}:{port}").parse().unwrap(),
        b"stale",
        1500,
    )
    .unwrap();
    assert!(matches!(
        session.submit(&new_packet.bytes, None).await,
        Err(Error::InvalidState)
    ));
    session.close().unwrap();
}
#[tokio::test]
async fn resolver_replacement_rejects_old_generation_even_same_ranges() {
    let upstream = support::fixture().await;
    let old = resolver(upstream.port, true);
    let (manager, session) = setup(old.clone(), 2);
    session
        .submit(&request("old.test", RecordType::A, false).bytes, None)
        .await
        .unwrap();
    let ip = answer(&session).answers[0].data.to_string();
    let new = resolver(upstream.port, true);
    assert_ne!(old.fake_ip_generation(), new.fake_ip_generation());
    manager.configure_dns(Some(new)).unwrap();
    assert!(matches!(
        session
            .submit(&request("new.test", RecordType::A, false).bytes, None)
            .await,
        Err(Error::InvalidState)
    ));
    let packet = encode(
        "192.0.2.1:2345".parse().unwrap(),
        format!("{ip}:1234").parse().unwrap(),
        b"no",
        1500,
    )
    .unwrap();
    assert!(matches!(
        session.submit(&packet.bytes, None).await,
        Err(Error::InvalidState)
    ));
}
#[tokio::test]
async fn real_query_negative_failure_malformed_and_backpressure() {
    let upstream = support::fixture().await;
    let (_, session) = setup(resolver(upstream.port, false), 1);
    for (name, code) in [
        ("example.test", ResponseCode::NoError),
        ("nxdomain.example.test", ResponseCode::NXDomain),
    ] {
        session
            .submit(&request(name, RecordType::A, false).bytes, None)
            .await
            .unwrap();
        assert_eq!(answer(&session).metadata.response_code, code);
    }
    let packet = encode(
        "192.0.2.1:12345".parse().unwrap(),
        "198.19.0.53:53".parse().unwrap(),
        &[0x12, 0x34, 0],
        1500,
    )
    .unwrap();
    session.submit(&packet.bytes, None).await.unwrap();
    assert!(matches!(
        session
            .submit(&request("blocked.test", RecordType::A, false).bytes, None)
            .await,
        Err(Error::ResourceLimit)
    ));
    assert_eq!(
        answer(&session).metadata.response_code,
        ResponseCode::FormErr
    );
    let unused = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let (_, failing) = setup(resolver(unused.local_addr().unwrap().port(), false), 1);
    failing
        .submit(&request("failure.test", RecordType::A, false).bytes, None)
        .await
        .unwrap();
    assert_eq!(
        answer(&failing).metadata.response_code,
        ResponseCode::ServFail
    );
}
#[tokio::test]
async fn endpoint_validation_cancellation_and_ordinary_forwarding() {
    let upstream = support::fixture().await;
    let (manager, _) = setup(resolver(upstream.port, false), 1);
    for endpoints in [
        vec!["0.0.0.0:53"],
        vec!["198.19.0.53:54"],
        vec!["198.19.0.53:53", "198.19.0.53:53"],
    ] {
        assert!(
            Session::new(
                manager.clone(),
                Options {
                    dns_endpoints: endpoints
                        .iter()
                        .map(|s| s.parse::<SocketAddr>().unwrap())
                        .collect(),
                    ..Options::default()
                }
            )
            .is_err()
        );
    }
    let unused = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let (_, session) = setup(resolver(unused.local_addr().unwrap().port(), false), 1);
    let session = Arc::new(session);
    let pending = {
        let session = session.clone();
        tokio::spawn(async move {
            session
                .submit(&request("cancel.test", RecordType::A, false).bytes, None)
                .await
        })
    };
    tokio::time::sleep(Duration::from_millis(10)).await;
    session.close().unwrap();
    assert!(matches!(pending.await.unwrap(), Err(Error::Cancelled)));
    assert!(session.receive().is_err());
    let (_, session) = setup(resolver(upstream.port, true), 1);
    let peer = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    session
        .submit(
            &encode(
                "192.0.2.1:1234".parse().unwrap(),
                peer.local_addr().unwrap(),
                b"normal",
                1500,
            )
            .unwrap()
            .bytes,
            None,
        )
        .await
        .unwrap();
    let mut body = [0; 64];
    let (size, _) = peer.recv_from(&mut body).await.unwrap();
    assert_eq!(&body[..size], b"normal");
}
#[tokio::test]
async fn mapping_quota_wrong_owner_udp_block_and_restart() {
    let upstream = support::fixture().await;
    let (manager, session) = setup(resolver(upstream.port, true), 1);
    let overlap = Session::new(
        manager.clone(),
        Options {
            dns_endpoints: vec!["198.18.0.53:53".parse().unwrap()],
            ..Options::default()
        },
    );
    assert!(matches!(overlap, Err(Error::InvalidInput)));
    for index in 0..8 {
        session
            .submit(
                &request(&format!("name{index}.test"), RecordType::A, false).bytes,
                None,
            )
            .await
            .unwrap();
        assert_eq!(
            answer(&session).metadata.response_code,
            ResponseCode::NoError
        );
    }
    session
        .submit(
            &request("over-quota.test", RecordType::A, false).bytes,
            None,
        )
        .await
        .unwrap();
    assert_eq!(
        answer(&session).metadata.response_code,
        ResponseCode::ServFail
    );
    let foreign = resolver(upstream.port, true);
    let generation = foreign.fake_ip_generation().unwrap();
    let packet = encode(
        "192.0.2.1:3456".parse().unwrap(),
        "198.18.0.1:1234".parse().unwrap(),
        b"no",
        1500,
    )
    .unwrap();
    assert!(matches!(
        session.submit(&packet.bytes, Some(generation)).await,
        Err(Error::InvalidState)
    ));
    manager
        .configure_routing(Some(Arc::new(
            Router::compile(Routing {
                fallback: Policy::Direct,
                rules: vec![libfluxa_core::routing::Rule {
                    r#type: libfluxa_core::routing::Match::Transport,
                    value: "udp".into(),
                    policy: Policy::Reject,
                    outbound: None,
                }],
                ..Routing::default()
            })
            .unwrap(),
        )))
        .unwrap();
    assert!(matches!(
        session
            .submit(&request("blocked.test", RecordType::A, false).bytes, None)
            .await,
        Err(Error::Rejected)
    ));
    let (manager, session) = setup(resolver(upstream.port, true), 1);
    session.close().unwrap();
    manager
        .configure_dns(Some(resolver(upstream.port, true)))
        .unwrap();
    let reopened = Session::new(
        manager,
        Options {
            dns_endpoints: vec!["198.19.0.53:53".parse().unwrap()],
            ..Options::default()
        },
    )
    .unwrap();
    reopened
        .submit(&request("new.test", RecordType::A, false).bytes, None)
        .await
        .unwrap();
    assert_eq!(
        answer(&reopened).metadata.response_code,
        ResponseCode::NoError
    );
}

#[tokio::test]
async fn reopened_same_pool_never_reuses_old_synthetic_address() {
    let upstream = support::fixture().await;
    let (_, old) = setup(resolver(upstream.port, true), 1);
    old.submit(&request("old-owner.test", RecordType::A, false).bytes, None)
        .await
        .unwrap();
    let old_ip = answer(&old).answers[0].data.to_string();
    old.close().unwrap();
    drop(old);
    let (_, new) = setup(resolver(upstream.port, true), 1);
    new.submit(
        &request("different-owner.test", RecordType::A, false).bytes,
        None,
    )
    .await
    .unwrap();
    let new_ip = answer(&new).answers[0].data.to_string();
    assert_ne!(old_ip, new_ip);
    let stale = encode(
        "192.0.2.1:2345".parse().unwrap(),
        format!("{old_ip}:1234").parse().unwrap(),
        b"no",
        1500,
    )
    .unwrap();
    assert!(matches!(
        new.submit(&stale.bytes, None).await,
        Err(Error::InvalidState)
    ));
}
#[tokio::test]
async fn ordinary_mapping_before_and_after_tun_remains_owned_and_never_collides() {
    let upstream = support::fixture().await;
    let make = || {
        Arc::new(
            Resolver::new(Config {
                servers: vec![Server::Udp {
                    address: format!("127.0.0.1:{}", upstream.port),
                }],
                timeout_ms: 100,
                dnssec: false,
                service: None,
                fake_ip: Some(fake_ip::Config {
                    ipv4_range: "198.19.0.0/16".into(),
                    ipv6_range: "fd00:199:19::/64".into(),
                    capacity: 8,
                    exclude: vec![],
                }),
            })
            .unwrap(),
        )
    };
    let ordinary = make();
    let query = |name: &str| libfluxa_core::dns::Query {
        name: name.into(),
        query_type: "A".into(),
    };
    let old = ordinary
        .allocate_fake_ip(query("old-ordinary.test"))
        .unwrap()
        .addresses[0]
        .clone();
    let manager = Arc::new(Manager::default());
    let owned = make();
    manager
        .configure_network(
            Some(owned.clone()),
            Some(Arc::new(
                Router::compile(Routing {
                    fallback: Policy::Direct,
                    ..Routing::default()
                })
                .unwrap(),
            )),
            Limits::default(),
        )
        .unwrap();
    let session = Session::new(
        manager,
        Options {
            dns_endpoints: vec!["192.0.2.53:53".parse().unwrap()],
            ..Options::default()
        },
    )
    .unwrap();
    let current = owned
        .allocate_fake_ip(query("new-owned.test"))
        .unwrap()
        .addresses[0]
        .clone();
    assert_ne!(current, old);
    let stale = encode(
        "192.0.2.1:1234".parse().unwrap(),
        format!("{old}:1234").parse().unwrap(),
        b"no",
        1500,
    )
    .unwrap();
    assert!(matches!(
        session.submit(&stale.bytes, None).await,
        Err(Error::InvalidState)
    ));
    let foreign = ordinary
        .allocate_fake_ip(query("foreign-new.test"))
        .unwrap()
        .addresses[0]
        .clone();
    assert_ne!(foreign, current);
    let packet = encode(
        "192.0.2.1:1234".parse().unwrap(),
        format!("{foreign}:1234").parse().unwrap(),
        b"no",
        1500,
    )
    .unwrap();
    assert!(matches!(
        session.submit(&packet.bytes, None).await,
        Err(Error::InvalidState)
    ));
}
