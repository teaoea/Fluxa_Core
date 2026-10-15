//! 测试文件：tests/test_tun.rs
//! 测试类型：单元测试、迁移回归
//! 测试对象：src/tun.rs
//! 测试内容：
//! - ipv4_ipv6_tcp_udp_and_odd_payloads：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - every_truncation_and_checksum_corruption_is_rejected：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - fragments_options_extensions_and_unknown_protocols_are_rejected：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - udp_checksum_omission_only_allowed_for_ipv4：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - session_bounds_close_and_rejection_do_not_change_counters：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - invalid_transport_lengths_and_ip_headers_are_rejected：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - counter_overflow_is_atomic：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - arbitrary_bounded_bytes_never_panic：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//!
//! 测试依据：保留原测试的业务/格式/授权/资源约束；《测试文件与报告统一管理原则》，独立语义审查不以实现输出反推预期。
//!
//! 前置条件：本地测试需文件/回环套接字权限；平台限定及忽略项以实际登记为准。
//!
//! 执行状态：由同名 Markdown 报告记录，不在源码中固化通过结论。

#[cfg(test)]
mod tests {
    use super::super::*;
    fn packet(version: u8, protocol: u8) -> Vec<u8> {
        let ip = if version == 4 { 20 } else { 40 };
        let transport = if protocol == 6 { 20 } else { 8 };
        let mut bytes = vec![0; ip + transport + 3];
        bytes[0] = if version == 4 { 0x45 } else { 0x60 };
        if version == 4 {
            let len = bytes.len() as u16;
            bytes[2..4].copy_from_slice(&len.to_be_bytes());
            bytes[8] = 64;
            bytes[9] = protocol;
            bytes[12..16].copy_from_slice(&[192, 0, 2, 1]);
            bytes[16..20].copy_from_slice(&[198, 51, 100, 2]);
            let check = checksum(&bytes[..20], 0);
            bytes[10..12].copy_from_slice(&check.to_be_bytes());
        } else {
            bytes[4..6].copy_from_slice(&((transport + 3) as u16).to_be_bytes());
            bytes[6] = protocol;
            bytes[7] = 64;
            bytes[23] = 1;
            bytes[39] = 2;
        }
        bytes[ip..ip + 2].copy_from_slice(&12345_u16.to_be_bytes());
        bytes[ip + 2..ip + 4].copy_from_slice(&53_u16.to_be_bytes());
        let check_offset = if protocol == 6 {
            bytes[ip + 12] = 0x50;
            ip + 16
        } else {
            bytes[ip + 4..ip + 6].copy_from_slice(&((transport + 3) as u16).to_be_bytes());
            ip + 6
        };
        bytes[ip + transport..].copy_from_slice(b"abc");
        let check = checksum(
            &bytes[ip..],
            pseudo_sum(&bytes, version, protocol, transport + 3),
        );
        bytes[check_offset..check_offset + 2].copy_from_slice(&check.to_be_bytes());
        bytes
    }
    #[test]
    fn ipv4_ipv6_tcp_udp_and_odd_payloads() {
        for version in [4, 6] {
            for protocol in [6, 17] {
                let info = parse(&packet(version, protocol), 1500).unwrap();
                assert_eq!(
                    (
                        info.version,
                        info.protocol,
                        info.source_port,
                        info.destination_port,
                        info.payload_bytes
                    ),
                    (version, protocol, 12345, 53, 3)
                );
            }
        }
    }
    #[test]
    fn every_truncation_and_checksum_corruption_is_rejected() {
        for version in [4, 6] {
            for protocol in [6, 17] {
                let bytes = packet(version, protocol);
                for length in 0..bytes.len() {
                    assert!(parse(&bytes[..length], 1500).is_err());
                }
                let mut corrupt = bytes.clone();
                *corrupt.last_mut().unwrap() ^= 1;
                assert_eq!(parse(&corrupt, 1500), Err(Error::InvalidInput));
                let mut trailing = bytes;
                trailing.push(0);
                assert!(parse(&trailing, 1500).is_err());
            }
        }
    }
    #[test]
    fn fragments_options_extensions_and_unknown_protocols_are_rejected() {
        let mut fragment = packet(4, 17);
        fragment[6] = 0x20;
        fragment[10..12].fill(0);
        let check = checksum(&fragment[..20], 0);
        fragment[10..12].copy_from_slice(&check.to_be_bytes());
        assert_eq!(parse(&fragment, 1500), Err(Error::Unsupported));
        for next in [0, 43, 44, 50, 51, 58, 59, 60, 255] {
            let mut bytes = packet(6, 17);
            bytes[6] = next;
            assert_eq!(parse(&bytes, 1500), Err(Error::Unsupported));
        }
    }
    #[test]
    fn udp_checksum_omission_only_allowed_for_ipv4() {
        for version in [4, 6] {
            let mut bytes = packet(version, 17);
            let ip = if version == 4 { 20 } else { 40 };
            bytes[ip + 6..ip + 8].fill(0);
            assert_eq!(parse(&bytes, 1500).is_ok(), version == 4);
        }
    }
    #[test]
    fn session_bounds_close_and_rejection_do_not_change_counters() {
        assert!(PacketSession::new(1279).is_err());
        assert!(PacketSession::new(9001).is_err());
        let mut session = PacketSession::new(1500).unwrap();
        let bytes = packet(4, 17);
        assert_eq!(session.forward(&bytes), Err(Error::Unsupported));
        assert!(session.inspect(&vec![0; 1501]).is_err());
        assert_eq!(session.counters(), (0, 0));
        session.inspect(&bytes).unwrap();
        assert_eq!(session.counters(), (1, bytes.len() as u64));
        session.close();
        session.close();
        assert_eq!(session.inspect(&bytes), Err(Error::InvalidState));
        assert_eq!(session.forward(&bytes), Err(Error::InvalidState));
    }
    #[test]
    fn invalid_transport_lengths_and_ip_headers_are_rejected() {
        for version in [4, 6] {
            let ip = if version == 4 { 20 } else { 40 };
            let mut udp = packet(version, 17);
            udp[ip + 4..ip + 6].copy_from_slice(&8_u16.to_be_bytes());
            assert_eq!(parse(&udp, 1500), Err(Error::InvalidInput));
            for offset in [0x40, 0xf0] {
                let mut tcp = packet(version, 6);
                tcp[ip + 12] = offset;
                assert_eq!(parse(&tcp, 1500), Err(Error::InvalidInput));
            }
            let mut expired = packet(version, 17);
            expired[if version == 4 { 8 } else { 7 }] = 0;
            assert_eq!(parse(&expired, 1500), Err(Error::InvalidInput));
        }
        let mut options = packet(4, 17);
        options.splice(20..20, [1, 1, 1, 0]);
        options[0] = 0x46;
        let len = options.len() as u16;
        options[2..4].copy_from_slice(&len.to_be_bytes());
        options[10..12].fill(0);
        let check = checksum(&options[..24], 0);
        options[10..12].copy_from_slice(&check.to_be_bytes());
        assert_eq!(parse(&options, 1500), Err(Error::Unsupported));
    }

    #[test]
    fn counter_overflow_is_atomic() {
        let mut session = PacketSession::new(1500).unwrap();
        session.packets = u64::MAX;
        assert_eq!(session.inspect(&packet(4, 17)), Err(Error::ResourceLimit));
        assert_eq!(session.counters(), (u64::MAX, 0));
        session.packets = 0;
        session.bytes = u64::MAX;
        assert_eq!(session.inspect(&packet(4, 17)), Err(Error::ResourceLimit));
        assert_eq!(session.counters(), (0, u64::MAX));
    }

    #[test]
    fn arbitrary_bounded_bytes_never_panic() {
        let mut seed = 1_u64;
        for length in 0..=MAX_MTU {
            let bytes: Vec<_> = (0..length)
                .map(|_| {
                    seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                    (seed >> 32) as u8
                })
                .collect();
            let _ = parse(&bytes, MAX_MTU);
        }
    }
}
