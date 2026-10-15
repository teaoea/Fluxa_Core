//! 测试文件：tests/test_tun_tcp_session.rs
//! 测试类型：单元测试、跨模块网络互通与生命周期回归测试
//! 测试对象：src/tun/tcp/session.rs；TCP/IP → DNS/Fake-IP → 路由 → 共用出站契约
//! 测试内容：real_ipv4_ipv6_data_and_half_close；rejection_invalid_input_and_close_never_dial；dns_tcp_length_framing_and_real_resolver；automatic_fake_ip_uses_original_return_address；concurrent_close_with_full_output_releases_actual_resources_and_quotas；close_cancels_pending_tcp_dns_and_revokes_retained_resolver；network_update_preserves_active_stream_and_rejects_new_flow；invalid_handshake_packets_do_not_refresh_idle_deadline；absolute_handshake_deadline_cancels_pending_real_dns_before_dial。
//! 测试依据：RFC 9293；RFC 7766；会话生命周期与资源所有权隔离原则。
//! 前置条件：允许绑定本机回环 TCP/UDP；不修改系统 VPN。
//! 虚拟侧独立构造 TCP/IP 向量；出站使用真实系统 TCP 服务，覆盖往返及半关闭。
use crate::test_fixture_https::dns as dns_support;
use crate::{
    dns::{Config as DnsConfig, Resolver, Server, fake_ip},
    error::{Error, Result},
    outbound::{Limits, Manager},
    routing::{Config, Policy, Router},
    tun::{
        Packet,
        tcp::{self, wire},
        udp,
    },
};
use std::{net::SocketAddr, sync::Arc, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};
fn packet(
    source: SocketAddr,
    destination: SocketAddr,
    sequence: u32,
    acknowledgement: u32,
    flags: u8,
    payload: &[u8],
) -> Vec<u8> {
    let ip = if source.is_ipv4() { 20 } else { 40 };
    let size = ip + 20 + payload.len();
    let mut bytes = vec![0; size];
    match (source.ip(), destination.ip()) {
        (std::net::IpAddr::V4(src), std::net::IpAddr::V4(dst)) => {
            bytes[0] = 0x45;
            bytes[2..4].copy_from_slice(&(size as u16).to_be_bytes());
            bytes[8] = 64;
            bytes[9] = 6;
            bytes[12..16].copy_from_slice(&src.octets());
            bytes[16..20].copy_from_slice(&dst.octets());
            let sum = checksum(&bytes[..20]);
            bytes[10..12].copy_from_slice(&sum.to_be_bytes());
        }
        (std::net::IpAddr::V6(src), std::net::IpAddr::V6(dst)) => {
            bytes[0] = 0x60;
            bytes[4..6].copy_from_slice(&((size - 40) as u16).to_be_bytes());
            bytes[6] = 6;
            bytes[7] = 64;
            bytes[8..24].copy_from_slice(&src.octets());
            bytes[24..40].copy_from_slice(&dst.octets());
        }
        _ => panic!("测试地址族不一致"),
    }
    bytes[ip..ip + 2].copy_from_slice(&source.port().to_be_bytes());
    bytes[ip + 2..ip + 4].copy_from_slice(&destination.port().to_be_bytes());
    bytes[ip + 4..ip + 8].copy_from_slice(&sequence.to_be_bytes());
    bytes[ip + 8..ip + 12].copy_from_slice(&acknowledgement.to_be_bytes());
    bytes[ip + 12] = 0x50;
    bytes[ip + 13] = flags;
    bytes[ip + 14..ip + 16].copy_from_slice(&32768u16.to_be_bytes());
    bytes[ip + 20..].copy_from_slice(payload);
    let mut pseudo = if ip == 20 {
        bytes[12..20].to_vec()
    } else {
        bytes[8..40].to_vec()
    };
    pseudo.extend_from_slice(&[0, 6]);
    pseudo.extend_from_slice(&((size - ip) as u16).to_be_bytes());
    pseudo.extend_from_slice(&bytes[ip..]);
    let sum = checksum(&pseudo);
    bytes[ip + 16..ip + 18].copy_from_slice(&sum.to_be_bytes());
    bytes
}
fn checksum(bytes: &[u8]) -> u16 {
    let mut sum: u32 = bytes
        .chunks(2)
        .map(|p| u32::from(p[0]) * 256 + u32::from(*p.get(1).unwrap_or(&0)))
        .sum();
    while sum >> 16 != 0 {
        sum = (sum & 65535) + (sum >> 16);
    }
    !(sum as u16)
}
fn setup(
    policy: Policy,
    resolver: Option<Arc<Resolver>>,
) -> (Arc<Manager>, Arc<udp::Session>, tcp::Session) {
    let manager = Arc::new(Manager::default());
    manager
        .configure_network(
            resolver,
            Some(Arc::new(
                Router::compile(Config {
                    fallback: policy,
                    ..Config::default()
                })
                .unwrap(),
            )),
            Limits::default(),
        )
        .unwrap();
    let owner = Arc::new(
        udp::Session::new(
            manager.clone(),
            udp::Options {
                dns_endpoints: manager
                    .network_snapshot()
                    .unwrap()
                    .resolver
                    .as_ref()
                    .map(|_| vec!["192.0.2.53:53".parse().unwrap()])
                    .unwrap_or_default(),
                ..udp::Options::default()
            },
        )
        .unwrap(),
    );
    let session = tcp::Session::new(
        owner.clone(),
        tcp::Options {
            time_wait_ms: 20,
            ..tcp::Options::default()
        },
    )
    .unwrap();
    (manager, owner, session)
}
async fn next(session: &tcp::Session) -> Result<Packet> {
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            if let Some(packet) = session.receive()? {
                return Ok(packet);
            }
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    })
    .await
    .map_err(|_| Error::NetworkTimeout)?
}
async fn handshake(session: &tcp::Session, source: SocketAddr, destination: SocketAddr) -> u32 {
    session
        .submit(&packet(source, destination, 100, 0, 2, &[]), None)
        .unwrap();
    let response = next(session).await.unwrap();
    let syn = wire::decode(&response.bytes, 1500).unwrap();
    assert!(syn.flags.has(wire::Flags::SYN));
    assert_eq!(syn.acknowledgement, 101);
    let sequence = syn.sequence.wrapping_add(1);
    session
        .submit(&packet(source, destination, 101, sequence, 16, &[]), None)
        .unwrap();
    sequence
}
#[tokio::test]
async fn real_ipv4_ipv6_data_and_half_close() {
    for address in ["127.0.0.1:0", "[::1]:0"] {
        let listener = TcpListener::bind(address).await.unwrap();
        let destination = listener.local_addr().unwrap();
        let source = if destination.is_ipv4() {
            "192.0.2.1:12345"
        } else {
            "[2001:db8::1]:12345"
        }
        .parse()
        .unwrap();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut bytes = Vec::new();
            stream.read_to_end(&mut bytes).await.unwrap();
            assert_eq!(bytes, b"abcdef");
            stream
                .write_all(b"response after half-close")
                .await
                .unwrap();
            stream.shutdown().await.unwrap();
        });
        let (_, _, session) = setup(Policy::Direct, None);
        let mut sequence = handshake(&session, source, destination).await;
        // 后半段先到、重复前半段不能在真实上游多交付一次。
        session
            .submit(
                &packet(source, destination, 104, sequence, 16, b"def"),
                None,
            )
            .unwrap();
        session
            .submit(
                &packet(source, destination, 101, sequence, 16, b"abc"),
                None,
            )
            .unwrap();
        session
            .submit(
                &packet(source, destination, 101, sequence, 16, b"abc"),
                None,
            )
            .unwrap();
        session
            .submit(&packet(source, destination, 107, sequence, 17, &[]), None)
            .unwrap();
        let mut body = Vec::new();
        let mut finished = false;
        for _ in 0..30 {
            let response = next(&session).await.unwrap();
            let data = wire::decode(&response.bytes, 1500).unwrap();
            if !data.payload.is_empty() {
                assert_eq!(data.sequence, sequence);
                body.extend_from_slice(data.payload);
                sequence = sequence.wrapping_add(data.payload.len() as u32);
            }
            if data.flags.has(wire::Flags::FIN) {
                sequence = sequence.wrapping_add(1);
                finished = true;
            }
            session
                .submit(&packet(source, destination, 108, sequence, 16, &[]), None)
                .unwrap();
            if finished {
                break;
            }
        }
        assert!(finished);
        assert_eq!(body, b"response after half-close");
        server.await.unwrap();
        session.close().await.unwrap();
    }
}
#[tokio::test]
async fn rejection_invalid_input_and_close_never_dial() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let destination = listener.local_addr().unwrap();
    let source = "192.0.2.1:12345".parse().unwrap();
    let (_, _, session) = setup(Policy::Reject, None);
    session
        .submit(&packet(source, destination, 100, 0, 2, &[]), None)
        .unwrap();
    let response = next(&session).await.unwrap();
    assert!(
        wire::decode(&response.bytes, 1500)
            .unwrap()
            .flags
            .has(wire::Flags::RST)
    );
    assert!(matches!(next(&session).await, Err(Error::Rejected)));
    assert!(
        tokio::time::timeout(Duration::from_millis(50), listener.accept())
            .await
            .is_err()
    );
    let mut invalid = packet(source, destination, 100, 0, 2, &[]);
    invalid[10] ^= 1;
    assert!(matches!(
        session.submit(&invalid, None),
        Err(Error::InvalidInput)
    ));
    assert!(
        session
            .submit(
                &packet(source, destination, 100, 0, 2, b"unsupported fast open"),
                None
            )
            .is_err()
    );
    session.close().await.unwrap();
    session.close().await.unwrap();
    assert!(
        session
            .submit(&packet(source, destination, 100, 0, 2, &[]), None)
            .is_err()
    );
}
#[tokio::test]
async fn dns_tcp_length_framing_and_real_resolver() {
    let upstream = dns_support::fixture().await;
    let resolver = Arc::new(
        Resolver::new(DnsConfig {
            servers: vec![Server::Udp {
                address: format!("127.0.0.1:{}", upstream.port),
            }],
            timeout_ms: 500,
            dnssec: false,
            fake_ip: None,
            service: None,
        })
        .unwrap(),
    );
    let (_, _, session) = setup(Policy::Direct, Some(resolver));
    let source = "192.0.2.1:12345".parse().unwrap();
    let destination = "192.0.2.53:53".parse().unwrap();
    let mut sequence = handshake(&session, source, destination).await;
    let mut query = vec![0x12, 0x34, 1, 0, 0, 1, 0, 0, 0, 0, 0, 0];
    for label in ["example", "test"] {
        query.push(label.len() as u8);
        query.extend_from_slice(label.as_bytes());
    }
    query.extend_from_slice(&[0, 0, 1, 0, 1]);
    let mut frame = (query.len() as u16).to_be_bytes().to_vec();
    frame.extend_from_slice(&query);
    session
        .submit(
            &packet(source, destination, 101, sequence, 16, &frame[..1]),
            None,
        )
        .unwrap();
    session
        .submit(
            &packet(source, destination, 102, sequence, 16, &frame[1..]),
            None,
        )
        .unwrap();
    let mut body = Vec::new();
    for _ in 0..30 {
        let response = next(&session).await.unwrap();
        let decoded = wire::decode(&response.bytes, 1500).unwrap();
        if !decoded.payload.is_empty() {
            body.extend_from_slice(decoded.payload);
            sequence = sequence.wrapping_add(decoded.payload.len() as u32);
        }
        session
            .submit(
                &packet(
                    source,
                    destination,
                    101 + frame.len() as u32,
                    sequence,
                    16,
                    &[],
                ),
                None,
            )
            .unwrap();
        if body.len() >= 2 && body.len() == 2 + usize::from(u16::from_be_bytes([body[0], body[1]]))
        {
            break;
        }
    }
    assert_eq!(body[2..4], [0x12, 0x34]);
    assert_eq!(&body[body.len() - 4..], [192, 0, 2, 44]);
    session.close().await.unwrap();
}
#[tokio::test]
async fn automatic_fake_ip_uses_original_return_address() {
    let upstream = dns_support::fixture().await;
    let resolver = Arc::new(
        Resolver::new(DnsConfig {
            servers: vec![Server::Udp {
                address: format!("127.0.0.1:{}", upstream.port),
            }],
            timeout_ms: 500,
            dnssec: false,
            fake_ip: Some(fake_ip::Config {
                ipv4_range: "198.18.0.0/16".into(),
                ipv6_range: "fd00:198:18::/64".into(),
                capacity: 8,
                exclude: vec![],
            }),
            service: None,
        })
        .unwrap(),
    );
    let (_, _, session) = setup(Policy::Direct, Some(resolver.clone()));
    let mapped = resolver
        .allocate_fake_ip(crate::dns::Query {
            name: "connect.ipv4only.example.test".into(),
            query_type: "A".into(),
        })
        .unwrap()
        .addresses[0]
        .clone();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let destination = format!("{mapped}:{}", listener.local_addr().unwrap().port())
        .parse()
        .unwrap();
    let source = "192.0.2.1:12345".parse().unwrap();
    let sequence = handshake(&session, source, destination).await;
    let (mut peer, _) = tokio::time::timeout(Duration::from_secs(3), listener.accept())
        .await
        .unwrap()
        .unwrap();
    session
        .submit(
            &packet(source, destination, 101, sequence, 16, b"mapped"),
            None,
        )
        .unwrap();
    let mut bytes = [0; 6];
    tokio::time::timeout(Duration::from_secs(3), peer.read_exact(&mut bytes))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(&bytes, b"mapped");
    peer.write_all(b"echo").await.unwrap();
    let mut echoed = false;
    for _ in 0..10 {
        let response = next(&session).await.unwrap();
        let data = wire::decode(&response.bytes, 1500).unwrap();
        assert_eq!(data.source.ip().to_string(), mapped);
        if !data.payload.is_empty() {
            assert_eq!(data.payload, b"echo");
            echoed = true;
            break;
        }
    }
    assert!(echoed, "必须实际收到 Fake-IP 返回数据");
    session.close().await.unwrap();
}

#[tokio::test]
async fn concurrent_close_with_full_output_releases_actual_resources_and_quotas() {
    let baseline = super::MEMORY.usage().0;
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let destination = listener.local_addr().unwrap();
    let (manager, owner, original) = setup(Policy::Direct, None);
    // 先关闭初始空会话，再用同一 Manager 重建；旧对象引用故意一直保留。
    original.close().await.unwrap();
    let owner_new = Arc::new(
        udp::Session::new(
            manager.clone(),
            udp::Options {
                output_capacity: 1,
                ..udp::Options::default()
            },
        )
        .unwrap(),
    );
    let session = Arc::new(
        tcp::Session::new(
            owner_new,
            tcp::Options {
                maximum_flows: 1,
                ..tcp::Options::default()
            },
        )
        .unwrap(),
    );
    let source = "192.0.2.1:12000".parse().unwrap();
    session
        .submit(&packet(source, destination, 100, 0, 2, &[]), None)
        .unwrap();
    let (mut peer, _) = tokio::time::timeout(Duration::from_secs(3), listener.accept())
        .await
        .unwrap()
        .unwrap();
    tokio::time::timeout(Duration::from_secs(3), async {
        while session.sender.capacity() != 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_eq!(super::MEMORY.usage().0, baseline + super::FLOW_MEMORY);
    let other = "192.0.2.1:12001".parse().unwrap();
    assert!(matches!(
        session.submit(&packet(other, destination, 100, 0, 2, &[]), None),
        Err(Error::ResourceLimit)
    ));
    let (first, second) = tokio::time::timeout(Duration::from_secs(3), async {
        tokio::join!(session.close(), session.close())
    })
    .await
    .unwrap();
    first.unwrap();
    second.unwrap();
    assert_eq!(super::MEMORY.usage().0, baseline);
    assert!(manager.status().unwrap().sessions.is_empty());
    let mut byte = [0];
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(3), peer.read(&mut byte))
            .await
            .unwrap()
            .unwrap(),
        0
    );
    assert!(matches!(session.receive(), Err(Error::InvalidState)));
    assert!(matches!(owner.receive(), Err(Error::InvalidState)));
    assert!(matches!(
        session.submit(&packet(source, destination, 100, 0, 2, &[]), None),
        Err(Error::InvalidState)
    ));
    // 内核已完成释放；保留旧 Session/Arc 不得占据新会话资源。
    let replacement =
        crate::tun::session::Session::new(manager, crate::tun::session::Options::default())
            .unwrap();
    replacement.close().await.unwrap();
}

#[tokio::test]
async fn close_cancels_pending_tcp_dns_and_revokes_retained_resolver() {
    let upstream = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let resolver = Arc::new(
        Resolver::new(DnsConfig {
            servers: vec![Server::Udp {
                address: upstream.local_addr().unwrap().to_string(),
            }],
            timeout_ms: 30000,
            dnssec: false,
            fake_ip: None,
            service: None,
        })
        .unwrap(),
    );
    let (manager, owner, session) = setup(Policy::Direct, Some(resolver.clone()));
    let source = "192.0.2.1:12345".parse().unwrap();
    let destination = "192.0.2.53:53".parse().unwrap();
    let sequence = handshake(&session, source, destination).await;
    let query = [
        0x12, 0x34, 1, 0, 0, 1, 0, 0, 0, 0, 0, 0, 1, b'a', 4, b't', b'e', b's', b't', 0, 0, 1, 0, 1,
    ];
    let mut frame = (query.len() as u16).to_be_bytes().to_vec();
    frame.extend_from_slice(&query);
    session
        .submit(
            &packet(source, destination, 101, sequence, 16, &frame),
            None,
        )
        .unwrap();
    let mut scratch = [0; 4096];
    tokio::time::timeout(Duration::from_secs(3), upstream.recv_from(&mut scratch))
        .await
        .unwrap()
        .unwrap();
    tokio::time::timeout(Duration::from_secs(3), session.close())
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(resolver.ensure_active(), Err(Error::InvalidState)));
    assert!(matches!(owner.receive(), Err(Error::InvalidState)));
    assert!(manager.status().unwrap().sessions.is_empty());
}

#[tokio::test]
async fn network_update_preserves_active_stream_and_rejects_new_flow() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let destination = listener.local_addr().unwrap();
    let source = "192.0.2.1:12345".parse().unwrap();
    let (manager, _, session) = setup(Policy::Direct, None);
    let sequence = handshake(&session, source, destination).await;
    let (mut peer, _) = tokio::time::timeout(Duration::from_secs(3), listener.accept())
        .await
        .unwrap()
        .unwrap();
    manager
        .configure_network(
            None,
            Some(Arc::new(
                Router::compile(Config {
                    fallback: Policy::Reject,
                    ..Config::default()
                })
                .unwrap(),
            )),
            Limits::default(),
        )
        .unwrap();
    // 已建立的真实连接继续使用原决策；不能因规则更新偷偷重拨或切换出口。
    session
        .submit(
            &packet(source, destination, 101, sequence, 16, b"existing"),
            None,
        )
        .unwrap();
    let mut bytes = [0; 8];
    tokio::time::timeout(Duration::from_secs(3), peer.read_exact(&mut bytes))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(&bytes, b"existing");
    // 新五元组必须使用已提交的新快照，拒绝后不能产生第二个真实 Socket。
    session
        .submit(
            &packet(
                "192.0.2.1:12346".parse().unwrap(),
                destination,
                100,
                0,
                2,
                &[],
            ),
            None,
        )
        .unwrap();
    let mut rejected = false;
    for _ in 0..8 {
        match next(&session).await {
            Err(Error::Rejected) => {
                rejected = true;
                break;
            }
            Ok(_) => {}
            Err(error) => panic!("新流应按规则拒绝，实际错误：{error:?}"),
        }
    }
    assert!(rejected);
    assert!(
        tokio::time::timeout(Duration::from_millis(50), listener.accept())
            .await
            .is_err()
    );
    session.close().await.unwrap();
}

#[tokio::test]
async fn invalid_handshake_packets_do_not_refresh_idle_deadline() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let destination = listener.local_addr().unwrap();
    let source = "192.0.2.1:12345".parse().unwrap();
    let (_, owner, original) = setup(Policy::Direct, None);
    let session = tcp::Session::new(
        owner,
        tcp::Options {
            idle_timeout_ms: 100,
            ..tcp::Options::default()
        },
    )
    .unwrap();
    session
        .submit(&packet(source, destination, 100, 0, 2, &[]), None)
        .unwrap();
    let syn_ack = next(&session).await.unwrap();
    let wrong_ack = wire::decode(&syn_ack.bytes, 1500)
        .unwrap()
        .sequence
        .wrapping_add(2);
    let (mut peer, _) = listener.accept().await.unwrap();
    let started = std::time::Instant::now();
    let send_invalid = async {
        loop {
            tokio::time::sleep(Duration::from_millis(15)).await;
            if let Err(error) =
                session.submit(&packet(source, destination, 101, wrong_ack, 16, &[]), None)
            {
                assert!(matches!(error, Error::InvalidState));
                break;
            }
        }
    };
    let observe_timeout = async {
        loop {
            match next(&session).await {
                Ok(_) => {}
                Err(Error::NetworkTimeout) => break,
                Err(error) => panic!("半开流应超时，实际：{error:?}"),
            }
        }
    };
    tokio::time::timeout(Duration::from_secs(2), async {
        tokio::join!(send_invalid, observe_timeout);
    })
    .await
    .unwrap();
    assert!(started.elapsed() < Duration::from_secs(1));
    let mut byte = [0];
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(1), peer.read(&mut byte))
            .await
            .unwrap()
            .unwrap(),
        0
    );
    session.close().await.unwrap();
    drop(original);
}

#[tokio::test]
async fn absolute_handshake_deadline_cancels_pending_real_dns_before_dial() {
    let upstream = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let resolver = Arc::new(
        Resolver::new(DnsConfig {
            servers: vec![Server::Udp {
                address: upstream.local_addr().unwrap().to_string(),
            }],
            timeout_ms: 30000,
            dnssec: false,
            fake_ip: Some(fake_ip::Config {
                ipv4_range: "198.18.0.0/16".into(),
                ipv6_range: "fd00:198:18::/64".into(),
                capacity: 8,
                exclude: vec![],
            }),
            service: None,
        })
        .unwrap(),
    );
    let (manager, owner, session) = setup(Policy::Direct, Some(resolver.clone()));
    let mapped = resolver
        .allocate_fake_ip(crate::dns::Query {
            name: "pending.example.test".into(),
            query_type: "A".into(),
        })
        .unwrap();
    let destination: SocketAddr = format!("{}:80", mapped.addresses[0]).parse().unwrap();
    let source = "192.0.2.1:12345".parse().unwrap();
    let (snapshot, generation) = owner.route_context(destination.ip(), None).unwrap();
    let (sender, _output) = tokio::sync::mpsc::channel(32);
    let (_keep_input_open, mut input) = tokio::sync::mpsc::channel(8);
    let authority = owner.authority();
    let mut context = super::Context {
        manager: manager.clone(),
        owner: owner.clone(),
        options: tcp::Options::default(),
        key: super::Key {
            source,
            destination,
        },
        snapshot: Some(snapshot),
        generation,
        dns: None,
        sender,
        cancel: owner.cancellation(),
        _lease: super::MEMORY.reserve(super::FLOW_MEMORY).unwrap(),
        _operation: authority.enter(authority.identity()).unwrap(),
    };
    let syn = packet(source, destination, 100, 0, 2, &[]);
    // 生产期限仍为 30 秒；以已消耗 29.8 秒的单调时间验证剩余期限约束真实 DNS 等待。
    let mut state = super::TcpState::new(
        &wire::decode(&syn, 1500).unwrap(),
        500,
        1500,
        Duration::from_millis(20),
        std::time::Instant::now() - Duration::from_millis(29800),
    )
    .unwrap();
    let started = std::time::Instant::now();
    let verify_query = async {
        let mut query = [0; 4096];
        upstream.recv_from(&mut query).await.unwrap();
    };
    let (result, _) = tokio::time::timeout(Duration::from_secs(2), async {
        tokio::join!(
            super::relay(&mut context, &mut state, &mut input),
            verify_query
        )
    })
    .await
    .unwrap();
    assert!(matches!(result, Err(Error::NetworkTimeout)));
    assert!(started.elapsed() < Duration::from_secs(1));
    assert!(manager.status().unwrap().sessions.is_empty());
    drop(context);
    session.close().await.unwrap();
}
