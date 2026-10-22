//! 测试文件：tests/test_error_contract.rs
//! 测试类型：集成测试、回归测试
//! 测试对象：src/error.rs
//! 测试内容：
//! - registered_contract_and_initial_baseline_are_complete：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - strict_error_schema_rejects_unregistered_or_unbounded_data：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - embedded_kernel_failure_survives_io_wrapper_without_sensitive_text：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - serialization_failure_remains_a_failure：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - fixed_integer_protocol_has_no_string_or_heap_representation：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - real_dns_deadline_and_closed_port_fail_without_policy_success：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - nested_and_cyclic_io_causes_preserve_classification_and_remain_bounded：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - all_abi_failures_use_segmented_codes_and_nonerrors_remain_states：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - every_internal_error_maps_to_the_independent_protocol_identity：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - invalid_config_keeps_snapshot_and_machine_failure：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - diagnostics_remain_fixed_size_and_do_not_hold_error_chains：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - segmented_codes_have_one_category_per_prefix：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - typed_context_is_required_bounded_and_specific_to_code：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - safety_filter_preserves_unknown_and_duplicate_classification：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//!
//! 测试依据：保留原有公开接口契约及全部断言；测试文件原注释保留在下方，L3 必须独立核对。
//!
//! 前置条件：本地测试需文件/回环套接字权限；平台限定及忽略项以实际登记为准。
//!
//! 执行状态：由同名 Markdown 报告记录，不在源码中固化通过结论。

// 一致性检查核对契约，语义检查采用独立确认的业务预期；不由当前实现生成断言。
use nodus_core::{
    error::{Error, ErrorCode, ErrorInfo},
    routing::{Config, Policy, Transport},
};
use std::collections::HashSet;
// 改名只迁移源码定位中的 C 符号；历史基线文件保持原文，业务字段逐字比较。
fn renamed_source(entry: &serde_json::Value) -> serde_json::Value {
    let mut entry = entry.clone();
    if entry["source"] == "src/ffi.rs fluxa_engine_command" {
        entry["source"] = "src/ffi.rs nodus_core_command".into();
    }
    entry
}
#[test]
fn registered_contract_and_initial_baseline_are_complete() {
    let current: Vec<_> = ErrorCode::ALL.iter().map(|code| code.contract()).collect();
    let json = serde_json::to_value(current).unwrap();
    let registered: serde_json::Value =
        serde_json::from_str(include_str!("../Documentation/error-codes.json")).unwrap();
    let baseline: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/error-contract-v1.json")).unwrap();
    assert_eq!(
        json, registered,
        "代码、分类、参数 Schema 与机器契约必须一致"
    );
    let published: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/error-contract-v2.json")).unwrap();
    for entry in published.as_array().unwrap() {
        assert!(
            registered
                .as_array()
                .unwrap()
                .contains(&renamed_source(entry)),
            "新方案已登记语义不得静默删除、修改或复用"
        );
    }
    let geoip_published: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/error-contract-v2.4.1.json")).unwrap();
    for entry in geoip_published.as_array().unwrap() {
        assert!(
            registered
                .as_array()
                .unwrap()
                .contains(&renamed_source(entry)),
            "v2.4.1 契约不得静默改义、改号或删除"
        );
    }
    let mut seen = HashSet::new();
    let document = include_str!("../Documentation/错误代码查询表.md");
    for code in ErrorCode::ALL {
        // 新增代码也必须带说明性中文注释，不能只登记名称而遗漏源码契约。
        let source = include_str!("../src/error.rs");
        let declaration = format!("\n    {code:?} =>");
        let position = source.find(&declaration).expect("公开代码必须集中定义");
        let comment = source[..position].lines().last().unwrap().trim();
        assert!(
            comment.starts_with("/// ")
                && comment
                    .chars()
                    .any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c)),
            "每个代码必须有中文注释"
        );
        assert!(seen.insert(code.value()));
        let row = code.contract();
        for field in [
            row.name,
            row.modules,
            row.category,
            row.detail,
            row.trigger,
            row.effect,
            row.params,
            row.source,
            row.status,
        ] {
            assert!(!field.trim().is_empty());
        }
        assert!(document.contains(&format!(
            "| `{}` | {} | {} | {} | {} | {} | {} | {} | {} | {} |",
            row.code,
            row.name,
            row.modules,
            row.category,
            row.detail,
            row.trigger,
            row.effect,
            row.params,
            row.source,
            row.status
        )));
        assert_eq!(document.matches(&format!("| `{}` |", row.code)).count(), 1);
        let mut info = ErrorInfo::new(*code);
        use nodus_core::error::FieldName;
        match code {
            ErrorCode::ConfigUnknownField | ErrorCode::ConfigDuplicateField => {
                info.params.field = FieldName::new("debug")
            }
            ErrorCode::ConfigUnsupportedVersion => {
                info.params.actual = Some(2);
                info.params.expected = Some(1);
            }
            ErrorCode::DebugInvalidLifetime => info.params.limit = Some(3600),
            ErrorCode::ResourceBudgetExceeded => {
                info.params.actual = Some(3);
                info.params.limit = Some(2);
            }
            _ => (),
        }
        let value: ErrorInfo = serde_json::from_slice(&serde_json::to_vec(&info).unwrap()).unwrap();
        assert_eq!(value.code, *code);
        assert_eq!(serde_json::to_value(value).unwrap()["code"], code.value());
    }
    let rows = document
        .lines()
        .filter(|line| line.starts_with("| `"))
        .count();
    assert_eq!(rows, ErrorCode::ALL.len(), "文档不得虚构代码");
    // 历史冻结记录保留用于迁移审查；用户已明确批准所有旧接口切换新方案。
    // 检查旧业务类别没有消失，迁移不把历史记录伪装成新协议基线。
    assert_eq!(baseline.as_array().unwrap().len(), 20);
    for old in baseline.as_array().unwrap() {
        assert!(
            ErrorCode::ALL
                .iter()
                .any(|code| code.contract().name == old["name"].as_str().unwrap()),
            "迁移不能丢失旧错误的业务类别"
        );
    }
}
#[test]
fn strict_error_schema_rejects_unregistered_or_unbounded_data() {
    for invalid in [
        r#"[1001,{}]"#,
        r#"{"code":1001,"params":[]}"#,
        r#"{"code":1001,"params":null}"#,
        r#"{"code":1001}"#,
        r#"{"params":{}}"#,
        r#"{"code":65535,"params":{}}"#,
        r#"{"code":1001,"params":{"token":"secret"}}"#,
        r#"{"code":1001,"params":{},"message":"secret"}"#,
        r#"{"code":1001,"code":1007,"params":{}}"#,
    ] {
        assert_eq!(
            ErrorInfo::from_json(invalid.as_bytes()),
            Err(Error::InvalidInput)
        );
    }
    assert_eq!(
        ErrorInfo::from_json(&vec![b' '; 513]),
        Err(Error::ResourceLimit)
    );
}
#[test]
fn embedded_kernel_failure_survives_io_wrapper_without_sensitive_text() {
    let wrapped = std::io::Error::other(Error::Rejected);
    assert_eq!(Error::from(wrapped), Error::Rejected);
    let secret = std::io::Error::new(
        std::io::ErrorKind::PermissionDenied,
        "token=do-not-publish /private/key.pem",
    );
    let public = serde_json::to_string(&Error::from(secret).public_info()).unwrap();
    assert_eq!(public, r#"{"code":6001,"params":{}}"#);
    assert!(!public.contains("token") && !public.contains("private"));
}

#[test]
fn serialization_failure_remains_a_failure() {
    struct Deny;
    impl std::io::Write for Deny {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("拒绝写入"))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    assert!(serde_json::to_writer(Deny, &Error::ResourceLimit.public_info()).is_err());
}

#[test]
fn fixed_integer_protocol_has_no_string_or_heap_representation() {
    assert_eq!(std::mem::size_of::<ErrorCode>(), 2);
    assert!(
        std::mem::size_of::<ErrorInfo>() <= 128,
        "上下文固定有界且无堆指针"
    );
    assert_eq!(ErrorCode::from_value(1001), Some(ErrorCode::InvalidInput));
    assert_eq!(ErrorCode::from_value(0), None);
    for invalid in [
        r#"{"code":"1001","params":{}}"#,
        r#"{"code":-1,"params":{}}"#,
        r#"{"code":65536,"params":{}}"#,
        r#"{"code":1001.0,"params":{}}"#,
    ] {
        assert_eq!(
            ErrorInfo::from_json(invalid.as_bytes()),
            Err(Error::InvalidInput)
        );
    }
}

#[tokio::test]
async fn real_dns_deadline_and_closed_port_fail_without_policy_success() {
    use nodus_core::dns::{Config as DnsConfig, Query, Resolver, Server};
    // 独立上游只占用 UDP 端口、不回复，预期来自“超时必须失败”契约。
    let upstream = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
    let resolver = Resolver::new(DnsConfig {
        servers: vec![Server::Udp {
            address: upstream.local_addr().unwrap().to_string(),
        }],
        timeout_ms: 100,
        dnssec: false,
        fake_ip: None,
        service: None,
    })
    .unwrap();
    let error = resolver
        .resolve(Query {
            name: "example.test".into(),
            query_type: "A".into(),
        })
        .await
        .unwrap_err();
    assert_eq!(error.code(), ErrorCode::DnsTimeout);
    // 已释放的本地 TCP 监听端口必须连接失败，不制造 DIRECT 成功连接。
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let target = listener.local_addr().unwrap().to_string();
    drop(listener);
    let manager = nodus_core::outbound::Manager::default();
    let result = manager
        .open(
            target.parse().unwrap(),
            Transport::Tcp,
            Policy::Direct,
            1000,
        )
        .await;
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().code(), ErrorCode::NetworkConnectRefused);
}

#[test]
fn nested_and_cyclic_io_causes_preserve_classification_and_remain_bounded() {
    #[derive(Debug)]
    struct Wrapper(Error);
    impl std::fmt::Display for Wrapper {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("受控测试包装")
        }
    }
    impl std::error::Error for Wrapper {
        fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
            Some(&self.0)
        }
    }
    assert_eq!(
        Error::from(std::io::Error::other(Wrapper(Error::ProtocolViolation))),
        Error::ProtocolViolation
    );
    assert_eq!(
        Error::from(std::io::Error::from(std::io::ErrorKind::TimedOut)).code(),
        ErrorCode::NetworkTimeout
    );
    assert_eq!(
        Error::from(std::io::Error::from(std::io::ErrorKind::OutOfMemory)).code(),
        ErrorCode::ResourceLimit
    );
    #[derive(Debug)]
    struct Cycle;
    impl std::fmt::Display for Cycle {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("不公开链内容")
        }
    }
    impl std::error::Error for Cycle {
        fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
            Some(self)
        }
    }
    assert_eq!(Error::from(std::io::Error::other(Cycle)), Error::Network);
}
#[test]
fn all_abi_failures_use_segmented_codes_and_nonerrors_remain_states() {
    use nodus_core::error::AbiStatus;
    assert_eq!(AbiStatus::Success.value(), 0);
    assert_eq!(AbiStatus::Empty.value(), 11);
    for (error, code) in [
        (Error::InvalidInput, 1001),
        (Error::ResourceLimit, 8001),
        (Error::InvalidState, 9001),
        (Error::Cancelled, 9002),
        (Error::VpnPreparationTimeout, 9003),
        (Error::Network, 6001),
        (Error::Rejected, 1101),
        (Error::ProtocolViolation, 1401),
        (Error::DnsTimeout, 5001),
        (Error::NetworkTimeout, 6002),
    ] {
        assert_eq!(error.code().value(), code);
    }
}

#[test]
fn every_internal_error_maps_to_the_independent_protocol_identity() {
    // 预期来自首版协议登记，不通过生产 match 或枚举下标推导。
    for (error, expected) in [
        (Error::InvalidInput, 1001),
        (Error::DnsTimeout, 5001),
        (Error::DnsResponse, 5002),
        (Error::DnsValidation, 5003),
        (Error::FakeIpUnavailable, 5004),
        (Error::ResourceLimit, 8001),
        (Error::Network, 6001),
        (Error::NetworkTimeout, 6002),
        (Error::Cancelled, 9002),
        (Error::VpnPreparationTimeout, 9003),
        (Error::Tls, 7001),
        (Error::Certificate, 7002),
        (Error::InvalidState, 9001),
        (Error::Unsupported, 1021),
        (Error::Internal, 9901),
        (Error::Script, 1301),
        (Error::ScriptTimeout, 1302),
        (Error::Rejected, 1101),
        (Error::ProtocolViolation, 1401),
    ] {
        assert_eq!(error.public_info().code.value(), expected);
    }
}
#[test]
fn invalid_config_keeps_snapshot_and_machine_failure() {
    use nodus_core::routing::{Match, Rule, runtime::GenerationInput, store::Store};
    // 预期来自独立规范：未知、重复字段及格式版本必须拒绝并准确区分。
    for (input, expected) in [
        (
            "format_version: 1\nsettings: {}\nsources: []\nunknown: true\n",
            2002,
        ),
        (
            "format_version: 1\nformat_version: 1\nsettings: {}\nsources: []\n",
            2003,
        ),
        ("format_version: 999\nsettings: {}\nsources: []\n", 2004),
        (
            "format_version: 1\nsettings: {debug_rules: []}\nsources: []\n",
            2002,
        ),
    ] {
        let error = GenerationInput::from_yaml(input)
            .err()
            .expect("非法配置必须失败");
        assert_eq!(error.code().value(), expected);
    }
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "nodus-error-transaction-{}-{unique}",
        std::process::id()
    ));
    let store = Store::memory();
    store.update(Config::default(), vec![]).unwrap();
    let previous = store.snapshot().unwrap();
    let mut invalid = Config::default();
    invalid.rules.push(Rule {
        protocol: None,
        r#type: Match::Domain,
        value: "bad host".into(),
        policy: Policy::Direct,
        outbound: None,
    });
    let error = store.update(invalid, vec![]).unwrap_err();
    assert_eq!(error.code().value(), 1001);
    assert!(std::sync::Arc::ptr_eq(
        &previous,
        &store.snapshot().unwrap()
    ));
    store.close().unwrap();
    drop(store);
    assert!(!path.exists(), "纯内存规则更新不能创建目录");
}
#[test]
fn diagnostics_remain_fixed_size_and_do_not_hold_error_chains() {
    use nodus_core::error::diagnostics::{self, DIAGNOSTIC_CAPACITY, DiagnosticSite};
    let (_, before) = diagnostics::snapshot();
    for _ in 0..DIAGNOSTIC_CAPACITY * 3 {
        diagnostics::record(Error::ResourceLimit, DiagnosticSite::Cleanup);
    }
    let (records, after) = diagnostics::snapshot();
    assert_eq!(records.len(), 64);
    assert!(records.iter().all(Option::is_some));
    assert!(after.saturating_sub(before) >= 128);
}

#[test]
fn segmented_codes_have_one_category_per_prefix() {
    // 类别表独立来自本次登记；只在验证中用算术检查分段，不生成生产编号。
    let categories = [
        (10, "INPUT"),
        (20, "CONFIG"),
        (40, "DEBUG"),
        (50, "DNS"),
        (60, "IO"),
        (70, "TLS"),
        (80, "RESOURCE"),
        (90, "LIFECYCLE"),
        (11, "POLICY"),
        (12, "ABI"),
        (13, "SCRIPT"),
        (14, "PROTOCOL"),
        (24, "GEOIP"),
        (99, "INTERNAL"),
    ];
    for retired in nodus_core::error::RETIRED_ERROR_CODES {
        assert_eq!(
            ErrorCode::from_value(*retired),
            None,
            "退役代码不得被重新解释为其它错误"
        );
        assert_eq!(
            ErrorInfo::from_json(format!("{{\"code\":{retired},\"params\":{{}}}}").as_bytes()),
            Err(Error::InvalidInput)
        );
    }
    for code in ErrorCode::ALL {
        assert!((1000..=9999).contains(&code.value()));
        assert_ne!(code.value() % 100, 0);
        assert_eq!(
            categories
                .iter()
                .find(|(prefix, _)| *prefix == code.value() / 100)
                .unwrap()
                .1,
            code.contract().category
        );
    }
}
#[test]
fn typed_context_is_required_bounded_and_specific_to_code() {
    use nodus_core::error::FieldName;
    for invalid in [
        r#"{"code":2002,"params":{}}"#,
        r#"{"code":2002,"params":{"field":"../../secret"}}"#,
        r#"{"code":2002,"params":{"field":"abcdefghijklmnopqrstuvwxyzabcdefg"}}"#,
        r#"{"code":2002,"params":{"field":"debug","field":"other"}}"#,
        r#"{"code":4001,"params":{"limit":3600}}"#,
        r#"{"code":4001,"params":{"actual":60,"limit":3600}}"#,
        r#"{"code":4001,"params":{"actual":0,"limit":3601}}"#,
        r#"{"code":8002,"params":{"actual":null,"limit":5}}"#,
        r#"{"code":2004,"params":{"actual":1,"expected":1}}"#,
        r#"{"code":2004,"params":{"actual":4294967296,"expected":1}}"#,
        r#"{"code":1001,"params":{"field":null}}"#,
        r#"{"code":1001,"params":[]}"#,
        r#"[1001,{}]"#,
        r#"{"code":0001,"params":{}}"#,
    ] {
        assert_eq!(
            ErrorInfo::from_json(invalid.as_bytes()),
            Err(Error::InvalidInput),
            "{invalid}"
        );
    }
    for error in [
        Error::ConfigUnknownField(Some(FieldName::new("debug").unwrap())),
        Error::ConfigDuplicateField(Some(FieldName::new("format_version").unwrap())),
        Error::ConfigUnsupportedVersion(2),
        Error::DebugInvalidLifetime(None),
        Error::DebugInvalidLifetime(Some(0)),
        Error::DebugInvalidLifetime(Some(3601)),
        Error::ResourceBudgetExceeded {
            actual: 1,
            limit: 4,
        },
    ] {
        let info = error.public_info();
        let bytes = serde_json::to_vec(&info).unwrap();
        assert!(bytes.len() <= nodus_core::error::MAX_ERROR_BYTES);
        assert_eq!(ErrorInfo::from_json(&bytes).unwrap(), info);
    }
    assert!(serde_json::to_vec(&ErrorInfo::new(ErrorCode::ResourceBudgetExceeded)).is_err());
}
#[test]
fn safety_filter_preserves_unknown_and_duplicate_classification() {
    use nodus_core::routing::runtime::GenerationInput;
    for key in ["abcdefghijklmnopqrstuvwxyzabcdefg", "隐私字段", "bad.field"] {
        let input = format!("format_version: 1\nsettings: {{}}\nsources: []\n{key}: true\n");
        let error = GenerationInput::from_yaml(&input).err().unwrap();
        assert_eq!(error.code().value(), 2002);
        let bytes = serde_json::to_vec(&error.public_info()).unwrap();
        assert_eq!(
            std::str::from_utf8(&bytes).unwrap(),
            r#"{"code":2002,"params":{"field":null}}"#
        );
        assert_eq!(ErrorInfo::from_json(&bytes).unwrap(), error.public_info());
    }
    let error = Error::ConfigDuplicateField(None);
    assert_eq!(
        serde_json::to_string(&error.public_info()).unwrap(),
        r#"{"code":2003,"params":{"field":null}}"#
    );
}
