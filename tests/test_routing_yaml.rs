//! 测试文件：tests/test_routing_yaml.rs
//! 测试类型：单元测试、迁移回归
//! 测试对象：src/routing/yaml.rs
//! 测试内容：
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
