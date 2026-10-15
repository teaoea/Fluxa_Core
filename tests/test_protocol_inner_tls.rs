//! 测试文件：tests/test_protocol_inner_tls.rs
//! 测试类型：单元测试、迁移回归
//! 测试对象：src/protocol/inner_tls.rs
//! 测试内容：
//! - fragmented_record_boundary：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - non_tls_does_not_enable_switch：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//!
//! 测试依据：保留原测试的业务/格式/授权/资源约束；《测试文件与报告统一管理原则》，独立语义审查不以实现输出反推预期。
//!
//! 前置条件：本地测试需文件/回环套接字权限；平台限定及忽略项以实际登记为准。
//!
//! 执行状态：由同名 Markdown 报告记录，不在源码中固化通过结论。

#[cfg(test)]
mod tests {
    use super::super::*;
    #[test]
    fn fragmented_record_boundary() {
        let mut b = Boundary::default();
        assert_eq!(b.application_end(&[22, 3, 3, 0, 2, 1]), None);
        assert_eq!(b.application_end(&[2, 23, 3, 3, 0, 3, 1]), None);
        assert_eq!(b.application_end(&[2, 3, 99]), Some(2));
    }
    #[test]
    fn non_tls_does_not_enable_switch() {
        assert_eq!(Boundary::default().application_end(b"GET / HTTP/1.1"), None);
        assert!(!ServerHello::default().feed(b"HTTP/1.1 200 OK").unwrap());
    }
}
