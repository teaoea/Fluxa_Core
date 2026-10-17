//! 测试文件：tests/test_routing_runtime.rs
//! 测试类型：单元测试、迁移回归
//! 测试对象：src/routing/runtime.rs
//! 测试内容：
//! - process_facts_preserve_source_type_and_special_reject_contract：进程精确匹配、未知跳过、来源优先与特殊拦截。
//! - scoped_quic_requires_real_packet_and_preserves_priority_in_flat_index：实际报文、标签边界、来源和特殊 REJECT 在树/平铺/独立参考中保持一致。
//! - full_value_validation_detects_provenance_and_report_changes：依据现行独立业务约定验证；新来源/模式约定的输入与预期在同名函数明确说明。
//! - audit_migration_and_input_leases_are_bounded：依据现行独立业务约定验证；新来源/模式约定的输入与预期在同名函数明确说明。
//! - audit_registry_uses_weak_owners_and_releases_handles：依据现行独立业务约定验证；新来源/模式约定的输入与预期在同名函数明确说明。
//! - shared_pool_covers_compilation_flattening_cache_io_and_retained_snapshots：依据现行独立业务约定验证；新来源/模式约定的输入与预期在同名函数明确说明。
//! - rule_class_slots_never_define_decision_priority：依据现行独立业务约定验证；新来源/模式约定的输入与预期在同名函数明确说明。
//!
//! 测试依据：保留原测试的业务/格式/授权/资源约束；《测试文件与报告统一管理原则》，独立语义审查不以实现输出反推预期。
//!
//! 前置条件：本地测试需文件/回环套接字权限；平台限定及忽略项以实际登记为准。
//!
//! 执行状态：由同名 Markdown 报告记录，不在源码中固化通过结论。

#[cfg(test)]
mod validation_tests {
    use super::super::*;
    #[test]
    fn full_value_validation_detects_provenance_and_report_changes() {
        let snapshot = Snapshot::generate(
            Config::default(),
            vec![Source {
                id: "local".into(),
                layer: Layer::Local,
                enabled: true,
                rules: vec![Rule {
                    protocol: None,
                    r#type: Match::Domain,
                    value: "example.com".into(),
                    policy: Policy::Reject,
                    outbound: None,
                }],
            }],
            7,
        )
        .unwrap();
        let valid = snapshot.to_yaml().unwrap();
        snapshot.verify_yaml(&valid).unwrap();
        for field in 0..5 {
            let mut document = snapshot.document().clone();
            match field {
                0 => document.rules[0].rule.value = "different.com".into(),
                1 => document.version += 1,
                2 => document.rules[0].origin.sequence += 1,
                3 => document.report.effective = 0,
                _ => document.sources[0].id = "other".into(),
            }
            let invalid = serde_json::to_string(&document).unwrap();
            assert!(snapshot.verify_yaml(&invalid).is_err());
        }
    }
    #[test]
    fn audit_migration_and_input_leases_are_bounded() {
        assert!(matches!(
            Snapshot::migrate_yaml(&"x".repeat(yaml::MAX_BYTES + 1)),
            Err(Error::ResourceLimit)
        ));
        assert!(matches!(
            scaled(usize::MAX, 8, PARSE_BASE),
            Err(Error::ResourceLimit)
        ));
        let input =
            GenerationInput::from_yaml(r#"{"format_version":1,"settings":{},"sources":[]}"#)
                .unwrap();
        assert!(input._memory.is_some());
        drop(input);
        assert!(GenerationInput::from_yaml("unknown: true").is_err());
        assert!(
            GenerationInput::from_yaml(r#"{"format_version":1,"settings":{},"sources":[]}"#)
                .is_ok()
        );
    }
    #[test]
    fn audit_registry_uses_weak_owners_and_releases_handles() {
        for _ in 0..256 {
            let snapshot =
                std::sync::Arc::new(Snapshot::generate(Config::default(), vec![], 1).unwrap());
            let handle = snapshot.instance_handle();
            register(&snapshot).unwrap();
            let retained = lookup(handle).unwrap();
            drop(snapshot);
            assert!(lookup(handle).is_ok());
            drop(retained);
            assert!(matches!(lookup(handle), Err(Error::InvalidState)));
        }
    }
    #[test]
    fn shared_pool_covers_compilation_flattening_cache_io_and_retained_snapshots() {
        let baseline = memory_usage().reserved_bytes;
        let snapshot = Snapshot::generate(Config::default(), vec![], 1)
            .unwrap()
            .into_flat()
            .unwrap();
        let retained = memory_usage().reserved_bytes;
        assert!(retained > baseline);
        let occupied = MEMORY.reserve(MAX_MEMORY - retained).unwrap();
        assert!(matches!(
            Snapshot::generate(Config::default(), vec![], 2),
            Err(Error::ResourceBudgetExceeded { .. })
        ));
        assert!(matches!(
            Index::default().flatten(&[]),
            Err(Error::ResourceBudgetExceeded { .. })
        ));
        // 安全创建的文件也必须先通过共享预算准入；超限时不能写入任何字节。
        let path = std::env::temp_dir().join(format!(
            "fluxa-cache-budget-{}-{}",
            std::process::id(),
            crate::lifecycle::identifier().unwrap()
        ));
        let mut file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap();
        assert!(matches!(
            crate::routing::cache::write(&mut file, &snapshot),
            Err(Error::ResourceBudgetExceeded { .. })
        ));
        assert_eq!(file.metadata().unwrap().len(), 0);
        drop(file);
        std::fs::remove_file(path).unwrap();
        drop(occupied);
        assert_eq!(memory_usage().reserved_bytes, retained);
        drop(snapshot);
        assert_eq!(memory_usage().reserved_bytes, baseline);
        assert!(memory_usage().peak_reserved_bytes <= MAX_MEMORY);
    }
    #[test]
    fn rule_class_slots_never_define_decision_priority() {
        let classes = [
            RuleClass::Reject,
            RuleClass::LocalIp,
            RuleClass::LocalDomain,
            RuleClass::LocalDomainSuffix,
            RuleClass::LocalPortTransport,
            RuleClass::PluginRemoteIp,
            RuleClass::PluginRemoteDomain,
            RuleClass::PluginRemoteDomainSuffix,
            RuleClass::PluginRemotePortTransport,
            RuleClass::LocalKeyword,
            RuleClass::PluginRemoteKeyword,
            RuleClass::RejectTcp,
            RuleClass::RejectUdp,
            RuleClass::LocalDomainTcp,
            RuleClass::PluginRemoteDomainTcp,
            RuleClass::LocalDomainUdp,
            RuleClass::PluginRemoteDomainUdp,
            RuleClass::LocalSuffixTcp,
            RuleClass::PluginRemoteSuffixTcp,
            RuleClass::LocalSuffixUdp,
            RuleClass::PluginRemoteSuffixUdp,
        ];
        assert_eq!(classes.len(), RuleClass::COUNT);
        for (slot, class) in classes.into_iter().enumerate() {
            assert_eq!(slot, class as usize);
        }
        let rules = [
            (Match::IpCidr, "192.0.2.0/24"),
            (Match::Domain, "example.com"),
            (Match::DestinationPort, "443"),
        ]
        .into_iter()
        .map(|(kind, value)| Rule {
            protocol: None,
            r#type: kind,
            value: value.into(),
            policy: Policy::Direct,
            outbound: None,
        })
        .collect();
        let snapshot = Snapshot::generate(
            Config::default(),
            vec![Source {
                id: "local".into(),
                layer: Layer::Local,
                enabled: true,
                rules,
            }],
            1,
        )
        .unwrap();
        let mut values = Candidates::default();
        values.add(RuleClass::LocalIp as usize, 0);
        values.add(RuleClass::LocalDomain as usize, 1);
        assert_eq!(
            snapshot.selected(values),
            Some(1),
            "同来源 DOMAIN 优先于 IP，不能按槽位编号选择"
        );
        values.add(RuleClass::LocalPortTransport as usize, 2);
        assert_eq!(
            snapshot.selected(values),
            Some(2),
            "同来源 PORT 优先于 DOMAIN，编号更大的槽位也能先匹配"
        );
    }
}

#[test]
fn scoped_quic_requires_real_packet_and_preserves_priority_in_flat_index() {
    use super::*;
    let mut quic = vec![0xc0, 0, 0, 0, 1, 1, 1, 1, 2, 0, 17];
    quic.extend([0; 17]);
    let rule = |kind, value: &str, policy| Rule {
        protocol: None,
        r#type: kind,
        value: value.into(),
        policy,
        outbound: None,
    };
    let source = |layer, id: &str, rules| Source {
        id: id.into(),
        layer,
        enabled: true,
        rules,
    };
    let sources = vec![
        source(
            Layer::Local,
            "local",
            vec![rule(Match::DomainQuic, "example.com", Policy::Direct)],
        ),
        source(
            Layer::Remote,
            "remote",
            vec![rule(Match::Domain, "example.com", Policy::Reject)],
        ),
    ];
    let reference = Reference::compile(Config::default(), sources.clone(), 9).unwrap();
    let snapshot = Snapshot::generate(Config::default(), sources, 9).unwrap();
    let verify = |snapshot: &Snapshot| {
        for (domain, bytes, expected) in [
            ("a.example.com", quic.as_slice(), Policy::Direct),
            ("deep.a.example.com", quic.as_slice(), Policy::Direct),
            ("badexample.com", quic.as_slice(), Policy::Reject),
            ("example.com.evil", quic.as_slice(), Policy::Reject),
            ("example.com", quic.as_slice(), Policy::Reject), // exact 特殊 REJECT 高于正式协议规则。
            ("a.example.com", b"not-quic".as_slice(), Policy::Reject),
            ("a.example.com", &quic[..10], Policy::Reject),
        ] {
            let target = Target {
                domain: Some(domain.into()),
                address: Some("192.0.2.1".into()),
                port: 5555,
                transport: super::super::Transport::Udp,
            };
            let actual = snapshot.evaluate_datagram(&target, bytes).unwrap();
            assert_eq!(actual.policy, expected, "{domain}");
            assert_eq!(actual, reference.evaluate_datagram(&target, bytes).unwrap());
        }
    };
    verify(&snapshot);
    let flat = snapshot.into_flat().unwrap();
    verify(&flat);
    let restored =
        Snapshot::from_flat_document(flat.document.clone(), flat.flat_bytes().unwrap().to_vec())
            .unwrap();
    verify(&restored);
    let tcp = Target {
        domain: Some("a.example.com".into()),
        address: None,
        port: 443,
        transport: super::super::Transport::Tcp,
    };
    assert!(matches!(
        restored.evaluate_datagram(&tcp, &quic),
        Err(Error::InvalidInput)
    ));
}

#[test]
fn process_facts_preserve_source_type_and_special_reject_contract() {
    use super::*;
    let rule = |kind, value: &str, policy| Rule {
        protocol: None,
        r#type: kind,
        value: value.into(),
        policy,
        outbound: None,
    };
    let source = |layer, id: &str, rules| Source {
        id: id.into(),
        layer,
        enabled: true,
        rules,
    };
    let settings = Config::default();
    let sources = vec![
        source(
            Layer::Local,
            "local",
            vec![
                rule(Match::ProcessName, "App", Policy::Direct),
                rule(Match::DomainSuffix, "local.example", Policy::Direct),
            ],
        ),
        source(
            Layer::Plugin,
            "plugin",
            vec![rule(Match::ProcessName, "Plugin", Policy::Reject)],
        ),
        source(
            Layer::Remote,
            "remote",
            vec![
                rule(Match::Domain, "blocked.example", Policy::Reject),
                rule(Match::ProcessName, "Remote", Policy::Reject),
            ],
        ),
    ];
    let snapshot = Snapshot::generate(settings, sources, 11).unwrap();
    let verify = |snapshot: &Snapshot| {
        // 预期从已确认规范独立写出；进程并不绕过特殊 REJECT，来源又优先于进程类型。
        for (domain, process, expected) in [
            ("other.example", Some("App"), Policy::Direct),
            ("other.example", Some("app"), Policy::Reject),
            ("other.example", None, Policy::Reject),
            ("blocked.example", Some("App"), Policy::Reject),
            ("a.local.example", Some("Remote"), Policy::Direct),
            ("a.local.example", Some("Plugin"), Policy::Direct),
        ] {
            let target = Target {
                domain: Some(domain.into()),
                address: Some("192.0.2.1".into()),
                port: 443,
                transport: super::super::Transport::Tcp,
            };
            let actual = snapshot.evaluate_facts(&target, false, process).unwrap();
            assert_eq!(actual.policy, expected, "{domain} {process:?}");
            assert_eq!(
                actual,
                snapshot
                    .evaluate_reference_facts(&target, false, process)
                    .unwrap()
            );
        }
    };
    verify(&snapshot);
    let flat = snapshot.into_flat().unwrap();
    verify(&flat);
    let restored =
        Snapshot::from_flat_document(flat.document.clone(), flat.flat_bytes().unwrap().to_vec())
            .unwrap();
    verify(&restored);
    for mode in [Mode::Direct, Mode::Global] {
        let snapshot = Snapshot::generate(
            Config {
                mode,
                fallback: Policy::Direct,
                ..Config::default()
            },
            vec![source(
                Layer::Local,
                "local",
                vec![rule(Match::ProcessName, "App", Policy::Reject)],
            )],
            1,
        )
        .unwrap();
        let target = Target {
            domain: Some("other.example".into()),
            address: None,
            port: 443,
            transport: super::super::Transport::Tcp,
        };
        assert_eq!(
            snapshot
                .evaluate_facts(&target, false, Some("App"))
                .unwrap()
                .policy,
            Policy::Direct,
            "global/direct 不执行普通进程拒绝"
        );
    }
}
