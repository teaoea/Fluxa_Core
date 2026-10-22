//! 测试文件：tests/test_main_configuration_contract.rs
//! 测试类型：集成测试、主配置与公开错误协议
//! 测试对象：configuration、routing::yaml、error 的跨模块校验契约。
//! 测试内容：
//! - os_process_rule_controls_actual_outbound_without_request_claim：OS 名称匹配控制真实出站且请求不能伪造进程字段。
//! - committed_main_configuration_controls_new_requests_and_keeps_live_connection：新配置生效且旧连接继续传输。
//! - main_configuration_atomically_publishes_nodes_rules_and_limits：节点、规则、限额和摘要同版本发布。
//! - unavailable_rule_dependencies_fail_explicitly：独立验证新规则语义与失败边界。
//! - keywords_match_only_the_registrable_tenant_label：独立验证新规则语义与失败边界。
//! - compiled_scalar_rules_obey_scoped_conditions_and_final：独立验证新规则语义与失败边界。
//! - configuration_errors_preserve_integer_protocol_and_do_not_expose_values：配置错误保留整数代码且不回显原文。
//! - validation_summary_is_not_running_status：合法配置只返回校验摘要，不虚构启动结果。
//!
//! 测试依据：主配置原子验证、统一错误体系及公开参数安全要求。
//!
//! 前置条件：无外部网络、节点凭据、监听端口或 VPN 权限。
use nodus_core::{configuration::Document, error::ErrorInfo};
#[test]
fn configuration_errors_preserve_integer_protocol_and_do_not_expose_values() {
    for (text, code) in [
        ("rules: ['final,missing']", 1001),
        (
            "rules: ['final,direct']\nmitm: {enabled: false, decrypt-all: true}\nlan-share: {account: user, password: do-not-disclose}",
            2002,
        ),
        (
            "rules: ['final,direct']\nsecret_field: do-not-disclose",
            2002,
        ),
        ("rules: ['final,direct']\nmode: rule\nmode: global", 2003),
    ] {
        let error = match Document::from_yaml(text) {
            Ok(_) => panic!("必须拒绝非法配置"),
            Err(error) => error,
        };
        let encoded = serde_json::to_vec(&error.public_info()).unwrap();
        let info = ErrorInfo::from_json(&encoded).unwrap();
        assert_eq!(info.code.value(), code);
        if text.contains("decrypt-all") {
            assert!(
                info.params.field.is_none(),
                "按既有 Schema 安全省略连字符字段"
            );
        }
        assert!(
            !String::from_utf8(encoded)
                .unwrap()
                .contains("do-not-disclose")
        );
    }
}
#[test]
fn validation_summary_is_not_running_status() {
    let config = Document::from_yaml(
        "mode: global\nrules: ['final,direct']\nsystem-proxy: {port: 1054}\nlan-share: {port: 1055}",
    )
    .unwrap();
    let summary = serde_json::to_value(config.summary()).unwrap();
    assert_eq!(summary["mode"], "global");
    assert_eq!(summary["node_count"], 0);
    assert_eq!(summary["system_proxy_port"], 1054);
    assert_eq!(summary["lan_share_port"], 1055);
    assert!(summary.get("running").is_none());
    assert!(summary.get("success").is_none());
}

/// 独立预期：协议 > 端口 > 域名，且组合条件同时检查后缀标签边界与实际端口/传输。
#[test]
fn compiled_scalar_rules_obey_scoped_conditions_and_final() {
    use nodus_core::routing::{Policy, Target, Transport};
    let yaml = "rules: ['protocol,tcp,example.com,direct', 'port,53,example.com,reject', 'domain-suffix,example.com,reject', 'final,direct']";
    let snapshot = Document::from_yaml(yaml)
        .unwrap()
        .compile_local_rules(7)
        .unwrap();
    for (host, port, transport, expected) in [
        ("example.com", 443, Transport::Tcp, Policy::Direct),
        ("deep.a.example.com", 443, Transport::Tcp, Policy::Direct),
        ("example.com", 53, Transport::Tcp, Policy::Reject),
        ("a.example.com", 53, Transport::Udp, Policy::Reject),
        ("a.example.com", 54, Transport::Udp, Policy::Reject),
        ("badexample.com", 53, Transport::Udp, Policy::Direct),
    ] {
        let target = Target {
            domain: Some(host.into()),
            address: Some("192.0.2.1".into()),
            port,
            transport,
        };
        let actual = snapshot.evaluate(&target).unwrap();
        assert_eq!(actual.policy, expected, "{host}:{port}/{transport:?}");
        assert_eq!(actual, snapshot.evaluate_reference(&target).unwrap());
    }
    assert!(
        Document::from_yaml("rules: ['final,direct']")
            .unwrap()
            .compile_local_rules(0)
            .is_err()
    );
}
/// PSL 包含私有后缀，关键词不得越过注册标签观察子域名、后缀或单标签主机。
#[test]
fn keywords_match_only_the_registrable_tenant_label() {
    use nodus_core::routing::{Policy, Target, Transport};
    for (keyword, host, expected) in [
        ("foo", "a.foo.github.io", Policy::Reject),
        ("github", "a.foo.github.io", Policy::Direct),
        ("example", "a.example.co.uk", Policy::Reject),
        ("co", "a.example.co.uk", Policy::Direct),
        ("ads", "ads.example.com", Policy::Direct),
        ("com", "example.com", Policy::Direct),
        ("abc", "abc", Policy::Direct),
        ("foo", "a.FOO.github.io", Policy::Reject),
    ] {
        let yaml = format!("rules: ['domain-keyword,{keyword},reject', 'final,direct']");
        let snapshot = Document::from_yaml(&yaml)
            .unwrap()
            .compile_local_rules(1)
            .unwrap();
        let target = Target {
            domain: Some(host.into()),
            address: None,
            port: 443,
            transport: Transport::Tcp,
        };
        assert_eq!(
            snapshot.evaluate(&target).unwrap().policy,
            expected,
            "{keyword}/{host}"
        );
        assert_eq!(
            snapshot.evaluate(&target).unwrap(),
            snapshot.evaluate_reference(&target).unwrap()
        );
    }
}
/// 指定但未具备能力时返回真实错误，不能伪装成成功、UDP 匹配或忽略 GeoIP。
#[test]
fn unavailable_rule_dependencies_fail_explicitly() {
    use nodus_core::error::Error;
    let input = "rules: ['protocol,quic,example.com,direct', 'final,direct']";
    // 已接入 QUIC 组合条件；编译可成功，但必须由实际报文入口选择，不能据 UDP 或端口猜测。
    let quic = Document::from_yaml(input)
        .unwrap()
        .compile_local_rules(1)
        .unwrap();
    assert_eq!(
        quic.document().rules[0].rule.r#type,
        nodus_core::routing::Match::DomainQuic
    );
    let geoip = "geoip: {enabled: true, url: 'https://example.com/db.mmdb'}\nrules: ['geoip,CN,direct', 'final,reject']";
    assert!(matches!(
        Document::from_yaml(geoip).unwrap().compile_local_rules(1),
        Err(Error::GeoipInitializationFailed)
    ));
    let disabled = "rules: ['geoip,CN,direct', 'final,reject']";
    assert_eq!(
        Document::from_yaml(disabled)
            .unwrap()
            .compile_local_rules(1)
            .unwrap()
            .document()
            .report
            .original,
        0
    );
}

#[tokio::test]
async fn committed_main_configuration_controls_new_requests_and_keeps_live_connection() {
    use nodus_core::{
        error::Error,
        outbound::{Family, Manager, RoutedTarget},
        routing::{Target, Transport},
    };
    let server = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let manager = Manager::default();
    let first = manager
        .apply_configuration("rules: ['final,direct']")
        .unwrap();
    let request = || RoutedTarget {
        target: Target {
            domain: None,
            address: Some("127.0.0.1".into()),
            port: server.local_addr().unwrap().port(),
            transport: Transport::Udp,
        },
        timeout_ms: 1000,
        family: Family::Ipv4,
        mapping_generation: None,
    };
    let opened = manager.connect_configured(request()).await.unwrap();
    let second = manager
        .apply_configuration("rules: ['final,reject']")
        .unwrap();
    assert_ne!(first, second);
    assert!(matches!(
        manager.connect_configured(request()).await,
        Err(Error::Rejected)
    ));
    manager
        .write(opened.connection.id, b"still-alive", 1000)
        .await
        .unwrap();
    let mut buffer = [0; 32];
    let (count, _) = tokio::time::timeout(
        std::time::Duration::from_secs(1),
        server.recv_from(&mut buffer),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(&buffer[..count], b"still-alive");
    manager.close(opened.connection.id).await.unwrap();
    manager.stop().await.unwrap();
}
#[test]
fn main_configuration_atomically_publishes_nodes_rules_and_limits() {
    use nodus_core::{
        outbound::Manager,
        routing::{Policy, Target, Transport},
    };
    let manager = Manager::default();
    let yaml = r#"
proxies:
  - {name: node, type: vless, server: 192.0.2.1, port: 443, uuid: '00000000-0000-0000-0000-000000000000', flow: xtls-rprx-vision, security: reality, sni: example.com, pbk: AQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQE, sid: '01'}
outbound-limits: {maximum-connections: 2, idle-timeout-ms: 1000}
rules: ['final,node']
"#;
    let revision = manager.apply_configuration(yaml).unwrap();
    let network = manager.network_snapshot().unwrap();
    assert_eq!(network.revision, revision);
    assert_eq!(network.limits.maximum_connections, 2);
    assert_eq!(network.outbounds.len(), 1);
    assert!(network.outbounds.contains_key("node"));
    let decision = network
        .router
        .as_ref()
        .unwrap()
        .evaluate(Target {
            domain: None,
            address: Some("203.0.113.1".into()),
            port: 443,
            transport: Transport::Tcp,
        })
        .unwrap();
    assert_eq!(decision.policy, Policy::Proxy);
    assert_eq!(decision.outbound.as_deref(), Some("node"));
    let status = manager.configuration_status().unwrap();
    assert_eq!(status.revision, revision);
    let encoded = serde_json::to_string(&status).unwrap();
    assert!(!encoded.contains("00000000"));
    assert!(!encoded.contains("AQEBAQE"));
    assert_eq!(status.configuration.unwrap().node_count, 1);
}

#[cfg(target_os = "macos")]
#[tokio::test]
async fn os_process_rule_controls_actual_outbound_without_request_claim() {
    use nodus_core::{
        error::Error,
        outbound::{Family, Manager, RoutedTarget},
        routing::{Policy, Target, Transport},
    };
    let peer = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let name = std::env::current_exe()
        .unwrap()
        .file_name()
        .unwrap()
        .to_str()
        .unwrap()
        .to_owned();
    let request = || RoutedTarget {
        target: Target {
            domain: None,
            address: Some("127.0.0.1".into()),
            port: peer.local_addr().unwrap().port(),
            transport: Transport::Udp,
        },
        timeout_ms: 1000,
        family: Family::Ipv4,
        mapping_generation: None,
    };
    let manager = Manager::default();
    let scalar = serde_json::to_string(&format!("process-name,{name},direct")).unwrap();
    manager
        .apply_configuration(&format!("rules: [{scalar}, 'final,reject']"))
        .unwrap();
    let opened = manager.connect_configured(request()).await.unwrap();
    assert_eq!(opened.decision.policy, Policy::Direct);
    manager
        .write(opened.connection.id, b"os-verified", 1000)
        .await
        .unwrap();
    let mut bytes = [0u8; 32];
    let (length, _) = tokio::time::timeout(
        std::time::Duration::from_secs(1),
        peer.recv_from(&mut bytes),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(&bytes[..length], b"os-verified");
    manager.close(opened.connection.id).await.unwrap();
    let scalar = serde_json::to_string(&format!("process-name,{name}-different,direct")).unwrap();
    manager
        .apply_configuration(&format!("rules: [{scalar}, 'final,reject']"))
        .unwrap();
    assert!(matches!(
        manager.connect_configured(request()).await,
        Err(Error::Rejected)
    ));
    // RoutedTarget 仅反序列化，不增加让请求声明可信身份的字段。
    let forged = serde_json::json!({"target":{"address":"127.0.0.1","port":443,"transport":"udp"},"timeout_ms":1000,"process_name":name});
    assert!(serde_json::from_value::<RoutedTarget>(forged).is_err());
    manager.stop().await.unwrap();
}
