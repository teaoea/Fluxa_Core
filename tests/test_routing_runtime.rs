//! 测试文件：tests/test_routing_runtime.rs
//! 测试类型：单元测试、迁移回归
//! 测试对象：src/routing/runtime.rs
//! 测试内容：
//! - full_value_validation_detects_provenance_and_report_changes：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - audit_migration_and_input_leases_are_bounded：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - audit_registry_uses_weak_owners_and_releases_handles：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - shared_pool_covers_compilation_flattening_cache_io_and_retained_snapshots：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - rule_class_slots_never_define_decision_priority：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
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
            RuleClass::RemoteIp,
            RuleClass::RemoteDomain,
            RuleClass::RemoteDomainSuffix,
            RuleClass::RemotePortTransport,
            RuleClass::LocalKeyword,
            RuleClass::RemoteKeyword,
            RuleClass::RejectTcp,
            RuleClass::RejectUdp,
            RuleClass::LocalDomainTcp,
            RuleClass::RemoteDomainTcp,
            RuleClass::LocalDomainUdp,
            RuleClass::RemoteDomainUdp,
            RuleClass::LocalSuffixTcp,
            RuleClass::RemoteSuffixTcp,
            RuleClass::LocalSuffixUdp,
            RuleClass::RemoteSuffixUdp,
        ];
        assert_eq!(classes.len(), RuleClass::COUNT);
        for (slot, class) in classes.into_iter().enumerate() {
            assert_eq!(slot, class as usize);
        }
        let mut values = Candidates::default();
        values.add(RuleClass::LocalDomain as usize, 10);
        values.add(RuleClass::RemoteIp as usize, 11);
        assert_eq!(
            Snapshot::selected(values),
            Some(11),
            "槽位2不能因为数值更小而覆盖槽位5的IP候选"
        );
        values.add(RuleClass::LocalPortTransport as usize, 1);
        assert_eq!(Snapshot::selected(values), Some(11), "端口规则不得越级");
    }
}
