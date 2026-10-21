//! 测试文件：tests/test_dns.rs
//! 测试类型：集成测试、回归测试
//! 测试对象：src/dns/mod.rs
//! 测试内容：
//! - udp_tcp_dot_doh_cname_aaaa_and_records：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - positive_negative_cache_ttl_zero_expiry_clear_and_isolation：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - timeout_wrong_transaction_and_failure_recovery：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - encrypted_certificate_and_name_failure_never_downgrade：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - invalid_config_names_and_mixed_security_are_rejected：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - explicit_upstream_failover_query_type_and_resource_validation：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - dnssec_signed_root_valid_tampered_expired_and_untrusted：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - fake_ip_stability_reverse_families_exclusions_and_reset_no_reuse：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - fake_ip_range_alignment_capacity_and_exhaustion：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - fake_ip_dns_does_not_contact_upstream_and_never_claims_dnssec：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - fake_ip_concurrent_reuse_is_unique_and_reversible：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//!
//! 测试依据：保留原有公开接口契约及全部断言；测试文件原注释保留在下方，L3 必须独立核对。
//!
//! 前置条件：本地测试需文件/回环套接字权限；平台限定及忽略项以实际登记为准。
//!
//! 执行状态：由同名 Markdown 报告记录，不在源码中固化通过结论。

// 每项检查使用真正套接字，不用模拟返回值替代 DNS 互通。
#[path = "test_fixture_dns.rs"]
mod support;
use fluxa_core::{
    dns::{Config, Query, Resolver, Server},
    error::Error,
};
use std::{sync::atomic::Ordering, time::Duration};
fn config(server: Server) -> Config {
    Config {
        servers: vec![server],
        timeout_ms: 1000,
        dnssec: false,
        fake_ip: None,
        service: None,
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
        let observed = resolver.last_query().unwrap().unwrap();
        assert!(observed.successful && !observed.from_cache);
        let cached = resolver
            .resolve(query("alias.example.test", "A"))
            .await
            .unwrap();
        assert!(cached.from_cache);
        let observed = resolver.last_query().unwrap().unwrap();
        assert!(observed.successful && observed.from_cache && observed.duration_ms == 0);
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
        let observation = resolver.last_query().unwrap().unwrap();
        assert!(!observation.successful && !observation.from_cache);
        assert!(observation.duration_ms >= 50);
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

#[path = "test_fixture_dnssec.rs"]
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
        let observation = resolver.last_query().unwrap().unwrap();
        assert!(!observation.successful && !observation.from_cache);
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

fn fake_config(capacity: usize) -> fluxa_core::dns::fake_ip::Config {
    fluxa_core::dns::fake_ip::Config {
        ipv4_range: "198.18.0.0/24".into(),
        ipv6_range: "fd00:198:18::/64".into(),
        capacity,
        exclude: vec!["example.com".into()],
    }
}
#[test]
fn fake_ip_stability_reverse_families_exclusions_and_reset_no_reuse() {
    use fluxa_core::dns::fake_ip::Table;
    let table = Table::new(fake_config(2)).unwrap();
    let (first, cached) = table.allocate("Example.test.", false).unwrap().unwrap();
    assert!(!cached);
    assert_eq!(first, "198.18.0.1");
    assert_eq!(
        table.allocate("example.test", false).unwrap().unwrap(),
        (first.clone(), true)
    );
    let (ipv6, _) = table.allocate("example.test", true).unwrap().unwrap();
    assert_eq!(
        table.reverse(&ipv6).unwrap().as_deref(),
        Some("example.test")
    );
    assert_eq!(
        table.reverse(&first).unwrap().as_deref(),
        Some("example.test")
    );
    for name in ["example.com", "www.example.com", ".", "192.0.2.1"] {
        assert!(table.allocate(name, false).unwrap().is_none());
    }
    table.allocate("example.com.evil", false).unwrap();
    assert!(matches!(
        table.allocate("overflow.test", false),
        Err(Error::ResourceLimit)
    ));
    table.reset().unwrap();
    assert!(table.reverse(&first).unwrap().is_none());
    assert_eq!(
        table
            .allocate("after-reset.test", false)
            .unwrap()
            .unwrap()
            .0,
        "198.18.0.3"
    );
    assert!(table.reverse("203.0.113.1").unwrap().is_none());
    assert!(table.reverse("invalid").is_err());
}
#[test]
fn fake_ip_range_alignment_capacity_and_exhaustion() {
    use fluxa_core::dns::fake_ip::Table;
    for bad in [
        "192.0.2.0/24",
        "198.18.0.1/24",
        "198.18.0.0/32",
        "198.18.0.0/0",
        "invalid",
    ] {
        let mut config = fake_config(1);
        config.ipv4_range = bad.into();
        assert!(config.validate().is_err());
    }
    let mut config = fake_config(2);
    config.ipv4_range = "198.18.0.0/30".into();
    let table = Table::new(config).unwrap();
    for name in ["one.test", "two.test"] {
        table.allocate(name, false).unwrap();
    }
    table.reset().unwrap();
    assert!(matches!(
        table.allocate("three.test", false),
        Err(Error::ResourceLimit)
    ));
    assert!(fake_config(0).validate().is_err());
    assert!(fake_config(4097).validate().is_err());
}
#[tokio::test]
async fn fake_ip_dns_does_not_contact_upstream_and_never_claims_dnssec() {
    let service = support::fixture().await;
    let mut config = config(udp(service.port));
    config.fake_ip = Some(fake_config(4));
    config.dnssec = true;
    let resolver = Resolver::new(config.clone()).unwrap();
    let answer = resolver.resolve(query("fake.test", "A")).await.unwrap();
    assert_eq!(answer.validation, "synthetic");
    assert_eq!(answer.addresses, ["198.18.0.1"]);
    let ipv6 = resolver.resolve(query("fake.test", "AAAA")).await.unwrap();
    assert_eq!(
        resolver
            .reverse_fake_ip(&ipv6.addresses[0])
            .unwrap()
            .as_deref(),
        Some("fake.test")
    );
    assert_eq!(service.count.load(Ordering::SeqCst), 0);
    config.dnssec = false;
    let real = Resolver::new(config).unwrap();
    assert_eq!(
        real.resolve(query("www.example.com", "A"))
            .await
            .unwrap()
            .addresses,
        ["192.0.2.44"]
    );
    assert_eq!(
        real.resolve(query("fake.test", "TXT"))
            .await
            .unwrap()
            .validation,
        "disabled"
    );
    resolver.reset_fake_ip().unwrap();
    assert!(
        resolver
            .reverse_fake_ip(&answer.addresses[0])
            .unwrap()
            .is_none()
    );
}

#[test]
fn fake_ip_concurrent_reuse_is_unique_and_reversible() {
    use fluxa_core::dns::fake_ip::Table;
    let table = std::sync::Arc::new(Table::new(fake_config(32)).unwrap());
    let workers: Vec<_> = (0..4)
        .map(|_| {
            let table = table.clone();
            std::thread::spawn(move || {
                for index in 0..128 {
                    let name = format!("host{}.test", index % 16);
                    let (address, _) = table.allocate(&name, index % 2 == 0).unwrap().unwrap();
                    assert_eq!(
                        table.reverse(&address).unwrap().as_deref(),
                        Some(name.as_str())
                    );
                }
            })
        })
        .collect();
    for worker in workers {
        worker.join().unwrap();
    }
    let unique: std::collections::BTreeSet<_> = (0..16)
        .map(|index| {
            table
                .allocate(&format!("host{index}.test"), false)
                .unwrap()
                .unwrap()
                .0
        })
        .collect();
    assert_eq!(unique.len(), 16);
}
