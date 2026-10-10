//! 产品优先级、包含裁剪和独立参考匹配器的回归测试。
use libfluxa_core::routing::{
    Config, Match, Mode, Policy, Rule, Target, Transport,
    runtime::{Layer, Snapshot, Source},
};
fn rule(kind: Match, value: &str, policy: Policy) -> Rule {
    Rule {
        r#type: kind,
        value: value.into(),
        policy,
        outbound: if policy == Policy::Proxy {
            Some("test".into())
        } else {
            None
        },
    }
}
fn source(id: &str, layer: Layer, rules: Vec<Rule>) -> Source {
    Source {
        id: id.into(),
        layer,
        enabled: true,
        rules,
    }
}
fn target(name: &str, ip: &str) -> Target {
    Target {
        domain: Some(name.into()),
        address: Some(ip.into()),
        port: 443,
        transport: Transport::Tcp,
    }
}
fn build(local: Vec<Rule>, remote: Vec<Rule>) -> Snapshot {
    Snapshot::generate(
        Config {
            fallback: Policy::Direct,
            ..Config::default()
        },
        vec![
            source("local", Layer::Local, local),
            source("remote", Layer::Remote, remote),
        ],
        1,
    )
    .unwrap()
}
#[test]
fn rejects_survive_coverage_and_every_mode() {
    for mode in [Mode::Rules, Mode::Direct, Mode::Global] {
        let snapshot = Snapshot::generate(
            Config {
                mode,
                outbound: Some("test".into()),
                ..Config::default()
            },
            vec![
                source(
                    "local",
                    Layer::Local,
                    vec![rule(Match::IpCidr, "10.0.0.0/8", Policy::Direct)],
                ),
                source(
                    "remote",
                    Layer::Remote,
                    vec![
                        rule(Match::DomainSuffix, "ads.bbb.com", Policy::Reject),
                        rule(Match::IpCidr, "10.1.0.0/16", Policy::Reject),
                    ],
                ),
            ],
            1,
        )
        .unwrap();
        assert_eq!(snapshot.document().rules.len(), 3);
        assert_eq!(
            snapshot
                .evaluate(&target("ads.bbb.com", "10.2.1.1"))
                .unwrap()
                .policy,
            Policy::Reject
        );
        assert_eq!(
            snapshot
                .evaluate(&target("bbb.com", "10.1.1.1"))
                .unwrap()
                .policy,
            Policy::Reject
        );
    }
}
#[test]
fn domain_match_skips_ip_for_domain_target() {
    let local = vec![rule(Match::DomainSuffix, "bbb.com", Policy::Direct)];
    let remote = vec![rule(Match::IpCidr, "10.0.0.0/8", Policy::Proxy)];
    assert_eq!(
        build(local.clone(), remote)
            .evaluate(&target("aaa.bbb.com", "10.1.1.1"))
            .unwrap()
            .policy,
        Policy::Direct
    );
    let mut local = local;
    local.push(rule(Match::IpCidr, "10.0.0.0/8", Policy::Proxy));
    assert_eq!(
        build(local, vec![])
            .evaluate(&target("bbb.com", "10.1.1.1"))
            .unwrap()
            .policy,
        Policy::Direct
    );
}
#[test]
fn suffix_boundaries_cover_all_depths_and_rejects_are_not_pruned() {
    let snapshot = build(
        vec![rule(Match::DomainSuffix, "BBB.COM.", Policy::Direct)],
        vec![
            rule(Match::Domain, "aaa.bbb.com", Policy::Proxy),
            rule(Match::DomainSuffix, "aaa.bbb.com", Policy::Proxy),
            rule(Match::Domain, "ads.bbb.com", Policy::Reject),
            rule(Match::DomainSuffix, "ads.bbb.com", Policy::Reject),
            rule(Match::Domain, "badbbb.com", Policy::Proxy),
            rule(Match::Domain, "bbb.com.example.net", Policy::Proxy),
        ],
    );
    assert_eq!(snapshot.document().report.pruned, 1);
    for name in ["bbb.com", "x.aaa.bbb.com"] {
        assert_eq!(
            snapshot
                .evaluate(&target(name, "192.0.2.1"))
                .unwrap()
                .policy,
            Policy::Direct
        );
    }
    for name in ["ads.bbb.com", "x.ads.bbb.com"] {
        assert_eq!(
            snapshot
                .evaluate(&target(name, "192.0.2.1"))
                .unwrap()
                .policy,
            Policy::Reject
        );
    }
    for name in ["badbbb.com", "bbb.com.example.net"] {
        assert_eq!(
            snapshot
                .evaluate(&target(name, "192.0.2.1"))
                .unwrap()
                .policy,
            Policy::Proxy
        );
    }
}
#[test]
fn only_complete_nonreject_coverage_is_removed() {
    let snapshot = build(
        vec![
            rule(Match::IpCidr, "10.0.0.0/8", Policy::Direct),
            rule(Match::IpCidr, "10.1.0.0/16", Policy::Proxy),
            rule(Match::IpCidr, "10.2.0.0/16", Policy::Reject),
        ],
        vec![
            rule(Match::IpCidr, "10.3.0.0/16", Policy::Proxy),
            rule(Match::IpCidr, "0.0.0.0/0", Policy::Proxy),
            rule(Match::IpCidr, "10.0.0.0/8", Policy::Reject),
        ],
    );
    assert_eq!(snapshot.document().report.pruned, 2);
    assert_eq!(snapshot.document().rules.len(), 4);
    let reversed = build(
        vec![
            rule(Match::IpCidr, "10.1.0.0/16", Policy::Proxy),
            rule(Match::IpCidr, "10.0.0.0/8", Policy::Direct),
        ],
        vec![],
    );
    assert_eq!(reversed.document().rules.len(), 2);
    assert_eq!(
        reversed
            .evaluate(&target("bbb.com", "10.1.1.1"))
            .unwrap()
            .policy,
        Policy::Proxy
    );
    assert_eq!(
        reversed
            .evaluate(&target("bbb.com", "10.2.1.1"))
            .unwrap()
            .policy,
        Policy::Direct
    );
}
#[test]
fn duplicates_record_origins_and_conflicts_choose_first() {
    let snapshot = build(
        vec![
            rule(Match::Domain, "BBB.COM.", Policy::Proxy),
            rule(Match::Domain, "bbb.com", Policy::Direct),
            rule(Match::Domain, "ads.bbb.com", Policy::Reject),
        ],
        vec![
            rule(Match::Domain, "bbb.com", Policy::Proxy),
            rule(Match::Domain, "ADS.BBB.COM.", Policy::Reject),
        ],
    );
    assert_eq!(snapshot.document().report.duplicates, 2);
    assert_eq!(snapshot.document().report.conflicts, 1);
    assert_eq!(snapshot.document().rules[0].additional_origins.len(), 1);
    assert_eq!(snapshot.document().rules[1].additional_origins.len(), 1);
}
#[test]
fn regeneration_restores_pruned_and_disabled_sources_stay_remote() {
    let local = vec![rule(Match::DomainSuffix, "bbb.com", Policy::Direct)];
    let remote = vec![rule(Match::DomainSuffix, "aaa.bbb.com", Policy::Proxy)];
    assert_eq!(build(local, remote.clone()).document().rules.len(), 1);
    assert_eq!(
        build(vec![], remote)
            .evaluate(&target("aaa.bbb.com", "192.0.2.1"))
            .unwrap()
            .policy,
        Policy::Proxy
    );
    let mut disabled = source(
        "remote",
        Layer::Remote,
        vec![rule(Match::Domain, "aaa.bbb.com", Policy::Reject)],
    );
    disabled.enabled = false;
    assert_eq!(
        Snapshot::generate(
            Config {
                fallback: Policy::Direct,
                ..Config::default()
            },
            vec![disabled],
            2
        )
        .unwrap()
        .document()
        .rules
        .len(),
        0
    );
}
#[test]
fn domain_hit_decides_before_dns_and_domain_miss_waits() {
    let snapshot = build(
        vec![rule(Match::DomainSuffix, "bbb.com", Policy::Direct)],
        vec![
            rule(Match::IpCidr, "192.0.2.0/24", Policy::Reject),
            rule(Match::Domain, "ads.bbb.com", Policy::Reject),
        ],
    );
    let mut request = target("bbb.com", "192.0.2.1");
    request.address = None;
    assert_eq!(
        snapshot
            .evaluate_before_dns(&request)
            .unwrap()
            .unwrap()
            .policy,
        Policy::Direct
    );
    request.domain = Some("ads.bbb.com".into());
    assert_eq!(
        snapshot
            .evaluate_before_dns(&request)
            .unwrap()
            .unwrap()
            .policy,
        Policy::Reject
    );
}
#[test]
fn indexes_equal_reference_for_mixed_inputs_and_overlapping_ranges() {
    let mut local = vec![
        rule(Match::IpCidr, "2001:db8::/32", Policy::Direct),
        rule(Match::IpCidr, "2001:db8:1::/48", Policy::Proxy),
        rule(Match::DestinationPort, "400-500", Policy::Proxy),
        rule(Match::DestinationPort, "450-600", Policy::Reject),
        rule(Match::Transport, "udp", Policy::Proxy),
    ];
    let mut remote = vec![];
    for i in 0..1000 {
        local.push(rule(
            Match::Domain,
            &format!("d{i}.example.com"),
            Policy::Direct,
        ));
        remote.push(rule(
            Match::IpCidr,
            &format!("10.{}.{}.0/24", i / 256, i % 256),
            if i % 5 == 0 {
                Policy::Reject
            } else {
                Policy::Proxy
            },
        ));
    }
    let snapshot = build(local, remote);
    for i in 0..4000 {
        let mut request = target(
            &format!("d{}.example.com", i % 1100),
            &format!("10.{}.{}.1", (i % 1000) / 256, i % 256),
        );
        if i % 11 == 0 {
            request.address = Some("2001:db8:1::1".into());
        }
        request.port = (i % 1000 + 1) as u16;
        request.transport = if i % 3 == 0 {
            Transport::Udp
        } else {
            Transport::Tcp
        };
        assert_eq!(
            snapshot.evaluate(&request).unwrap(),
            snapshot.evaluate_reference(&request).unwrap()
        );
    }
}

#[test]
fn strict_yaml_round_trip_unknown_duplicate_alias_and_depth() {
    let snapshot = build(
        vec![rule(Match::DomainSuffix, "bbb.com", Policy::Direct)],
        vec![rule(Match::Domain, "ads.bbb.com", Policy::Reject)],
    );
    let yaml = snapshot.to_yaml().unwrap();
    let loaded = Snapshot::from_yaml(&yaml).unwrap();
    assert_eq!(
        loaded
            .evaluate(&target("ads.bbb.com", "192.0.2.1"))
            .unwrap(),
        snapshot
            .evaluate(&target("ads.bbb.com", "192.0.2.1"))
            .unwrap()
    );
    assert!(Snapshot::from_yaml(&(yaml.clone() + "unknown: true\n")).is_err());
    assert!(Snapshot::from_yaml(&(yaml.clone() + "version: 1\n")).is_err());
    assert!(
        Snapshot::from_yaml(&yaml.replace("\"format_version\": 1", "\"format_version\": 2"))
            .is_err()
    );
    assert!(Snapshot::from_yaml("a: &a [1]\nb: *a\n").is_err());
    assert!(Snapshot::from_yaml(&format!("{}1{}", "[".repeat(100), "]".repeat(100))).is_err());
}
#[test]
fn failed_update_preserves_disk_and_snapshot_and_close_releases_owner() {
    use libfluxa_core::routing::store::Store;
    let path = std::env::temp_dir().join(format!("fluxa-runtime-store-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&path);
    let store = Store::open(&path).unwrap();
    let settings = Config {
        fallback: Policy::Direct,
        ..Config::default()
    };
    assert_eq!(
        store
            .update(
                settings.clone(),
                vec![source(
                    "local",
                    Layer::Local,
                    vec![rule(Match::Domain, "bbb.com", Policy::Reject)]
                )]
            )
            .unwrap()
            .version,
        1
    );
    let retained = store.snapshot().unwrap();
    let before = std::fs::read(path.join("runtime-rules.yaml")).unwrap();
    assert!(
        store
            .update(
                settings.clone(),
                vec![source(
                    "local",
                    Layer::Local,
                    vec![rule(Match::IpCidr, "invalid", Policy::Direct)]
                )]
            )
            .is_err()
    );
    assert_eq!(
        std::fs::read(path.join("runtime-rules.yaml")).unwrap(),
        before
    );
    assert_eq!(store.snapshot().unwrap().document().version, 1);
    let reader = std::thread::spawn(move || {
        for _ in 0..1000 {
            assert_eq!(
                retained
                    .evaluate(&target("bbb.com", "192.0.2.1"))
                    .unwrap()
                    .policy,
                Policy::Reject
            );
        }
    });
    assert_eq!(store.update(settings, vec![]).unwrap().version, 2);
    reader.join().unwrap();
    assert_eq!(
        store
            .snapshot()
            .unwrap()
            .evaluate(&target("bbb.com", "192.0.2.1"))
            .unwrap()
            .policy,
        Policy::Direct
    );
    assert!(Store::open(&path).is_err());
    store.close().unwrap();
    let reopened = Store::open(&path).unwrap();
    assert_eq!(reopened.snapshot().unwrap().document().version, 2);
    store.close().unwrap();
    assert!(store.snapshot().is_err());
    assert!(store.update(Config::default(), vec![]).is_err());
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn port_partial_overlap_stays_and_full_coverage_is_pruned() {
    let snapshot = build(
        vec![
            rule(Match::DestinationPort, "100-200", Policy::Direct),
            rule(Match::DestinationPort, "150-250", Policy::Proxy),
            rule(Match::DestinationPort, "120-180", Policy::Proxy),
        ],
        vec![
            rule(Match::DestinationPort, "130-190", Policy::Proxy),
            rule(Match::DestinationPort, "190-300", Policy::Proxy),
            rule(Match::DestinationPort, "150-160", Policy::Reject),
        ],
    );
    assert_eq!(snapshot.document().report.pruned, 2);
    assert_eq!(snapshot.document().rules.len(), 4);
    for port in [99, 100, 149, 150, 160, 161, 200, 201, 250, 300, 301] {
        let mut request = target("example.net", "192.0.2.1");
        request.port = port;
        assert_eq!(
            snapshot.evaluate(&request).unwrap(),
            snapshot.evaluate_reference(&request).unwrap()
        );
    }
}

#[test]
fn exact_domain_precedes_earlier_suffix_across_source_layers() {
    let snapshot = build(
        vec![
            rule(Match::DomainSuffix, "bbb.com", Policy::Direct),
            rule(Match::Domain, "aaa.bbb.com", Policy::Proxy),
        ],
        vec![],
    );
    assert_eq!(snapshot.document().rules.len(), 2);
    assert_eq!(
        snapshot
            .evaluate(&target("aaa.bbb.com", "192.0.2.1"))
            .unwrap()
            .policy,
        Policy::Proxy
    );
    assert_eq!(
        snapshot
            .evaluate(&target("x.aaa.bbb.com", "192.0.2.1"))
            .unwrap()
            .policy,
        Policy::Direct
    );
    let across = build(
        vec![rule(Match::DomainSuffix, "bbb.com", Policy::Direct)],
        vec![rule(Match::Domain, "aaa.bbb.com", Policy::Proxy)],
    );
    assert_eq!(across.document().report.pruned, 0);
    assert_eq!(
        across
            .evaluate(&target("aaa.bbb.com", "192.0.2.1"))
            .unwrap()
            .policy,
        Policy::Proxy
    );
    let remote = build(
        vec![],
        vec![
            rule(Match::DomainSuffix, "bbb.com", Policy::Direct),
            rule(Match::Domain, "aaa.bbb.com", Policy::Proxy),
        ],
    );
    assert_eq!(remote.document().rules.len(), 2);
    let request = target("aaa.bbb.com", "192.0.2.1");
    assert_eq!(remote.evaluate(&request).unwrap().policy, Policy::Proxy);
    assert_eq!(
        remote.evaluate(&request).unwrap(),
        remote.evaluate_reference(&request).unwrap()
    );
}

#[test]
fn pruned_index_agrees_with_unpruned_reference_including_original_origin() {
    use libfluxa_core::routing::runtime::Reference;
    let settings = Config {
        fallback: Policy::Direct,
        ..Config::default()
    };
    let mut sources = vec![
        source(
            "local",
            Layer::Local,
            vec![
                rule(Match::DomainSuffix, "bbb.com", Policy::Direct),
                rule(Match::Domain, "a.bbb.com", Policy::Proxy),
                rule(Match::IpCidr, "10.0.0.0/8", Policy::Direct),
                rule(Match::DestinationPort, "100-200", Policy::Direct),
            ],
        ),
        source(
            "remote",
            Layer::Remote,
            vec![
                rule(Match::Domain, "c.bbb.com", Policy::Proxy),
                rule(Match::DomainSuffix, "ads.bbb.com", Policy::Reject),
                rule(Match::IpCidr, "10.1.0.0/16", Policy::Proxy),
                rule(Match::DestinationPort, "120-150", Policy::Proxy),
            ],
        ),
    ];
    for i in 0..300 {
        sources[1].rules.push(rule(
            Match::IpCidr,
            &format!("192.0.{}.0/24", i % 256),
            if i % 7 == 0 {
                Policy::Reject
            } else {
                Policy::Proxy
            },
        ));
    }
    let reference = Reference::compile(settings.clone(), sources.clone(), 1).unwrap();
    let snapshot = Snapshot::generate(settings, sources, 1).unwrap();
    for i in 0..2000 {
        let mut request = target(
            [
                "bbb.com",
                "a.bbb.com",
                "x.a.bbb.com",
                "c.bbb.com",
                "ads.bbb.com",
                "badbbb.com",
            ][i % 6],
            &format!("192.0.{}.1", i % 256),
        );
        if i % 3 == 0 {
            request.address = Some("10.1.1.1".into());
        }
        request.port = (i % 300 + 1) as u16;
        let actual = snapshot.evaluate(&request).unwrap();
        let expected = reference.evaluate(&request).unwrap();
        assert_eq!(
            (actual.policy, actual.outbound, actual.origin),
            (expected.policy, expected.outbound, expected.origin)
        );
    }
}

#[test]
fn keyword_reject_overrides_domain_actions_and_domain_hit_skips_ip() {
    let snapshot = build(
        vec![rule(Match::DomainKeyword, "example", Policy::Direct)],
        vec![
            rule(Match::Domain, "a.example.com", Policy::Proxy),
            rule(Match::IpCidr, "2001:db8::/32", Policy::Proxy),
            rule(Match::DomainKeyword, "adexample", Policy::Reject),
        ],
    );
    assert_eq!(snapshot.document().rules.len(), 4);
    for (name, ip, action) in [
        ("a.example.com", "192.0.2.1", Policy::Proxy),
        ("myexample.net", "2001:db8::1", Policy::Direct),
        ("myexample.net", "192.0.2.1", Policy::Direct),
        ("adexample.net", "2001:db8::1", Policy::Reject),
    ] {
        let request = target(name, ip);
        assert_eq!(snapshot.evaluate(&request).unwrap().policy, action);
        assert_eq!(
            snapshot.evaluate(&request).unwrap(),
            snapshot.evaluate_reference(&request).unwrap()
        );
    }
    let overlapping = build(
        vec![
            rule(Match::DomainKeyword, "ample", Policy::Proxy),
            rule(Match::DomainKeyword, "ex", Policy::Direct),
        ],
        vec![rule(Match::DomainKeyword, "example", Policy::Direct)],
    );
    assert_eq!(
        overlapping
            .evaluate(&target("example.com", "192.0.2.1"))
            .unwrap()
            .policy,
        Policy::Proxy
    );
}

#[test]
fn domain_branch_skips_ip_and_respects_domain_type_and_source_priority() {
    let kinds = [
        Match::IpCidr,
        Match::Domain,
        Match::DomainSuffix,
        Match::DomainKeyword,
    ];
    let values = ["2001:db8::/32", "a.example.com", "example.com", "example"];
    for first in 0..8 {
        for second in first + 1..8 {
            let mut sources = Vec::new();
            for (position, policy) in [(second, Policy::Direct), (first, Policy::Proxy)] {
                sources.push(source(
                    &format!("source-{position}"),
                    if position % 2 == 0 {
                        Layer::Local
                    } else {
                        Layer::Remote
                    },
                    vec![rule(kinds[position / 2], values[position / 2], policy)],
                ));
            }
            let reference = libfluxa_core::routing::runtime::Reference::compile(
                Config::default(),
                sources.clone(),
                1,
            )
            .unwrap();
            let snapshot = Snapshot::generate(Config::default(), sources, 1).unwrap();
            let request = target("a.example.com", "2001:db8::1");
            assert_eq!(
                snapshot.evaluate(&request).unwrap().policy,
                if first < 2 && second >= 2 {
                    Policy::Direct
                } else {
                    Policy::Proxy
                },
                "域名分支中优先级 {first} 与 {second}"
            );
            assert_eq!(
                snapshot.evaluate(&request).unwrap().origin,
                reference.evaluate(&request).unwrap().origin
            );
        }
    }
}

#[test]
fn domain_hit_skips_ip_reject_but_domain_miss_checks_ip_reject() {
    for kind in [Match::Domain, Match::DomainSuffix, Match::DomainKeyword] {
        let value = if kind == Match::DomainKeyword {
            "example"
        } else {
            "example.com"
        };
        let snapshot = build(
            vec![rule(kind, value, Policy::Direct)],
            vec![rule(Match::IpCidr, "192.0.2.0/24", Policy::Reject)],
        );
        assert_eq!(
            snapshot
                .evaluate(&target("example.com", "192.0.2.1"))
                .unwrap()
                .policy,
            Policy::Direct
        );
        assert_eq!(
            snapshot
                .evaluate(&target("different.test", "192.0.2.1"))
                .unwrap()
                .policy,
            Policy::Reject
        );
        let numeric = Target {
            domain: None,
            address: Some("192.0.2.1".into()),
            port: 443,
            transport: Transport::Tcp,
        };
        assert_eq!(snapshot.evaluate(&numeric).unwrap().policy, Policy::Reject);
    }
}
