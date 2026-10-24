//! 测试文件：tests/test_routing_yaml.rs
//! 测试类型：单元测试、迁移回归
//! 测试对象：src/routing/yaml.rs
//! 测试内容：
//! - mapping_entry_uses_key_and_original_order：动态映射按键匹配原文序号，不按排序序号。
//! - mapping_entry_rejects_missing_invalid_and_oversized_inputs：缺失、非法和超长输入不伪造位置。
//! - duplicate_unsafe_identifier_keeps_its_real_classification：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//!
//! 测试依据：保留原测试的业务/格式/授权/资源约束；《测试文件与报告统一管理原则》，独立语义审查不以实现输出反推预期。
//!
//! 前置条件：本地测试需文件/回环套接字权限；平台限定及忽略项以实际登记为准。
//!
//! 执行状态：由同名 Markdown 报告记录，不在源码中固化通过结论。

#[cfg(test)]
mod error_context_tests {
    use super::super::*;
    #[test]
    fn duplicate_unsafe_identifier_keeps_its_real_classification() {
        for key in ["abcdefghijklmnopqrstuvwxyzabcdefg", "隐私字段", "bad.field"] {
            let input = format!("{key}: 1\n{key}: 2\n");
            let error = decode::<Value>(&input).unwrap_err();
            assert_eq!(error, Error::ConfigDuplicateField(None));
            assert_eq!(
                serde_json::to_string(&error.public_info()).unwrap(),
                r#"{"code":2003,"params":{"field":null}}"#
            );
        }
    }
}

#[cfg(test)]
mod source_position_tests {
    use super::super::*;
    #[test]
    fn mapping_entry_uses_key_and_original_order() {
        for (input, token) in [
            (
                "# 中文 😀\ndns:\n  hosts:\n    z.example: 192.0.2.1\n    a.example: bad\n",
                "a.example",
            ),
            (
                "dns: {hosts: {'z.example': '192.0.2.1', 'a.example': bad}}",
                "'a.example'",
            ),
        ] {
            assert_eq!(
                mapping_key_position(input, "dns.hosts", "a.example"),
                Some((1, input[..input.find(token).unwrap()].chars().count()))
            );
        }
    }
    #[test]
    fn mapping_entry_rejects_missing_invalid_and_oversized_inputs() {
        let yaml = "dns: {hosts: {z.example: 192.0.2.1}}";
        assert!(mapping_key_position(yaml, "dns.hosts", "missing.example").is_none());
        assert!(mapping_key_position(yaml, "other.hosts", "z.example").is_none());
        assert!(mapping_key_position("dns: [", "dns.hosts", "z.example").is_none());
        assert!(mapping_key_position(yaml, &"x".repeat(1024), "z.example").is_none());
        assert!(mapping_key_position(yaml, "dns.hosts", &"x".repeat(1025)).is_none());
    }
}
