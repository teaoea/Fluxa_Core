//! 一致性检查核对契约，语义检查采用独立确认的业务预期；不由当前实现生成断言。
use libfluxa_core::{
    error::{Error, ErrorCode, ErrorInfo},
    routing::{Config, Mode, Policy, Router, Target, Transport},
};
use std::collections::HashSet;
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
        let value: ErrorInfo = serde_json::from_slice(code.json_bytes()).unwrap();
        assert_eq!(value.code, *code);
        assert_eq!(serde_json::to_value(value).unwrap()["code"], code.value());
    }
    let rows = document
        .lines()
        .filter(|line| line.starts_with("| `"))
        .count();
    assert_eq!(rows, ErrorCode::ALL.len(), "文档不得虚构代码");
    for old in baseline.as_array().unwrap() {
        assert!(
            registered.as_array().unwrap().contains(old),
            "已登记语义不得静默删除、修改或复用"
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
fn policies_errors_and_debug_lifecycle_have_independent_expected_semantics() {
    // 业务契约：纯匹配返回 Reject 是成功的策略决策，执行拒绝与远端 I/O 故障不同。
    let router = Router::compile(Config {
        mode: Mode::Rules,
        fallback: Policy::Reject,
        ..Default::default()
    })
    .unwrap();
    let decision = router
        .evaluate(Target {
            domain: Some("example.test".into()),
            address: None,
            port: 443,
            transport: Transport::Tcp,
        })
        .unwrap();
    assert_eq!(decision.policy, Policy::Reject);
    assert_eq!(Error::Rejected.code().value(), 1017);
    let refused: Error = std::io::Error::from(std::io::ErrorKind::ConnectionRefused).into();
    assert_eq!(refused.code().value(), 1007);
    assert_ne!(refused.code(), Error::Rejected.code());
    let session = libfluxa_core::routing::debug::Session::new().unwrap();
    session.close().unwrap();
    assert_eq!(session.clear().unwrap_err().code().value(), 1012);
    assert_eq!(Error::DnsTimeout.code().value(), 1002);
    assert_eq!(Error::NetworkTimeout.code().value(), 1008);
    assert_eq!(Error::ResourceLimit.code().value(), 1006);
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
    assert_eq!(public, r#"{"code":1007,"params":{}}"#);
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
    assert_eq!(std::mem::size_of::<ErrorInfo>(), 2);
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

#[test]
fn failed_debug_replacement_keeps_the_authorized_active_rules() {
    use libfluxa_core::routing::{
        Match, Rule,
        debug::{DebugRule, Session},
    };
    let session = Session::new().unwrap();
    let row = |lifetime| DebugRule {
        rule: Rule {
            r#type: Match::Domain,
            value: "example.test".into(),
            policy: Policy::Direct,
            outbound: None,
        },
        protocol: None,
        lifetime_seconds: lifetime,
    };
    let before = session.replace(vec![row(Some(60))], None).unwrap();
    for lifetime in [None, Some(0), Some(3601)] {
        let error = session.replace(vec![row(lifetime)], None).unwrap_err();
        assert_eq!(error.code(), ErrorCode::InvalidInput);
        assert_eq!(session.status().unwrap().version, before.version);
    }
    let error = session.replace(vec![row(Some(60)); 257], None).unwrap_err();
    assert_eq!(error.code(), ErrorCode::ResourceLimit);
    assert_eq!(session.status().unwrap().version, before.version);
    let router = Router::compile(Config {
        fallback: Policy::Reject,
        ..Default::default()
    })
    .unwrap()
    .with_debug(&session)
    .unwrap();
    assert_eq!(
        router
            .evaluate(Target {
                domain: Some("example.test".into()),
                address: None,
                port: 443,
                transport: Transport::Tcp
            })
            .unwrap()
            .policy,
        Policy::Direct
    );
}

#[tokio::test]
async fn real_dns_deadline_and_closed_port_fail_without_policy_success() {
    use libfluxa_core::dns::{Config as DnsConfig, Query, Resolver, Server};
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
    let manager = libfluxa_core::outbound::Manager::default();
    let result = manager
        .open(
            target.parse().unwrap(),
            Transport::Tcp,
            Policy::Direct,
            1000,
        )
        .await;
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().code(), ErrorCode::Network);
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
fn legacy_statuses_keep_their_published_numeric_values() {
    use libfluxa_core::error::LegacyAbiStatus;
    assert_eq!(LegacyAbiStatus::Success.value(), 0);
    assert_eq!(LegacyAbiStatus::InvalidHandle.value(), 1);
    assert_eq!(LegacyAbiStatus::Unsupported.value(), 2);
    assert_eq!(LegacyAbiStatus::Internal.value(), 3);
    assert_eq!(LegacyAbiStatus::Empty.value(), 11);
    for (error, status) in [
        (Error::InvalidInput, 4),
        (Error::ResourceLimit, 5),
        (Error::InvalidState, 6),
        (Error::Cancelled, 7),
        (Error::Network, 8),
        (Error::Rejected, 9),
        (Error::ProtocolViolation, 9),
        (Error::DnsTimeout, 10),
        (Error::NetworkTimeout, 10),
    ] {
        assert_eq!(error.legacy_tun_status().value(), status);
    }
}

#[test]
fn every_internal_error_maps_to_the_independent_protocol_identity() {
    // 预期来自首版协议登记，不通过生产 match 或枚举下标推导。
    for (error, expected) in [
        (Error::InvalidInput, 1001),
        (Error::DnsTimeout, 1002),
        (Error::DnsResponse, 1003),
        (Error::DnsValidation, 1004),
        (Error::FakeIpUnavailable, 1005),
        (Error::ResourceLimit, 1006),
        (Error::Network, 1007),
        (Error::NetworkTimeout, 1008),
        (Error::Cancelled, 1009),
        (Error::Tls, 1010),
        (Error::Certificate, 1011),
        (Error::InvalidState, 1012),
        (Error::Unsupported, 1013),
        (Error::Internal, 1014),
        (Error::Script, 1015),
        (Error::ScriptTimeout, 1016),
        (Error::Rejected, 1017),
        (Error::ProtocolViolation, 1020),
    ] {
        assert_eq!(error.public_info().code.value(), expected);
    }
}
#[test]
fn invalid_config_keeps_snapshot_and_machine_failure() {
    use libfluxa_core::routing::{Match, Rule, runtime::GenerationInput, store::Store};
    // 公开约定只登记现有通用输入类别；不凭字段名字虚构未知字段专用代码。
    for input in [
        "format_version: 1\nsettings: {}\nsources: []\nunknown: true\n",
        "format_version: 1\nformat_version: 1\nsettings: {}\nsources: []\n",
        "format_version: 999\nsettings: {}\nsources: []\n",
        "format_version: 1\nsettings: {debug_rules: []}\nsources: []\n",
    ] {
        let error = GenerationInput::from_yaml(input)
            .err()
            .expect("非法配置必须失败");
        assert_eq!(error.code().value(), 1001);
    }
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "fluxa-error-transaction-{}-{unique}",
        std::process::id()
    ));
    let store = Store::open(&path).unwrap();
    store.update(Config::default(), vec![]).unwrap();
    let previous = store.snapshot().unwrap();
    let mut invalid = Config::default();
    invalid.rules.push(Rule {
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
    std::fs::remove_dir_all(path).unwrap();
}
#[test]
fn diagnostics_remain_fixed_size_and_do_not_hold_error_chains() {
    use libfluxa_core::error::diagnostics::{self, DIAGNOSTIC_CAPACITY, DiagnosticSite};
    let (_, before) = diagnostics::snapshot();
    for _ in 0..DIAGNOSTIC_CAPACITY * 3 {
        diagnostics::record(Error::ResourceLimit, DiagnosticSite::Cleanup);
    }
    let (records, after) = diagnostics::snapshot();
    assert_eq!(records.len(), 64);
    assert!(records.iter().all(Option::is_some));
    assert!(after.saturating_sub(before) >= 128);
}
