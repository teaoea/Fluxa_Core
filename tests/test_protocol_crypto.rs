//! 测试文件：tests/test_protocol_crypto.rs
//! 测试类型：单元测试、迁移回归
//! 测试对象：src/protocol/crypto.rs
//! 测试内容：
//! - all_suites_authenticate_records_and_key_updates：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
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
    fn all_suites_authenticate_records_and_key_updates() {
        for suite in [Suite::Aes128, Suite::Aes256, Suite::ChaCha] {
            let make = || Cipher::new(Zeroizing::new(vec![7; suite.hash_len()]), suite).unwrap();
            let mut writer = make();
            let mut reader = make();
            for n in [0, 1, 1024, 16384] {
                let data = vec![42; n];
                let record = writer.seal(23, &data).unwrap();
                let (kind, plain) = reader.open(&record).unwrap();
                assert_eq!(kind, 23);
                assert_eq!(*plain, data);
            }
            writer.update().unwrap();
            reader.update().unwrap();
            let record = writer.seal(22, b"updated").unwrap();
            assert_eq!(&*reader.open(&record).unwrap().1, b"updated");
            let mut corrupt = make().seal(23, b"auth").unwrap();
            corrupt[7] ^= 1;
            assert!(make().open(&corrupt).is_err());
            let mut record = make().seal(23, b"length").unwrap();
            record[4] ^= 1;
            assert!(make().open(&record).is_err());
            assert!(make().seal(23, &vec![0; 16385]).is_err());
        }
    }
}
