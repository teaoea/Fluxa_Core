//! 从真实 DNS 报文取得地址，再连接实际服务；禁止系统解析隐式回退。
#[path = "support/dns.rs"]
mod support;
use libfluxa_core::{
    dns::{Config, Resolver, Server},
    error::Error,
    outbound::{DomainTarget, Family, Manager},
    routing::{Policy, Transport},
};
use std::{
    sync::{Arc, atomic::Ordering},
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, UdpSocket},
};
fn config(server: Server) -> Config {
    Config {
        servers: vec![server],
        timeout_ms: 1000,
        dnssec: false,
        fake_ip: None,
        service: None,
    }
}
fn target(port: u16, transport: Transport) -> DomainTarget {
    DomainTarget {
        domain: "alias.connect.example.test".into(),
        port,
        transport,
        policy: Policy::Direct,
        timeout_ms: 2000,
        family: Family::Ipv4,
    }
}
#[tokio::test]
async fn all_dns_transports_feed_real_tcp_and_reuse_cache() {
    let fixture = support::fixture().await;
    assert!(fixture.pem.starts_with("-----BEGIN CERTIFICATE-----"));
    let mut roots = rustls::RootCertStore::empty();
    roots.add(fixture.root.clone()).unwrap();
    let trust = rustls::ClientConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .unwrap()
    .with_root_certificates(roots)
    .with_no_client_auth();
    for server in [
        Server::Udp {
            address: format!("127.0.0.1:{}", fixture.port),
        },
        Server::Tcp {
            address: format!("127.0.0.1:{}", fixture.port),
        },
        Server::Dot {
            address: format!("127.0.0.1:{}", fixture.dot_port),
            server_name: "localhost".into(),
        },
        Server::Doh {
            url: format!("https://localhost:{}/dns-query", fixture.doh_port),
            bootstrap: format!("127.0.0.1:{}", fixture.doh_port),
        },
    ] {
        let resolver = Resolver::with_tls_config(config(server), trust.clone()).unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let peer = tokio::spawn(async move {
            for _ in 0..2 {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut byte = [0];
                socket.read_exact(&mut byte).await.unwrap();
                socket.write_all(&byte).await.unwrap();
            }
        });
        let manager = Manager::default();
        let before = fixture.count.load(Ordering::SeqCst);
        for _ in 0..2 {
            let opened = manager
                .connect(&resolver, target(port, Transport::Tcp))
                .await
                .unwrap();
            assert_eq!(opened.address, format!("127.0.0.1:{port}"));
            manager.write(opened.id, b"x", 1000).await.unwrap();
            assert_eq!(
                &*manager.read(opened.id, 1, 1000).await.unwrap().bytes,
                b"x"
            );
            manager.close(opened.id).await.unwrap();
        }
        assert_eq!(fixture.count.load(Ordering::SeqCst) - before, 1);
        peer.await.unwrap();
    }
}
#[tokio::test]
async fn family_fallback_and_udp_use_configured_records() {
    let fixture = support::fixture().await;
    let resolver = Resolver::new(config(Server::Udp {
        address: format!("127.0.0.1:{}", fixture.port),
    }))
    .unwrap();
    let manager = Manager::default();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let mut request = target(listener.local_addr().unwrap().port(), Transport::Tcp);
    request.family = Family::Auto;
    let opened = manager.connect(&resolver, request).await.unwrap();
    assert!(opened.address.starts_with("127.0.0.1:"));
    let _peer = listener.accept().await.unwrap();
    manager.close(opened.id).await.unwrap();
    let socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let opened = manager
        .connect(
            &resolver,
            target(socket.local_addr().unwrap().port(), Transport::Udp),
        )
        .await
        .unwrap();
    manager.write(opened.id, b"udp", 1000).await.unwrap();
    let mut bytes = [0; 3];
    let (size, peer) = socket.recv_from(&mut bytes).await.unwrap();
    assert_eq!(size, 3);
    socket.send_to(&bytes, peer).await.unwrap();
    assert_eq!(
        &*manager.read(opened.id, 3, 1000).await.unwrap().bytes,
        b"udp"
    );
    manager.close(opened.id).await.unwrap();
}
#[tokio::test]
async fn rejected_policy_synthetic_and_negative_answers_never_connect() {
    let fixture = support::fixture().await;
    let mut cfg = config(Server::Udp {
        address: format!("127.0.0.1:{}", fixture.port),
    });
    let resolver = Resolver::new(cfg.clone()).unwrap();
    let manager = Manager::default();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    for policy in [Policy::Reject, Policy::Proxy] {
        let mut request = target(port, Transport::Tcp);
        request.policy = policy;
        assert!(manager.connect(&resolver, request).await.is_err());
    }
    assert_eq!(fixture.count.load(Ordering::SeqCst), 0);
    for domain in ["nxdomain.example.test", "nodata.example.test"] {
        let mut request = target(port, Transport::Tcp);
        request.domain = domain.into();
        assert!(matches!(
            manager.connect(&resolver, request).await,
            Err(Error::DnsResponse)
        ));
    }
    cfg.fake_ip = Some(libfluxa_core::dns::fake_ip::Config {
        ipv4_range: "198.18.0.0/16".into(),
        ipv6_range: "fd00:198:18::/64".into(),
        capacity: 16,
        exclude: vec![],
    });
    let synthetic = Resolver::new(cfg).unwrap();
    assert!(matches!(
        manager
            .connect(&synthetic, target(port, Transport::Tcp))
            .await,
        Err(Error::Unsupported)
    ));
    assert!(
        tokio::time::timeout(Duration::from_millis(30), listener.accept())
            .await
            .is_err()
    );
}
#[tokio::test]
async fn total_deadline_and_close_all_cancel_pending_dns() {
    let fixture = support::fixture().await;
    let resolver = Arc::new(
        Resolver::new(config(Server::Udp {
            address: format!("127.0.0.1:{}", fixture.port),
        }))
        .unwrap(),
    );
    let manager = Arc::new(Manager::default());
    let mut request = target(443, Transport::Tcp);
    request.domain = "timeout.example.test".into();
    request.timeout_ms = 30;
    assert!(matches!(
        manager.connect(&resolver, request.clone()).await,
        Err(Error::NetworkTimeout)
    ));
    request.timeout_ms = 2000;
    let pending_manager = manager.clone();
    let pending_resolver = resolver.clone();
    let pending =
        tokio::spawn(async move { pending_manager.connect(&pending_resolver, request).await });
    tokio::time::sleep(Duration::from_millis(20)).await;
    manager.close_all().await.unwrap();
    assert!(matches!(pending.await.unwrap(), Err(Error::Cancelled)));
}

#[path = "support/dnssec.rs"]
mod signed_support;
#[tokio::test]
async fn dnssec_failure_cannot_reach_outbound() {
    let fixture = signed_support::fixture().await;
    let mut settings = config(Server::Udp {
        address: format!("127.0.0.1:{}", fixture.port),
    });
    settings.dnssec = true;
    let resolver = Resolver::with_trust_anchors(settings, fixture.anchors.clone()).unwrap();
    let manager = Manager::default();
    for domain in ["bogus.", "expired."] {
        let mut request = target(443, Transport::Tcp);
        request.domain = domain.into();
        assert!(matches!(
            manager.connect(&resolver, request).await,
            Err(Error::DnsValidation)
        ));
    }
}

#[tokio::test]
async fn explicit_ipv6_uses_aaaa_and_invalid_input_avoids_dns() {
    let fixture = support::fixture().await;
    let resolver = Resolver::new(config(Server::Udp {
        address: format!("127.0.0.1:{}", fixture.port),
    }))
    .unwrap();
    let manager = Manager::default();
    for domain in ["127.0.0.1", "bad host", ""] {
        let mut request = target(443, Transport::Tcp);
        request.domain = domain.into();
        assert!(matches!(
            manager.connect(&resolver, request).await,
            Err(Error::InvalidInput)
        ));
    }
    assert_eq!(fixture.count.load(Ordering::SeqCst), 0);
    let listener = TcpListener::bind("[::1]:0").await.unwrap();
    let mut request = target(listener.local_addr().unwrap().port(), Transport::Tcp);
    request.family = Family::Ipv6;
    let opened = manager.connect(&resolver, request).await.unwrap();
    assert!(opened.address.starts_with("[::1]:"));
    let _peer = listener.accept().await.unwrap();
    manager.close(opened.id).await.unwrap();
    assert_eq!(fixture.count.load(Ordering::SeqCst), 1);
}
