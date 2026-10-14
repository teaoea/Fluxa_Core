use libfluxa_core::{
    error::Error,
    outbound::{Family, Limits, Manager, RoutedTarget},
    routing::{
        Config, Match, Mode, Policy, Router, Rule, Target, Transport,
        runtime::{Layer, Source},
        store::Store,
    },
};
use std::{sync::Arc, time::Duration};
use tokio::net::UdpSocket;
fn rule(value: &str) -> Rule {
    Rule {
        r#type: Match::Transport,
        value: value.into(),
        policy: Policy::Reject,
        outbound: None,
    }
}
fn packet() -> Vec<u8> {
    let mut p = vec![0xc0, 0, 0, 0, 1, 1, 1, 1, 2, 0, 17];
    p.extend([0; 17]);
    p
}
fn target(port: u16, transport: Transport) -> RoutedTarget {
    RoutedTarget {
        target: Target {
            domain: None,
            address: Some("127.0.0.1".into()),
            port,
            transport,
        },
        timeout_ms: 500,
        family: Family::Ipv4,
        mapping_generation: None,
    }
}
#[tokio::test]
async fn datagram_guard_is_shared_and_existing_connection_keeps_snapshot() {
    let peer = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let port = peer.local_addr().unwrap().port();
    let manager = Manager::default();
    let router = Arc::new(
        Router::compile(Config {
            fallback: Policy::Direct,
            rules: vec![rule("quic")],
            ..Default::default()
        })
        .unwrap(),
    );
    manager
        .configure_network(None, Some(router), Limits::default())
        .unwrap();
    let opened = manager
        .connect_configured(target(port, Transport::Udp))
        .await
        .unwrap();
    assert_eq!(
        manager
            .write(opened.connection.id, b"ordinary UDP", 500)
            .await
            .unwrap(),
        12
    );
    let mut data = [0; 128];
    assert_eq!(peer.recv(&mut data).await.unwrap(), 12);
    assert_eq!(
        manager
            .write(opened.connection.id, &packet(), 500)
            .await
            .err(),
        Some(Error::Rejected)
    );
    assert!(
        tokio::time::timeout(Duration::from_millis(30), peer.recv(&mut data))
            .await
            .is_err()
    );
    let fresh = manager
        .connect_configured(target(port, Transport::Udp))
        .await
        .unwrap();
    let router = Arc::new(
        Router::compile(Config {
            mode: Mode::Direct,
            ..Default::default()
        })
        .unwrap(),
    );
    manager
        .configure_network(None, Some(router), Limits::default())
        .unwrap();
    assert_eq!(
        manager
            .write(fresh.connection.id, &packet(), 500)
            .await
            .err(),
        Some(Error::Rejected)
    );
    let allowed = manager
        .connect_configured(target(port, Transport::Udp))
        .await
        .unwrap();
    manager
        .write(allowed.connection.id, &packet(), 500)
        .await
        .unwrap();
    peer.recv(&mut data).await.unwrap();
    manager.close_all().await.unwrap();
    assert!(manager.status().unwrap().sessions.is_empty());
}
#[tokio::test]
async fn transport_reject_cannot_be_bypassed_by_modes() {
    for mode in [Mode::Direct, Mode::Rules, Mode::Global] {
        for (name, transport) in [("tcp", Transport::Tcp), ("udp", Transport::Udp)] {
            let manager = Manager::default();
            let router = Arc::new(
                Router::compile(Config {
                    mode,
                    rules: vec![rule(name)],
                    fallback: Policy::Direct,
                    outbound: (mode == Mode::Global).then(|| "missing".into()),
                    ..Default::default()
                })
                .unwrap(),
            );
            manager
                .configure_network(None, Some(router), Limits::default())
                .unwrap();
            assert_eq!(
                manager
                    .open(
                        "127.0.0.1:443".parse().unwrap(),
                        transport,
                        Policy::Direct,
                        500
                    )
                    .await
                    .err(),
                Some(Error::Rejected)
            );
            assert_eq!(
                manager
                    .connect_configured(target(443, transport))
                    .await
                    .err(),
                Some(Error::Rejected)
            );
        }
    }
}
#[test]
fn duplicate_sources_and_binary_cache_restore_guard() {
    let dir = std::env::temp_dir().join(format!("fluxa-quic-rules-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let sources = vec![
        Source {
            id: "local".into(),
            layer: Layer::Local,
            enabled: true,
            rules: vec![rule("quic")],
        },
        Source {
            id: "remote".into(),
            layer: Layer::Remote,
            enabled: true,
            rules: vec![rule("quic")],
        },
    ];
    {
        let store = Store::open(&dir).unwrap();
        store
            .update(
                Config {
                    fallback: Policy::Direct,
                    ..Default::default()
                },
                sources,
            )
            .unwrap();
        let s = store.snapshot().unwrap();
        assert!(s.blocks_quic());
        assert_eq!(s.document().report.effective, 1);
        assert_eq!(s.document().report.duplicates, 1);
        store.close().unwrap();
    }
    {
        let store = Store::open(&dir).unwrap();
        assert!(store.snapshot().unwrap().blocks_quic());
        store.close().unwrap();
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn incoming_quic_and_explicit_router_are_guarded() {
    let peer = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let port = peer.local_addr().unwrap().port();
    let manager = Manager::default();
    let router = Router::compile(Config {
        fallback: Policy::Direct,
        rules: vec![rule("quic")],
        ..Default::default()
    })
    .unwrap();
    let opened = manager
        .connect_routed(&router, None, target(port, Transport::Udp))
        .await
        .unwrap();
    manager
        .write(opened.connection.id, b"ordinary", 500)
        .await
        .unwrap();
    let mut bytes = [0; 64];
    let (_, address) = peer.recv_from(&mut bytes).await.unwrap();
    peer.send_to(&packet(), address).await.unwrap();
    assert_eq!(
        manager.read(opened.connection.id, 64, 500).await.err(),
        Some(Error::Rejected)
    );
    manager.close_all().await.unwrap();
    assert!(manager.status().unwrap().sessions.is_empty());
}
