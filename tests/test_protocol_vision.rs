//! 测试文件：tests/test_protocol_vision.rs
//! 测试类型：单元测试、迁移回归
//! 测试对象：src/protocol/vision.rs
//! 测试内容：
//! - every_fragment_boundary_and_stop：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - bad_uuid_command_truncation_and_quota：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
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
    fn every_fragment_boundary_and_stop() {
        let uuid = [7; 16];
        let mut wire = frame(Some(&uuid), b"first", 0).unwrap();
        wire.extend(frame(None, b"last", 2).unwrap());
        for split in 0..=wire.len() {
            let mut d = Decoder::new();
            let mut out = d.feed(&wire[..split], &uuid).unwrap();
            out.extend(d.feed(&wire[split..], &uuid).unwrap());
            assert_eq!(out, b"firstlast");
            assert!(d.direct);
            d.finish().unwrap();
        }
    }
    #[test]
    fn bad_uuid_command_truncation_and_quota() {
        let uuid = [7; 16];
        let wire = frame(Some(&uuid), b"first", 0).unwrap();
        assert!(Decoder::new().feed(&wire, &[8; 16]).is_err());
        let mut d = Decoder::new();
        d.feed(&wire[..17], &uuid).unwrap();
        assert!(d.finish().is_err());
        assert!(
            Decoder::new()
                .feed(&vec![0; 2 * LIMIT + 22], &uuid)
                .is_err()
        );
        let mut bad = uuid.to_vec();
        bad.extend([3, 0, 0, 0, 0]);
        assert!(Decoder::new().feed(&bad, &uuid).is_err());
    }
}
