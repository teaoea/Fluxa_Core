//! 测试文件：tests/test_protocol_reality.rs
//! 测试类型：单元测试、迁移回归
//! 测试对象：src/protocol/reality.rs
//! 测试内容：
//! - record_cipher_authentication_sequence_and_update：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
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
    fn record_cipher_authentication_sequence_and_update() {
        let key = Zeroizing::new(vec![1; 32]);
        let mut tx = Cipher::new(key.clone(), Suite::Aes128).unwrap();
        let mut rx = Cipher::new(key, Suite::Aes128).unwrap();
        let frame = tx.seal(23, b"payload").unwrap();
        assert_eq!(rx.open(&frame).unwrap().1.as_slice(), b"payload");
        assert!(rx.open(&frame).is_err());
        tx.update().unwrap();
        rx.update().unwrap();
        let mut frame = tx.seal(23, b"new").unwrap();
        frame[7] ^= 1;
        assert!(rx.open(&frame).is_err());
    }
}
