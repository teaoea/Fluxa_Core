//! 测试文件：tests/test_protocol_datagram.rs
//! 测试类型：单元测试、迁移回归
//! 测试对象：src/protocol/datagram.rs
//! 测试内容：
//! - independent_response_boundaries_and_peer_constraints：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - invalid_session_and_peer_are_not_policy_rejects：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
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
    fn independent_response_boundaries_and_peer_constraints() {
        let target = "192.0.2.4:53".parse().unwrap();
        let response = [
            0, 12, 0, 0, 2, 1, 2, 0, 53, 1, 192, 0, 2, 4, 0, 3, 0, 255, 1,
        ];
        for size in 0..response.len() {
            assert!(decode(&response[..size], target).unwrap().is_none());
        }
        let frame = decode(&response, target).unwrap().unwrap();
        assert_eq!(frame.payload, Some((16, 3)));
        assert_eq!(frame.consumed, response.len());
        assert!(decode(&response, "192.0.2.5:53".parse().unwrap()).is_err());
        assert!(encode(target, true, &[]).is_err());
        assert!(encode(target, false, &vec![0; MAX_PAYLOAD + 1]).is_err());
        assert_eq!(
            &*encode(target, true, &[1]).unwrap(),
            &[0, 12, 0, 0, 1, 1, 2, 0, 53, 1, 192, 0, 2, 4, 0, 1, 1]
        );
        assert_eq!(
            &*encode(target, false, &[2]).unwrap(),
            &[0, 4, 0, 0, 2, 1, 0, 1, 2]
        );
        for size in 0..=9000 {
            let bytes = vec![0xff; size];
            let _ = decode(&bytes, target);
        }
    }
}
#[cfg(test)]
mod error_semantics_tests {
    use super::super::*;
    #[test]
    fn invalid_session_and_peer_are_not_policy_rejects() {
        let target: SocketAddr = "192.0.2.1:443".parse().unwrap();
        let bad_session = [0, 4, 0, 1, 2, 0];
        assert!(matches!(
            decode(&bad_session, target),
            Err(Error::ProtocolViolation)
        ));
        let bad_flags = [0, 4, 0, 0, 2, 2];
        assert!(matches!(
            decode(&bad_flags, target),
            Err(Error::ProtocolViolation)
        ));
        // 会话 0、NEW 状态、DATA 标志，携带非绑定 IPv4 来源；不放宽目标检查。
        let wrong_peer = [0, 12, 0, 0, 2, 1, 2, 0x01, 0xbb, 1, 192, 0, 2, 2];
        assert!(matches!(
            decode(&wrong_peer, target),
            Err(Error::ProtocolViolation)
        ));
    }
}
