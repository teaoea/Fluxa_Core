//! 路由是纯决策，验证匹配边界、失败原子性、共享快照和资源回收，不伪造连接成功。
use libfluxa_core::{error::Error, routing::*};
use std::sync::Arc;
fn rule(kind: Match, value: &str, policy: Policy) -> Rule {
    Rule {
        r#type: kind,
        value: value.into(),
        policy,
        outbound: None,
    }
}
fn target(domain: Option<&str>, address: Option<&str>, port: u16, transport: Transport) -> Target {
    Target {
        domain: domain.map(str::to_owned),
        address: address.map(str::to_owned),
        port,
        transport,
    }
}
fn compile(rules: Vec<Rule>) -> Router {
    Router::compile(Config {
        rules,
        outbound: Some("node-a".into()),
        ..Config::default()
    })
    .unwrap()
}
#[test]
fn exact_suffix_order_and_ascii_normalization() {
    let router = compile(vec![
        rule(Match::Domain, "API.Example.com.", Policy::Reject),
        rule(Match::DomainSuffix, "example.com", Policy::Proxy),
    ]);
    for name in ["api.example.com", "API.EXAMPLE.COM."] {
        let answer = router
            .evaluate(target(Some(name), None, 443, Transport::Tcp))
            .unwrap();
        assert_eq!(
            (answer.policy, answer.rule_index, answer.reason),
            (Policy::Reject, Some(0), Reason::Rule)
        );
    }
    for name in ["example.com", "a.example.com", "a.b.example.com"] {
        let answer = router
            .evaluate(target(Some(name), None, 443, Transport::Tcp))
            .unwrap();
        assert_eq!(answer.policy, Policy::Proxy);
        assert_eq!(answer.outbound.as_deref(), Some("node-a"));
    }
    for name in ["notexample.com", "example.com.evil", "api.notexample.com"] {
        let answer = router
            .evaluate(target(Some(name), None, 443, Transport::Tcp))
            .unwrap();
        assert_eq!(answer.reason, Reason::Fallback);
        assert_eq!(answer.policy, Policy::Reject);
    }
    assert!(
        compile(vec![rule(Match::Domain, "xn--fiqs8s.test", Policy::Direct)])
            .evaluate(target(Some("XN--FIQS8S.TEST"), None, 80, Transport::Tcp))
            .unwrap()
            .check_support()
            .is_ok()
    );
}
#[test]
fn ipv4_ipv6_prefix_boundaries_and_absent_metadata() {
    for (cidr, inside, outside) in [
        ("192.0.2.0/24", "192.0.2.255", "192.0.3.0"),
        ("2001:db8::/32", "2001:db8:ffff::1", "2001:db9::1"),
        ("::1/128", "::1", "::2"),
        ("0.0.0.0/0", "255.255.255.255", "::1"),
    ] {
        let router = compile(vec![rule(Match::IpCidr, cidr, Policy::Direct)]);
        assert_eq!(
            router
                .evaluate(target(None, Some(inside), 443, Transport::Tcp))
                .unwrap()
                .policy,
            Policy::Direct
        );
        assert_eq!(
            router
                .evaluate(target(None, Some(outside), 443, Transport::Tcp))
                .unwrap()
                .policy,
            Policy::Reject
        );
        assert_eq!(
            router
                .evaluate(target(Some("example.com"), None, 443, Transport::Tcp))
                .unwrap()
                .reason,
            Reason::Fallback
        );
    }
    let router = compile(vec![
        rule(Match::Domain, "example.com", Policy::Direct),
        rule(Match::IpCidr, "192.0.2.0/24", Policy::Reject),
    ]);
    assert_eq!(
        router
            .evaluate(target(
                Some("example.com"),
                Some("192.0.2.1"),
                443,
                Transport::Tcp
            ))
            .unwrap()
            .rule_index,
        Some(0)
    );
}
#[test]
fn port_range_transport_and_priority() {
    let router = compile(vec![
        rule(Match::DestinationPort, "80-443", Policy::Direct),
        rule(Match::Transport, "udp", Policy::Proxy),
    ]);
    for port in [80, 443] {
        assert_eq!(
            router
                .evaluate(target(Some("example.com"), None, port, Transport::Udp))
                .unwrap()
                .policy,
            Policy::Direct
        );
    }
    assert_eq!(
        router
            .evaluate(target(None, Some("192.0.2.1"), 53, Transport::Udp))
            .unwrap()
            .policy,
        Policy::Proxy
    );
    assert_eq!(
        router
            .evaluate(target(None, Some("192.0.2.1"), 444, Transport::Tcp))
            .unwrap()
            .policy,
        Policy::Reject
    );
    assert_eq!(
        compile(vec![rule(Match::DestinationPort, "65535", Policy::Direct)])
            .evaluate(target(Some("example.com"), None, 65535, Transport::Tcp))
            .unwrap()
            .policy,
        Policy::Direct
    );
}
#[test]
fn modes_fallback_named_outbound_and_explicit_support() {
    for (mode, expected) in [
        (Mode::Direct, Policy::Direct),
        (Mode::Global, Policy::Proxy),
    ] {
        let router = Router::compile(Config {
            mode,
            outbound: Some("node-a".into()),
            rules: vec![rule(Match::Transport, "tcp", Policy::Reject)],
            ..Config::default()
        })
        .unwrap();
        let answer = router
            .evaluate(target(Some("example.com"), None, 443, Transport::Tcp))
            .unwrap();
        assert_eq!(answer.policy, expected);
        assert_eq!(answer.reason, Reason::Mode);
        assert_eq!(
            answer.check_support(),
            if mode == Mode::Direct {
                Ok(())
            } else {
                Err(Error::Unsupported)
            }
        );
    }
    let mut specific = rule(Match::Domain, "example.com", Policy::Proxy);
    specific.outbound = Some("node-b".into());
    let answer = compile(vec![specific])
        .evaluate(target(Some("example.com"), None, 443, Transport::Tcp))
        .unwrap();
    assert_eq!(answer.outbound.as_deref(), Some("node-b"));
    assert_eq!(answer.check_support(), Err(Error::Unsupported));
    let answer = compile(vec![])
        .evaluate(target(Some("example.com"), None, 443, Transport::Tcp))
        .unwrap();
    assert_eq!(answer.check_support(), Err(Error::Rejected));
    let router = Router::compile(Config {
        fallback: Policy::Proxy,
        outbound: Some("fallback-node".into()),
        ..Config::default()
    })
    .unwrap();
    let answer = router
        .evaluate(target(Some("example.com"), None, 443, Transport::Tcp))
        .unwrap();
    assert_eq!(answer.reason, Reason::Fallback);
    assert_eq!(answer.outbound.as_deref(), Some("fallback-node"));
}
#[test]
fn malformed_rules_targets_and_unknown_fields_rejected() {
    for (kind, value) in [
        (Match::Domain, ".example.com"),
        (Match::Domain, "example..com"),
        (Match::Domain, "https://example.com"),
        (Match::Domain, "192.0.2.1"),
        (Match::Domain, "-bad.com"),
        (Match::Domain, "中文.test"),
        (Match::IpCidr, "192.0.2.1/24"),
        (Match::IpCidr, "::1/64"),
        (Match::IpCidr, "::/129"),
        (Match::DestinationPort, "0"),
        (Match::DestinationPort, "443-80"),
        (Match::DestinationPort, "65536"),
        (Match::DestinationPort, "+80"),
        (Match::DestinationPort, "80-90-100"),
        (Match::Transport, "quic"),
    ] {
        assert!(
            Router::compile(Config {
                rules: vec![rule(kind, value, Policy::Direct)],
                ..Config::default()
            })
            .is_err(),
            "{value}"
        );
    }
    assert!(
        Router::compile(Config {
            mode: Mode::Global,
            ..Config::default()
        })
        .is_err()
    );
    assert!(
        Router::compile(Config {
            rules: vec![rule(Match::Domain, "example.com", Policy::Proxy)],
            ..Config::default()
        })
        .is_err()
    );
    let mut invalid = rule(Match::Domain, "example.com", Policy::Direct);
    invalid.outbound = Some("node-a".into());
    assert!(
        Router::compile(Config {
            rules: vec![invalid],
            ..Config::default()
        })
        .is_err()
    );
    for outbound in ["", "node/token", "user:secret", "has space"] {
        assert!(
            Router::compile(Config {
                outbound: Some(outbound.into()),
                ..Config::default()
            })
            .is_err()
        );
    }
    let router = compile(vec![]);
    for target in [
        target(None, None, 443, Transport::Tcp),
        target(Some("example.com"), None, 0, Transport::Tcp),
        target(None, Some("bad-ip"), 443, Transport::Tcp),
        target(Some("bad..domain"), Some("192.0.2.1"), 443, Transport::Tcp),
    ] {
        assert!(router.evaluate(target).is_err());
    }
    assert!(serde_json::from_str::<Config>(r#"{"unknown":true}"#).is_err());
    assert!(
        serde_json::from_str::<Target>(r#"{"address":"192.0.2.1","port":65536,"transport":"tcp"}"#)
            .is_err()
    );
}
#[test]
fn immutable_snapshot_is_shared_without_state_between_connections() {
    let router = Arc::new(compile(vec![rule(
        Match::DomainSuffix,
        "example.com",
        Policy::Direct,
    )]));
    let jobs: Vec<_> = (0..8)
        .map(|_| {
            let router = router.clone();
            std::thread::spawn(move || {
                for _ in 0..100 {
                    assert_eq!(
                        router
                            .evaluate(target(Some("a.example.com"), None, 443, Transport::Tcp))
                            .unwrap()
                            .policy,
                        Policy::Direct
                    );
                }
            })
        })
        .collect();
    for job in jobs {
        job.join().unwrap();
    }
    assert_eq!(
        router
            .evaluate(target(Some("other.test"), None, 443, Transport::Tcp))
            .unwrap()
            .policy,
        Policy::Reject
    );
}
#[test]
fn limits_and_last_owner_return_global_budget() {
    let rules = vec![rule(Match::Domain, "example.com", Policy::Direct); 10000];
    let config = Config {
        rules: rules.clone(),
        ..Config::default()
    };
    let router = Arc::new(Router::compile(config.clone()).unwrap());
    let retained = router.clone();
    drop(router);
    assert!(matches!(
        Router::compile(config.clone()),
        Err(Error::ResourceLimit)
    ));
    drop(retained);
    assert!(Router::compile(config).is_ok());
    assert!(matches!(
        Router::compile(Config {
            rules: vec![rule(Match::Domain, "example.com", Policy::Direct); 10001],
            ..Config::default()
        }),
        Err(Error::ResourceLimit)
    ));
}
#[test]
fn arbitrary_short_values_do_not_panic() {
    for byte in 0u8..=127 {
        let value = String::from_utf8(vec![byte; 16]).unwrap();
        for kind in [
            Match::Domain,
            Match::IpCidr,
            Match::DestinationPort,
            Match::Transport,
        ] {
            let _ = Router::compile(Config {
                rules: vec![rule(kind, &value, Policy::Direct)],
                ..Config::default()
            });
        }
    }
}
