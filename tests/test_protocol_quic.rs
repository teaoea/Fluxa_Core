//! 测试文件：tests/test_protocol_quic.rs
//! 测试类型：单元测试、迁移回归
//! 测试对象：src/protocol/quic.rs
//! 测试内容：
//! - versions_lengths_association_and_bounded_input：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//!
//! 测试依据：保留原测试的业务/格式/授权/资源约束；《测试文件与报告统一管理原则》，独立语义审查不以实现输出反推预期。
//!
//! 前置条件：本地测试需文件/回环套接字权限；平台限定及忽略项以实际登记为准。
//!
//! 执行状态：由同名 Markdown 报告记录，不在源码中固化通过结论。

#[cfg(test)]
mod tests {
    use super::super::*;
    fn initial(version: u32) -> Vec<u8> {
        let mut bytes = vec![if version == V1 { 0xc0 } else { 0xd0 }];
        bytes.extend(version.to_be_bytes());
        bytes.extend([1, 1, 1, 2, 0, 17]);
        bytes.extend([0; 17]);
        bytes
    }
    #[test]
    fn versions_lengths_association_and_bounded_input() {
        for version in [V1, V2] {
            let packet = initial(version);
            for length in 0..packet.len() {
                assert!(!Flow::default().observe(&packet[..length]));
            }
            let mut flow = Flow::default();
            assert!(!flow.observe(&[0x40; 30]));
            assert!(flow.observe(&packet));
            assert!(flow.observe(&[0x40; 30]));
            assert!(!Flow::default().observe(&[0x40; 30]));
            let mut joined = packet.clone();
            joined.extend(&packet);
            assert!(Flow::default().observe(&joined));
            let mut padded = packet.clone();
            padded.extend([0; 1200]);
            assert!(Flow::default().observe(&padded));
            let mut malformed = packet;
            malformed[5] = 21;
            assert!(!Flow::default().observe(&malformed));
        }
        for length in 0..=9000 {
            let _ = Flow::default().observe(&vec![0xff; length]);
        }
    }
}
