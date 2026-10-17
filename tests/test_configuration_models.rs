//! 测试文件：tests/test_configuration_models.rs
//! 测试类型：单元测试、字段模型契约
//! 测试对象：src/configuration/models.rs
//! 测试内容：
//! - enum_values_are_explicit：模式、日志、IP 策略只接受约定拼写。
//! - nested_unknown_fields_are_rejected：节点和 TUN 不接受已移除字段。
//!
//! 测试依据：新 example_config.yaml 注释与用户移除旧字段的要求。
//!
//! 前置条件：无网络或系统资源需求。
use super::*;
fn decode<T: serde::de::DeserializeOwned>(text: &str) -> crate::error::Result<T> {
    crate::routing::runtime::yaml::decode(text)
}
#[test]
fn enum_values_are_explicit() {
    for value in ["rule", "global", "direct"] {
        assert!(decode::<Mode>(value).is_ok());
    }
    assert!(decode::<Mode>("rules").is_err());
    for value in ["silent", "info", "debug", "warn", "error"] {
        assert!(decode::<LogFilter>(value).is_ok());
    }
    assert!(decode::<LogFilter>("warning").is_err());
    for value in ["prefer-ipv4", "prefer-ipv6", "only-ipv4", "only-ipv6"] {
        assert!(decode::<IpStrategy>(value).is_ok());
    }
    assert!(decode::<IpStrategy>("auto").is_err());
    assert!(decode::<IpStrategy>("prefer_ipv4").is_err());
}
#[test]
fn nested_unknown_fields_are_rejected() {
    assert!(matches!(
        decode::<Tun>("exclude-routes: []"),
        Err(crate::error::Error::ConfigUnknownField(_))
    ));
    assert!(matches!(
        decode::<Mitm>("exclude-hosts: []"),
        Err(crate::error::Error::ConfigUnknownField(_))
    ));
    assert!(matches!(
        decode::<FakeIp>("ipv4-range: 198.18.0.0/16\nipv6-range: fd00::/64\nexclude: []"),
        Err(crate::error::Error::ConfigUnknownField(_))
    ));

    assert!(matches!(
        decode::<SystemProxy>("port: 0\nunknown: true"),
        Err(crate::error::Error::ConfigUnknownField(_))
    ));
    assert!(matches!(
        decode::<LanShare>("port: 0\nunknown: true"),
        Err(crate::error::Error::ConfigUnknownField(_))
    ));
    assert!(matches!(
        decode::<Dns>("nameserver: ['1.1.1.1']"),
        Err(crate::error::Error::ConfigUnknownField(_))
    ));
    assert!(matches!(
        decode::<Tun>("enable: false\ndns-hijack: true"),
        Err(crate::error::Error::ConfigUnknownField(_))
    ));
    assert!(matches!(
        decode::<Proxy>("skip-cert-verify: true"),
        Err(crate::error::Error::ConfigUnknownField(_))
    ));
}
