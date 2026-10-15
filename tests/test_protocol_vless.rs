//! 测试文件：tests/test_protocol_vless.rs
//! 测试类型：单元测试、迁移回归
//! 测试对象：src/protocol/vless.rs
//! 测试内容：
//! - address_encoding_and_flow：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
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
    fn address_encoding_and_flow() {
        let uuid = [7; 16];
        let v4 = request(&uuid, "192.0.2.4", 443).unwrap();
        assert_eq!(&v4[..18], &[&[0][..], &uuid[..], &[18][..]].concat());
        assert_eq!(&v4[18..36], b"\x0a\x10xtls-rprx-vision");
        assert_eq!(&v4[36..], &[1, 1, 187, 1, 192, 0, 2, 4]);
        let v6 = request(&uuid, "2001:db8::1", 80).unwrap();
        assert_eq!(v6[39], 3);
        assert_eq!(v6.len(), 56);
        let domain = request(&uuid, "ExAmPlE.com.", 80).unwrap();
        assert_eq!(&domain[39..], b"\x02\x0bexample.com");
        assert!(request(&uuid, "example.com", 0).is_err());
        assert!(request(&uuid, "bad host", 80).is_err());
    }
}
