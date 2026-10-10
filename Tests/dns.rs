//! 每项检查使用真正套接字，不用模拟返回值替代 DNS 互通。
#[path = "support/dns.rs"]
mod support;
use libfluxa_core::{
    dns::{Config, Query, Resolver, Server},
    error::Error,
};
use std::{sync::atomic::Ordering, time::Duration};
fn config(server: Server) -> Config {
    Config {
        servers: vec![server],
        timeout_ms: 1000,
        dnssec: false,
    }
}
fn udp(port: u16) -> Server {
    Server::Udp {
        address: format!("127.0.0.1:{port}"),
    }
}
fn query(name: &str, kind: &str) -> Query {
    Query {
        name: name.into(),
        query_type: kind.into(),
    }
}
fn tls() -> rustls::ClientConfig {
    let roots = rustls::RootCertStore::empty();
    rustls::ClientConfig::builder_with_provider(std::sync::Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .unwrap()
    .with_root_certificates(roots)
    .with_no_client_auth()
}
#[tokio::test]
async fn udp_tcp_dot_doh_cname_aaaa_and_records() {
    let service = support::fixture().await;
    assert!(service.pem.starts_with("-----BEGIN CERTIFICATE-----"));
    let mut roots = rustls::RootCertStore::empty();
    roots.add(service.root.clone()).unwrap();
    let trust = rustls::ClientConfig::builder_with_provider(std::sync::Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .unwrap()
    .with_root_certificates(roots)
    .with_no_client_auth();
    for server in [
        udp(service.port),
        Server::Tcp {
            address: format!("127.0.0.1:{}", service.port),
        },
        Server::Dot {
            address: format!("127.0.0.1:{}", service.dot_port),
            server_name: "localhost".into(),
        },
        Server::Doh {
            url: format!("https://localhost:{}/dns-query", service.doh_port),
            bootstrap: format!("127.0.0.1:{}", service.doh_port),
        },
    ] {
        let resolver = Resolver::with_tls_config(config(server), trust.clone()).unwrap();
        let a = resolver
            .resolve(query("alias.example.test", "A"))
            .await
            .unwrap();
        assert_eq!(a.addresses, ["192.0.2.44"]);
        assert!(a.records.iter().any(|r| r.record_type == "CNAME"));
        assert_eq!(
            resolver
                .resolve(query("example.test", "AAAA"))
                .await
                .unwrap()
                .addresses,
            ["2001:db8::44"]
        );
        assert!(
            resolver
                .resolve(query("example.test", "TXT"))
                .await
                .unwrap()
                .records
                .iter()
                .any(|r| r.data.contains("dns-fixture"))
        );
        assert_eq!(
            resolver
                .resolve(query("tcp.example.test", "A"))
                .await
                .unwrap()
                .addresses,
            ["192.0.2.44"]
        );
    }
}
#[tokio::test]
async fn positive_negative_cache_ttl_zero_expiry_clear_and_isolation() {
    let service = support::fixture().await;
    let resolver = Resolver::new(config(udp(service.port))).unwrap();
    for name in [
        "cache.example.test",
        "nxdomain.example.test",
        "nodata.example.test",
    ] {
        let first = resolver.resolve(query(name, "A")).await.unwrap();
        let before = service.count.load(Ordering::SeqCst);
        let cached = resolver.resolve(query(name, "A")).await.unwrap();
        assert!(cached.from_cache);
        assert_eq!(cached.rcode, first.rcode);
        assert_eq!(service.count.load(Ordering::SeqCst), before);
    }
    for _ in 0..2 {
        assert!(
            !resolver
                .resolve(query("zero.example.test", "A"))
                .await
                .unwrap()
                .from_cache
        );
    }
    resolver
        .resolve(query("short.example.test", "A"))
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(1100)).await;
    assert!(
        !resolver
            .resolve(query("short.example.test", "A"))
            .await
            .unwrap()
            .from_cache
    );
    resolver.clear().unwrap();
    assert!(
        !resolver
            .resolve(query("cache.example.test", "A"))
            .await
            .unwrap()
            .from_cache
    );
    let fresh = Resolver::new(config(udp(service.port))).unwrap();
    assert!(
        !fresh
            .resolve(query("cache.example.test", "A"))
            .await
            .unwrap()
            .from_cache
    );
}
#[tokio::test]
async fn timeout_wrong_transaction_and_failure_recovery() {
    let service = support::fixture().await;
    let mut settings = config(udp(service.port));
    settings.timeout_ms = 100;
    let resolver = Resolver::new(settings).unwrap();
    for name in ["timeout.example.test", "mismatch.example.test"] {
        assert!(resolver.resolve(query(name, "A")).await.is_err());
    }
    assert_eq!(
        resolver
            .resolve(query("example.test", "A"))
            .await
            .unwrap()
            .addresses,
        ["192.0.2.44"]
    );
}
#[tokio::test]
async fn encrypted_certificate_and_name_failure_never_downgrade() {
    let service = support::fixture().await;
    for server in [
        Server::Dot {
            address: format!("127.0.0.1:{}", service.dot_port),
            server_name: "wrong.example".into(),
        },
        Server::Doh {
            url: format!("https://localhost:{}/dns-query", service.doh_port),
            bootstrap: format!("127.0.0.1:{}", service.doh_port),
        },
    ] {
        let resolver = Resolver::with_tls_config(config(server), tls()).unwrap();
        assert!(resolver.resolve(query("example.test", "A")).await.is_err());
    }
}
#[test]
fn invalid_config_names_and_mixed_security_are_rejected() {
    for server in [
        Server::Udp {
            address: "host.example:53".into(),
        },
        Server::Udp {
            address: "0.0.0.0:53".into(),
        },
        Server::Doh {
            url: "http://localhost/dns-query".into(),
            bootstrap: "127.0.0.1:80".into(),
        },
        Server::Dot {
            address: "127.0.0.1:853".into(),
            server_name: "bad..name".into(),
        },
    ] {
        assert!(config(server).validate().is_err());
    }
    let mut mixed = config(udp(53));
    mixed.servers.push(Server::Dot {
        address: "127.0.0.1:853".into(),
        server_name: "localhost".into(),
    });
    assert_eq!(mixed.validate(), Err(Error::InvalidInput));
}

#[tokio::test]
async fn explicit_upstream_failover_query_type_and_resource_validation() {
    let service = support::fixture().await;
    let silent = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let mut settings = config(udp(silent.local_addr().unwrap().port()));
    settings.servers.push(udp(service.port));
    let resolver = Resolver::new(settings).unwrap();
    assert_eq!(
        resolver
            .resolve(query("example.test", "A"))
            .await
            .unwrap()
            .addresses,
        ["192.0.2.44"]
    );
    assert!(
        !resolver
            .resolve(query("example.test", "AAAA"))
            .await
            .unwrap()
            .from_cache
    );
    for (name, kind) in [
        ("bad..name", "A"),
        ("example.test", "AXFR"),
        ("example.test", "ANY"),
    ] {
        assert!(resolver.resolve(query(name, kind)).await.is_err());
    }
    let trusted = {
        let mut roots = rustls::RootCertStore::empty();
        roots.add(service.root.clone()).unwrap();
        rustls::ClientConfig::builder_with_provider(std::sync::Arc::new(
            rustls::crypto::ring::default_provider(),
        ))
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_root_certificates(roots)
        .with_no_client_auth()
    };
    let wrong_name = Resolver::with_tls_config(
        config(Server::Dot {
            address: format!("127.0.0.1:{}", service.dot_port),
            server_name: "wrong.example".into(),
        }),
        trusted,
    )
    .unwrap();
    assert!(
        wrong_name
            .resolve(query("example.test", "A"))
            .await
            .is_err()
    );
}

#[path = "support/dnssec.rs"]
mod signed_support;
#[tokio::test]
async fn dnssec_signed_root_valid_tampered_expired_and_untrusted() {
    let service = signed_support::fixture().await;
    let mut settings = config(udp(service.port));
    settings.dnssec = true;
    let resolver = Resolver::with_trust_anchors(settings.clone(), service.anchors.clone()).unwrap();
    let valid = resolver.resolve(query("valid.", "A")).await.unwrap();
    assert_eq!(valid.validation, "secure");
    assert_eq!(valid.addresses, ["192.0.2.55"]);
    for name in ["bogus.", "expired."] {
        assert!(resolver.resolve(query(name, "A")).await.is_err());
    }
    let negative = resolver.resolve(query("nxdomain.", "A")).await.unwrap();
    assert_eq!(negative.rcode, 3);
    assert_eq!(negative.validation, "secure");
    assert!(negative.ttl <= 5);
    assert!(
        resolver
            .resolve(query("nxdomain.", "A"))
            .await
            .unwrap()
            .from_cache
    );
    let untrusted = Resolver::new(settings).unwrap();
    assert!(untrusted.resolve(query("valid.", "A")).await.is_err());
}
