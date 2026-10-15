//! 产品优先级、包含裁剪和独立参考匹配器的回归测试。
use libfluxa_core::routing::{
    Config, Match, Mode, Policy, Rule, Target, Transport,
    runtime::{Layer, Snapshot, Source},
};
fn rule(kind: Match, value: &str, policy: Policy) -> Rule {
    Rule {
        protocol: None,
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
    let before = std::fs::read(path.join("runtime-rules.bin")).unwrap();
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
        std::fs::read(path.join("runtime-rules.bin")).unwrap(),
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

#[test]
fn clash_payload_is_strict_uppercase_and_supports_both_address_families() {
    use libfluxa_core::subscription::parse_rules;
    let input = "payload:\n - DOMAIN,WWW.Example.COM.\n - DOMAIN-SUFFIX,example.com\n - DOMAIN-KEYWORD,Example\n - IP-CIDR,192.0.2.0/24\n - IP-CIDR,2001:0DB8:0000:0000:0000:0000:0000:0000/32\n";
    let rows = parse_rules(input, Policy::Reject, None).unwrap();
    assert_eq!(
        rows.iter().map(|r| r.value.as_str()).collect::<Vec<_>>(),
        [
            "www.example.com",
            "example.com",
            "example",
            "192.0.2.0/24",
            "2001:db8::/32"
        ]
    );
    assert!(rows.iter().all(|r| r.policy == Policy::Reject));
    assert!(
        parse_rules(input, Policy::Proxy, None)
            .unwrap()
            .iter()
            .all(|r| r.outbound.is_none())
    );
    for invalid in [
        "payload: [domain,example.com]",
        "payload: ['domain-suffix,example.com']",
        "payload: ['DOMAIN,example.com,REJECT']",
        "payload: ['IP-CIDR,2001:db8::/129']",
        "payload: ['IP-CIDR,192.0.2.1/33']",
        "payload: ['IP-CIDR6,::/0']",
        "payload: []\npayload: []",
        "payload: []\nunknown: true",
        "payload: ['DOMAIN,example.com']\n---\npayload: []",
    ] {
        assert!(
            parse_rules(invalid, Policy::Direct, None).is_err(),
            "{invalid}"
        );
    }
    let document = build(
        vec![rule(Match::Domain, "a.example.com", Policy::Direct)],
        vec![],
    )
    .to_yaml()
    .unwrap();
    assert!(Snapshot::from_yaml(&document.replace("DOMAIN", "domain")).is_err());
}

#[test]
fn binary_cache_reopens_merged_rules_and_migrates_only_internal_legacy_yaml() {
    use libfluxa_core::routing::store::Store;
    let path = std::env::temp_dir().join(format!("fluxa-binary-migration-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).unwrap();
    let snapshot = build(
        vec![rule(Match::Domain, "a.example.com", Policy::Reject)],
        vec![],
    );
    let legacy = snapshot.to_yaml().unwrap().replace("DOMAIN", "domain");
    std::fs::write(path.join("runtime-rules.yaml"), &legacy).unwrap();
    let store = Store::open(&path).unwrap();
    assert_eq!(store.snapshot().unwrap().document(), snapshot.document());
    let bytes = std::fs::read(path.join("runtime-rules.bin")).unwrap();
    assert!(bytes.starts_with(b"FLXARULE"));
    assert_eq!(
        std::fs::read_to_string(path.join("runtime-rules.yaml")).unwrap(),
        legacy
    );
    store.close().unwrap();
    let reopened = Store::open(&path).unwrap();
    assert_eq!(reopened.snapshot().unwrap().document(), snapshot.document());
    reopened.close().unwrap();
    let mut bytes = std::fs::read(path.join("runtime-rules.bin")).unwrap();
    bytes[8..12].copy_from_slice(&3u32.to_le_bytes());
    std::fs::write(path.join("runtime-rules.bin"), bytes).unwrap();
    // 已存在二进制缓存损坏或版本未知时不能回退旧文件，伪装成最新方案。
    assert!(Store::open(&path).is_err());
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn binary_cache_rejects_corruption_offsets_truncation_and_unknown_version() {
    use libfluxa_core::routing::store::Store;
    for mode in 0..6 {
        let path =
            std::env::temp_dir().join(format!("fluxa-binary-tamper-{}-{mode}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        let store = Store::open(&path).unwrap();
        store
            .update(
                Config::default(),
                vec![source(
                    "local",
                    Layer::Local,
                    vec![rule(Match::Domain, "example.com", Policy::Reject)],
                )],
            )
            .unwrap();
        store.close().unwrap();
        let file = path.join("runtime-rules.bin");
        let mut bytes = std::fs::read(&file).unwrap();
        match mode {
            0 => {
                let last = bytes.len() - 1;
                bytes[last] ^= 1;
            }
            1 => bytes[8..12].copy_from_slice(&3u32.to_le_bytes()),
            2 => {
                bytes.truncate(bytes.len() - 1);
            }
            3 => bytes.extend_from_slice(&[0]),
            4 => bytes[12..16].copy_from_slice(&u32::MAX.to_le_bytes()),
            _ => {
                // 重新计算校验和后仍须拒绝越界区段，校验和不能替代结构验证。
                let doc_size = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
                let index = 56 + doc_size;
                bytes[index..index + 4].copy_from_slice(&u32::MAX.to_le_bytes());
                let digest = ring::digest::digest(&ring::digest::SHA256, &bytes[56..]);
                bytes[24..56].copy_from_slice(digest.as_ref());
            }
        }
        std::fs::write(file, bytes).unwrap();
        assert!(Store::open(&path).is_err());
        std::fs::remove_dir_all(path).unwrap();
    }
}

#[test]
fn binary_cache_rejects_legacy_sqlite_without_silent_yaml_fallback() {
    use libfluxa_core::routing::store::Store;
    let path = std::env::temp_dir().join(format!("fluxa-legacy-sqlite-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).unwrap();
    std::fs::write(path.join("runtime-rules.sqlite"), b"SQLite format 3\0").unwrap();
    assert!(matches!(
        Store::open(&path),
        Err(libfluxa_core::error::Error::Unsupported)
    ));
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn binary_cache_matches_all_matcher_shapes_before_and_after_reopen() {
    use libfluxa_core::routing::store::Store;
    let path = std::env::temp_dir().join(format!("fluxa-flat-differential-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&path);
    let inputs = vec![source(
        "local",
        Layer::Local,
        vec![
            rule(Match::Domain, "exact.example", Policy::Direct),
            rule(Match::DomainSuffix, "suffix.example", Policy::Proxy),
            rule(Match::DomainKeyword, "abcd", Policy::Proxy),
            rule(Match::DomainKeyword, "bc", Policy::Reject),
            rule(Match::IpCidr, "192.0.2.0/24", Policy::Proxy),
            rule(Match::IpCidr, "2001:db8::/32", Policy::Direct),
            rule(Match::DestinationPort, "8000-9000", Policy::Proxy),
            rule(Match::Transport, "udp", Policy::Reject),
        ],
    )];
    let reference = Snapshot::generate(Config::default(), inputs.clone(), 1).unwrap();
    let store = Store::open(&path).unwrap();
    store.update(Config::default(), inputs).unwrap();
    let flat = store.snapshot().unwrap();
    store.close().unwrap();
    let store = Store::open(&path).unwrap();
    let reopened = store.snapshot().unwrap();
    for domain in [
        "exact.example",
        "suffix.example",
        "a.suffix.example",
        "xabcdx.test",
        "xbcy.test",
        "miss.example",
    ] {
        for address in ["192.0.2.7", "198.51.100.2", "2001:db8::7"] {
            for port in [443, 7999, 8000, 8999, 9000, 9001] {
                for transport in [Transport::Tcp, Transport::Udp] {
                    let target = Target {
                        domain: Some(domain.into()),
                        address: Some(address.into()),
                        port,
                        transport,
                    };
                    let expected = reference.evaluate_reference(&target).unwrap();
                    assert_eq!(flat.evaluate(&target).unwrap(), expected);
                    assert_eq!(reopened.evaluate(&target).unwrap(), expected);
                    assert_eq!(
                        reopened.evaluate_before_dns(&target).unwrap(),
                        Some(expected)
                    );
                }
            }
        }
    }
    store.close().unwrap();
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn generation_input_accepts_structured_and_clash_sources() {
    use libfluxa_core::routing::runtime::GenerationInput;
    let input = GenerationInput::from_yaml(
        "format_version: 1\nsettings: {mode: rules, fallback: direct}\nsources:\n  - id: local\n    layer: local\n    rules:\n      - {type: DOMAIN-KEYWORD, value: example, policy: direct}\n  - id: remote\n    layer: remote\n    policy: reject\n    payload_yaml: |\n      payload:\n        - DOMAIN,ads.example.com\n",
    );
    assert!(
        input.is_ok(),
        "{}",
        input.err().map(|e| e.to_string()).unwrap_or_default()
    );
}

#[test]
fn empty_clash_payload_still_validates_outbound_metadata() {
    for (policy, id) in [
        (Policy::Reject, "node"),
        (Policy::Direct, "node"),
        (Policy::Proxy, ""),
        (Policy::Proxy, "invalid id"),
    ] {
        assert!(
            libfluxa_core::subscription::parse_rules("payload: []", policy, Some(id.into()))
                .is_err()
        );
    }
}

// 审查回归：直接断言产品规范，再比较独立未裁剪实现，避免只有差分自洽。
#[test]
fn audit_primary_domain_and_ip_precede_local_port() {
    for (kind, value, name) in [
        (Match::Domain, "a.example.com", "a.example.com"),
        (Match::DomainSuffix, "example.com", "a.example.com"),
        (Match::DomainKeyword, "example", "a.example.com"),
        (Match::IpCidr, "192.0.2.0/24", "unmatched.test"),
        (Match::IpCidr, "2001:db8::/32", "unmatched.test"),
    ] {
        for local in [true, false] {
            let sources = vec![
                source(
                    "port",
                    if local { Layer::Local } else { Layer::Remote },
                    vec![rule(Match::DestinationPort, "443", Policy::Direct)],
                ),
                source(
                    "primary",
                    Layer::Remote,
                    vec![rule(kind, value, Policy::Proxy)],
                ),
            ];
            let reference = libfluxa_core::routing::runtime::Reference::compile(
                Config::default(),
                sources.clone(),
                1,
            )
            .unwrap();
            let snapshot = Snapshot::generate(Config::default(), sources, 1).unwrap();
            let request = target(
                name,
                if value.contains(':') {
                    "2001:db8::1"
                } else {
                    "192.0.2.1"
                },
            );
            let actual = snapshot.evaluate(&request).unwrap();
            assert_eq!(actual.policy, Policy::Proxy, "普通主规则不能被端口规则覆盖");
            assert_eq!(actual.origin, reference.evaluate(&request).unwrap().origin);
        }
    }
    let fallback = build(
        vec![rule(Match::DestinationPort, "443", Policy::Direct)],
        vec![],
    );
    assert_eq!(
        fallback
            .evaluate(&target("miss.test", "192.0.2.1"))
            .unwrap()
            .policy,
        Policy::Direct
    );
}

#[test]
fn audit_global_transport_only_allows_reject_and_quic_is_not_udp() {
    for protocol in ["tcp", "udp"] {
        for action in [Policy::Direct, Policy::Proxy] {
            let mut disabled = source(
                "disabled",
                Layer::Local,
                vec![rule(Match::Transport, protocol, action)],
            );
            disabled.enabled = false;
            assert!(matches!(
                Snapshot::generate(Config::default(), vec![disabled], 1),
                Err(libfluxa_core::error::Error::InvalidInput)
            ));

            assert!(matches!(
                Snapshot::generate(
                    Config::default(),
                    vec![source(
                        "local",
                        Layer::Local,
                        vec![rule(Match::Transport, protocol, action)]
                    )],
                    1
                ),
                Err(libfluxa_core::error::Error::InvalidInput)
            ));
        }
        let snapshot = build(
            vec![rule(Match::Transport, protocol, Policy::Reject)],
            vec![],
        );
        let mut request = target("example.com", "192.0.2.1");
        request.transport = if protocol == "udp" {
            Transport::Udp
        } else {
            Transport::Tcp
        };
        assert_eq!(snapshot.evaluate(&request).unwrap().policy, Policy::Reject);
    }
    let snapshot = build(vec![rule(Match::Transport, "quic", Policy::Reject)], vec![]);
    assert!(snapshot.blocks_quic());
    let mut request = target("example.com", "192.0.2.1");
    request.transport = Transport::Udp;
    assert_eq!(snapshot.evaluate(&request).unwrap().policy, Policy::Direct);
}

#[test]
fn audit_document_trust_requires_original_sources() {
    use libfluxa_core::routing::runtime::{RemovalReason, ReportVerification};
    let sources = vec![source(
        "local",
        Layer::Local,
        vec![
            rule(Match::DomainSuffix, "example.com", Policy::Direct),
            rule(Match::DomainSuffix, "a.example.com", Policy::Proxy),
        ],
    )];
    let snapshot = Snapshot::generate(Config::default(), sources.clone(), 1).unwrap();
    assert_eq!(
        snapshot.report_verification(),
        ReportVerification::VerifiedFromSources
    );
    let restored = Snapshot::from_yaml(&snapshot.to_yaml().unwrap()).unwrap();
    assert_eq!(
        restored.report_verification(),
        ReportVerification::StructuralOnly
    );
    restored.verify_against_sources(sources.clone()).unwrap();
    let mut fake = snapshot.document().clone();
    fake.report.removals[0].reason = RemovalReason::Conflict;
    fake.report.pruned -= 1;
    fake.report.conflicts += 1;
    // 数量仍一致并不证明删除理由；明确保持不可信，再由原始来源重算发现伪造。
    let loaded = Snapshot::from_yaml(&serde_json::to_string(&fake).unwrap()).unwrap();
    assert_eq!(
        loaded.report_verification(),
        ReportVerification::StructuralOnly
    );
    assert!(loaded.verify_against_sources(sources).is_err());
}

#[test]
fn audit_invalid_documents_are_rejected_with_low_sensitive_locations() {
    let snapshot = build(
        vec![
            rule(Match::Domain, "a.example.com", Policy::Direct),
            rule(Match::Domain, "b.example.com", Policy::Reject),
        ],
        vec![],
    );
    for mutation in 0..10 {
        let mut doc = snapshot.document().clone();
        match mutation {
            0 => doc.format_version = 999,
            1 => doc.sources.push(doc.sources[0].clone()),
            2 => doc.rules[0].origin.source = 63,
            3 => doc.rules[1].origin = doc.rules[0].origin.clone(),
            4 => doc.rules.swap(0, 1),
            5 => doc.report.original += 1,
            6 => doc.report.effective = 0,
            7 => {
                let origin = doc.rules[0].origin.clone();
                doc.rules[0].additional_origins.push(origin);
            }
            8 => doc.rules[0].rule.value = "INVALID DOMAIN".into(),
            _ => doc.rules[1].origin.sequence = 99,
        }
        let result = Snapshot::from_yaml_diagnosed(&serde_json::to_string(&doc).unwrap());
        let diagnostic = match result {
            Ok(_) => panic!("非法文档被接受：{mutation}"),
            Err(d) => d,
        };
        assert!(matches!(
            diagnostic.stage,
            "header" | "sources" | "rules" | "report" | "budget" | "removals"
        ));
        assert!(!format!("{diagnostic:?}").contains("example.com"));
    }
    // 原始文本层的未知字段和重复键仍需独立保留。
    assert!(Snapshot::from_yaml("version: 1\nversion: 2\n").is_err());
}

#[test]
fn audit_dns_before_after_matrix() {
    let snapshot = build(
        vec![rule(Match::DestinationPort, "443", Policy::Direct)],
        vec![
            rule(Match::Domain, "hit.test", Policy::Proxy),
            rule(Match::DomainKeyword, "blocked", Policy::Reject),
            rule(Match::IpCidr, "192.0.2.0/24", Policy::Reject),
            rule(Match::IpCidr, "2001:db8::/32", Policy::Proxy),
        ],
    );
    for name in ["hit.test", "blocked.test", "miss.test"] {
        let mut before = target(name, "192.0.2.1");
        before.address = None;
        let early = snapshot.evaluate_before_dns(&before).unwrap();
        if name == "miss.test" {
            assert!(early.is_none(), "端口直连不能在 DNS 前提前放行");
        }
        for ip in ["192.0.2.1", "2001:db8::1", "198.51.100.1"] {
            let full = snapshot.evaluate(&target(name, ip)).unwrap();
            if let Some(early) = &early {
                assert_eq!(
                    (early.policy, &early.outbound, &early.origin),
                    (full.policy, &full.outbound, &full.origin)
                );
            }
        }
    }
    assert_eq!(
        snapshot
            .evaluate(&target("miss.test", "192.0.2.1"))
            .unwrap()
            .policy,
        Policy::Reject
    );
    assert_eq!(
        snapshot
            .evaluate(&target("miss.test", "2001:db8::1"))
            .unwrap()
            .policy,
        Policy::Proxy
    );
}

#[test]
fn audit_resource_limits_reject_before_index_expansion() {
    use libfluxa_core::error::Error;
    let dense = (0..20_000)
        .map(|_| {
            rule(
                Match::DomainSuffix,
                &("a.".repeat(125) + "com"),
                Policy::Direct,
            )
        })
        .collect();
    assert!(matches!(
        Snapshot::generate(
            Config::default(),
            vec![source("dense", Layer::Local, dense)],
            1
        ),
        Err(Error::ResourceBudgetExceeded {actual,limit:536_870_912}) if actual>536_870_912
    ));
    let too_many = (0..100_001)
        .map(|_| rule(Match::Domain, "example.com", Policy::Direct))
        .collect();
    assert!(matches!(
        Snapshot::generate(
            Config::default(),
            vec![source("many", Layer::Local, too_many)],
            1
        ),
        Err(Error::ResourceLimit)
    ));
    let payload = "payload:\n".to_owned() + &" - DOMAIN,example.com\n".repeat(100_001);
    assert!(matches!(
        libfluxa_core::subscription::parse_rules(&payload, Policy::Direct, None),
        Err(Error::ResourceLimit)
    ));
    for invalid in [
        format!("payload: ['DOMAIN,{}']", "a".repeat(2048)),
        "x".repeat(8 * 1024 * 1024 + 1),
    ] {
        assert!(matches!(
            libfluxa_core::subscription::parse_rules(&invalid, Policy::Direct, None),
            Err(Error::ResourceLimit)
        ));
    }
    // 超限失败之后仍可正常生成，不能泄漏配额。
    assert_eq!(
        build(
            vec![rule(Match::Domain, "example.com", Policy::Reject)],
            vec![]
        )
        .document()
        .report
        .effective,
        1
    );
}

#[test]
fn audit_seeded_random_unpruned_differential_and_round_trip() {
    use libfluxa_core::routing::runtime::Reference;
    let mut seed = 0x12_2026_1012u64;
    let mut next = || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    for batch in 0..40 {
        let mut sources = vec![];
        for group in 0..4 {
            let mut rules = vec![];
            for _ in 0..80 {
                let n = next();
                let k = (n % 7) as usize;
                let policy = [Policy::Direct, Policy::Proxy, Policy::Reject][(n / 7 % 3) as usize];
                let value = match k {
                    0 => format!("d{}.example.com", n / 21 % 16),
                    1 => {
                        if n % 2 == 0 {
                            "example.com".into()
                        } else {
                            format!("d{}.example.com", n / 21 % 16)
                        }
                    }
                    2 => format!("10.{}.0.0/16", n / 21 % 8),
                    3 => format!("2001:db8:{:x}::/48", n / 21 % 8),
                    4 => format!("{}-{}", 400 + n % 40, 450 + n % 40),
                    5 => "tcp".into(),
                    _ => ["example", "d1", "d", "test"][(n / 21 % 4) as usize].into(),
                };
                let kind = [
                    Match::Domain,
                    Match::DomainSuffix,
                    Match::IpCidr,
                    Match::IpCidr,
                    Match::DestinationPort,
                    Match::Transport,
                    Match::DomainKeyword,
                ][k];
                let mut generated =
                    rule(kind, &value, if k == 5 { Policy::Reject } else { policy });
                if k < 2 && n % 3 == 0 {
                    generated.protocol = Some(if n % 2 == 0 {
                        Transport::Tcp
                    } else {
                        Transport::Udp
                    });
                }
                rules.push(generated);
            }
            sources.push(source(
                &format!("source-{group}"),
                if group % 2 == 0 {
                    Layer::Remote
                } else {
                    Layer::Local
                },
                rules,
            ));
        }
        let settings = Config {
            fallback: Policy::Direct,
            ..Config::default()
        };
        let reference = Reference::compile(settings.clone(), sources.clone(), batch + 1).unwrap();
        let path =
            std::env::temp_dir().join(format!("fluxa-flat-random-{}-{batch}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        let store = libfluxa_core::routing::store::Store::open(&path).unwrap();
        store.update(settings.clone(), sources.clone()).unwrap();
        store.close().unwrap();
        let store = libfluxa_core::routing::store::Store::open(&path).unwrap();
        let binary = store.snapshot().unwrap();
        let snapshot = Snapshot::generate(settings, sources, batch + 1).unwrap();
        let loaded = Snapshot::from_yaml(&snapshot.to_yaml().unwrap()).unwrap();
        for i in 0..150 {
            let n = next();
            let mut request = target(
                &format!(
                    "d{}.{}.com",
                    n % 20,
                    if i % 3 == 0 { "miss" } else { "example" }
                ),
                &if i % 2 == 0 {
                    format!("10.{}.1.1", n % 10)
                } else {
                    format!("2001:db8:{:x}::1", n % 10)
                },
            );
            if i % 5 == 0 {
                request.domain = None;
            }
            request.port = (400 + n % 100) as u16;
            request.transport = if i % 2 == 0 {
                Transport::Udp
            } else {
                Transport::Tcp
            };
            let actual = snapshot.evaluate(&request).unwrap();
            let expected = reference.evaluate(&request).unwrap();
            assert_eq!(
                (&actual.policy, &actual.outbound, &actual.origin),
                (&expected.policy, &expected.outbound, &expected.origin),
                "批次 {batch} 目标 {i}"
            );
            assert_eq!(actual, loaded.evaluate(&request).unwrap());
            let cached = binary.evaluate(&request).unwrap();
            assert_eq!(
                (&cached.policy, &cached.outbound, &cached.origin),
                (&expected.policy, &expected.outbound, &expected.origin)
            );
        }
        store.close().unwrap();
        std::fs::remove_dir_all(path).unwrap();
    }
}

#[test]
fn binary_cache_fault_injection_never_panics_with_recomputed_checksum() {
    use libfluxa_core::routing::store::Store;
    let path = std::env::temp_dir().join(format!("fluxa-flat-fault-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&path);
    let store = Store::open(&path).unwrap();
    let rules = (0..32)
        .map(|id| {
            rule(
                Match::DomainSuffix,
                &format!("s{id}.example.org"),
                Policy::Reject,
            )
        })
        .collect();
    store
        .update(
            Config::default(),
            vec![source("local", Layer::Local, rules)],
        )
        .unwrap();
    store.close().unwrap();
    let file = path.join("runtime-rules.bin");
    let original = std::fs::read(&file).unwrap();
    let doc_size = u32::from_le_bytes(original[12..16].try_into().unwrap()) as usize;
    let index_start = 56 + doc_size;
    for mutation in 0..256usize {
        let mut bytes = original.clone();
        let position = index_start + (mutation * 977) % (bytes.len() - index_start);
        bytes[position] ^= 1 << (mutation % 8);
        let digest = ring::digest::digest(&ring::digest::SHA256, &bytes[56..]);
        bytes[24..56].copy_from_slice(digest.as_ref());
        std::fs::write(&file, bytes).unwrap();
        let result = std::panic::catch_unwind(|| {
            if let Ok(store) = Store::open(&path) {
                let _ = store
                    .snapshot()
                    .unwrap()
                    .evaluate(&target("s17.example.org", "192.0.2.1"));
                store.close().unwrap();
            }
        });
        assert!(result.is_ok(), "故障样本 {mutation} 发生 panic");
    }
    std::fs::remove_dir_all(path).unwrap();
}

// 预期直接取自用户约定：正式拦截优先，域名+协议优先于普通域名；来源只在同类型比较。
#[test]
fn formal_domain_transport_matrix_and_roundtrip() {
    for kind in [Match::Domain, Match::DomainSuffix] {
        for transport in [Transport::Tcp, Transport::Udp] {
            let mut scoped = rule(kind, "example.com", Policy::Proxy);
            scoped.protocol = Some(transport);
            let mut other = scoped.clone();
            other.protocol = Some(if transport == Transport::Tcp {
                Transport::Udp
            } else {
                Transport::Tcp
            });
            other.policy = Policy::Direct;
            other.outbound = None;
            let snapshot = build(
                vec![rule(Match::Domain, "example.com", Policy::Direct)],
                vec![scoped.clone(), other, scoped],
            );
            assert_eq!(snapshot.document().report.duplicates, 1);
            assert_eq!(snapshot.document().rules.len(), 3);
            let loaded = Snapshot::from_yaml(&snapshot.to_yaml().unwrap()).unwrap();
            for value in [&snapshot, &loaded] {
                let mut query = target("example.com", "192.0.2.1");
                query.transport = transport;
                let chosen = value.evaluate(&query).unwrap();
                assert_eq!(chosen.policy, Policy::Proxy);
                assert_eq!(chosen.outbound.as_deref(), Some("test"));
                assert_eq!(chosen.origin.as_ref().unwrap().source, 1);
                assert_eq!(value.evaluate_reference(&query).unwrap(), chosen);
                assert_eq!(value.evaluate_before_dns(&query).unwrap(), Some(chosen));
                query.transport = if transport == Transport::Tcp {
                    Transport::Udp
                } else {
                    Transport::Tcp
                };
                assert_eq!(value.evaluate(&query).unwrap().policy, Policy::Direct);
            }
        }
    }
}

#[test]
fn mismatched_scoped_domain_does_not_skip_ip_or_early_dns() {
    let mut scoped = rule(Match::DomainSuffix, "example.com", Policy::Reject);
    scoped.protocol = Some(Transport::Udp);
    let snapshot = build(
        vec![scoped],
        vec![rule(Match::IpCidr, "192.0.2.0/24", Policy::Proxy)],
    );
    let query = target("api.example.com", "192.0.2.1");
    assert_eq!(snapshot.evaluate(&query).unwrap().policy, Policy::Proxy);
    assert_eq!(
        snapshot.evaluate(&query).unwrap(),
        snapshot.evaluate_reference(&query).unwrap()
    );
    let mut without_ip = query;
    without_ip.address = None;
    assert!(snapshot.evaluate_before_dns(&without_ip).unwrap().is_none());
    without_ip.transport = Transport::Udp;
    assert_eq!(
        snapshot
            .evaluate_before_dns(&without_ip)
            .unwrap()
            .unwrap()
            .policy,
        Policy::Reject
    );
}

#[test]
fn formal_reject_and_same_type_local_precedence_survive_scoped_conditions() {
    let mut local = rule(Match::DomainSuffix, "example.com", Policy::Direct);
    local.protocol = Some(Transport::Tcp);
    let mut remote = rule(Match::DomainSuffix, "api.example.com", Policy::Proxy);
    remote.protocol = Some(Transport::Tcp);
    let snapshot = build(vec![local.clone()], vec![remote]);
    assert_eq!(snapshot.document().report.pruned, 1);
    assert_eq!(
        snapshot
            .evaluate(&target("api.example.com", "192.0.2.1"))
            .unwrap()
            .policy,
        Policy::Direct
    );
    let snapshot = build(
        vec![local],
        vec![rule(Match::Domain, "api.example.com", Policy::Reject)],
    );
    assert_eq!(
        snapshot
            .evaluate(&target("api.example.com", "192.0.2.1"))
            .unwrap()
            .policy,
        Policy::Reject
    );
}

#[test]
fn scoped_rule_validation_is_strict_and_disabled_sources_cannot_hide_illegal_rules() {
    for kind in [
        Match::IpCidr,
        Match::DomainKeyword,
        Match::DestinationPort,
        Match::Transport,
    ] {
        let mut invalid = rule(kind, "tcp", Policy::Reject);
        invalid.protocol = Some(Transport::Tcp);
        let mut input = source("disabled", Layer::Local, vec![invalid]);
        input.enabled = false;
        assert!(Snapshot::generate(Config::default(), vec![input], 1).is_err());
    }
    for text in [
        r#"{"type":"DOMAIN","value":"example.com","policy":"direct","protocol":"quic"}"#,
        r#"{"type":"DOMAIN","value":"example.com","policy":"direct","protocol":"tcp","protocol":"udp"}"#,
        r#"{"type":"DOMAIN","value":"example.com","policy":"direct","protocol":"tcp","debug":true}"#,
    ] {
        assert!(serde_json::from_str::<Rule>(text).is_err());
    }
}

#[test]
fn scoped_flat_cache_retains_both_protocols_and_rejects_previous_layout() {
    use libfluxa_core::routing::store::Store;
    let path = std::env::temp_dir().join(format!("fluxa-scoped-cache-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&path);
    let mut tcp = rule(Match::Domain, "example.com", Policy::Direct);
    tcp.protocol = Some(Transport::Tcp);
    let mut udp = tcp.clone();
    udp.protocol = Some(Transport::Udp);
    udp.policy = Policy::Reject;
    let input = vec![source("local", Layer::Local, vec![tcp, udp])];
    let store = Store::open(&path).unwrap();
    store.update(Config::default(), input).unwrap();
    let old = store.snapshot().unwrap();
    store.close().unwrap();
    let reopened = Store::open(&path).unwrap();
    let flat = reopened.snapshot().unwrap();
    for transport in [Transport::Tcp, Transport::Udp] {
        let mut query = target("example.com", "192.0.2.1");
        query.transport = transport;
        let expected = if transport == Transport::Tcp {
            Policy::Direct
        } else {
            Policy::Reject
        };
        assert_eq!(flat.evaluate(&query).unwrap().policy, expected);
        assert_eq!(
            old.evaluate(&query).unwrap(),
            flat.evaluate(&query).unwrap()
        );
        assert_eq!(
            flat.evaluate(&query).unwrap(),
            flat.evaluate_reference(&query).unwrap()
        );
    }
    reopened.close().unwrap();
    let file = path.join("runtime-rules.bin");
    let mut bytes = std::fs::read(&file).unwrap();
    bytes[8..12].copy_from_slice(&1u32.to_le_bytes());
    std::fs::write(file, bytes).unwrap();
    assert!(Store::open(&path).is_err());
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn both_complete_yaml_examples_strictly_compile_formal_rules() {
    use libfluxa_core::routing::runtime::{GenerationInput, SourceInput};
    for full in [
        include_str!("../fluxa.yaml"),
        include_str!("../fluxa_english.yaml"),
    ] {
        // 完整示例的顶层由宿主配置层负责；这里用内核严格解析器验收整个 rules 序列，
        // 不用字符串替换伪造组合，也不把 JSON Value 解析成功当作规则合法证明。
        let body = full.split_once("\nrules:\n").unwrap().1;
        let block: Vec<_> = body
            .lines()
            .take_while(|line| line.is_empty() || line.starts_with([' ', '#']))
            .collect();
        let mut document = String::from(
            "format_version: 1\nsettings: {}\nsources:\n  - id: complete\n    layer: local\n    rules:\n",
        );
        for line in block {
            document.push_str("    ");
            document.push_str(line);
            document.push('\n');
        }
        let input = GenerationInput::from_yaml(&document).unwrap();
        let sources = input
            .sources
            .into_iter()
            .map(|source| match source {
                SourceInput::Rules(source) => source,
                SourceInput::Clash(_) => panic!("完整示例不能被替换成订阅"),
            })
            .collect();
        let snapshot = Snapshot::generate(input.settings, sources, 1).unwrap();
        assert!(
            snapshot
                .document()
                .rules
                .iter()
                .any(|row| row.rule.protocol == Some(Transport::Udp))
        );
    }
}

#[test]
fn duplicate_vec_capacity_is_accounted_without_false_rejection() {
    use libfluxa_core::routing::store::Store;
    let path =
        std::env::temp_dir().join(format!("fluxa-duplicate-capacity-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&path);
    let store = Store::open(&path).unwrap();
    let duplicate = rule(Match::Domain, "duplicate.example.net", Policy::Direct);
    store
        .update(
            Config::default(),
            vec![source("local", Layer::Local, vec![duplicate; 1000])],
        )
        .unwrap();
    let before = store.snapshot().unwrap();
    assert_eq!(before.document().report.original, 1000);
    assert_eq!(before.document().report.duplicates, 999);
    assert_eq!(before.document().rules.len(), 1);
    store.close().unwrap();
    let reopened = Store::open(&path).unwrap();
    let loaded = reopened.snapshot().unwrap();
    assert_eq!(before.document(), loaded.document());
    assert_eq!(
        loaded
            .evaluate(&target("duplicate.example.net", "192.0.2.1"))
            .unwrap()
            .policy,
        Policy::Direct
    );
    reopened.close().unwrap();
    std::fs::remove_dir_all(path).unwrap();
}
