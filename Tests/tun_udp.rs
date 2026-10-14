use libfluxa_core::{
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
async fn next(session: &Session) -> libfluxa_core::error::Result<libfluxa_core::tun::udp::Packet> {
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
        let peer = UdpSocket::bind(format!("{}:0", if ip.contains(':') { "[::1]" } else { ip }))
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

#[path = "support/dns.rs"]
mod support;
#[tokio::test]
async fn fake_ip_requires_live_resolver_generation_and_never_dials_synthetic_address() {
    use libfluxa_core::dns::{Config as DnsConfig, Query, Resolver, Server, fake_ip};
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
    let (size, peer) = tokio::time::timeout(Duration::from_secs(2), socket.recv_from(&mut buffer))
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
    socket.send_to(&vec![0; 1280], peer).await.unwrap();
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
        let (_, peer) = tokio::time::timeout(Duration::from_secs(2), socket.recv_from(&mut buffer))
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
