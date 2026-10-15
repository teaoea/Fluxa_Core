//! 测试文件：tests/test_error.rs
//! 测试类型：单元测试、迁移回归
//! 测试对象：src/error.rs
//! 测试内容：
//! - tls_keeps_budget_state_protocol_and_cancellation_causes：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//!
//! 测试依据：保留原测试的业务/格式/授权/资源约束；《测试文件与报告统一管理原则》，独立语义审查不以实现输出反推预期。
//!
//! 前置条件：本地测试需文件/回环套接字权限；平台限定及忽略项以实际登记为准。
//!
//! 执行状态：由同名 Markdown 报告记录，不在源码中固化通过结论。

#[cfg(test)]
mod tls_mapping_tests {
    use super::super::*;
    #[test]
    fn tls_keeps_budget_state_protocol_and_cancellation_causes() {
        // 预期来自业务契约：改变传输层不能把资源或权限/状态原因改报为 TLS 校验。
        for error in [
            Error::ResourceLimit,
            Error::Internal,
            Error::InvalidState,
            Error::Cancelled,
            Error::Rejected,
            Error::Unsupported,
            Error::ProtocolViolation,
        ] {
            assert_eq!(Error::from_tls(io::Error::other(error)), error);
        }
        assert_eq!(
            Error::from_tls(io::Error::from(io::ErrorKind::OutOfMemory)),
            Error::ResourceLimit
        );
        assert_eq!(
            Error::from_tls(io::Error::from(io::ErrorKind::TimedOut)),
            Error::NetworkTimeout
        );
        assert_eq!(
            Error::from_tls(io::Error::from(io::ErrorKind::InvalidData)),
            Error::Tls
        );
    }
}
