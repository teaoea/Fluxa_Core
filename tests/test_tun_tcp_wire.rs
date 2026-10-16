//! 测试文件：tests/test_tun_tcp_wire.rs
//! 测试类型：单元测试、TCP 报文输入边界回归测试
//! 测试对象：src/tun/tcp/wire.rs
//! 测试内容：exact_fields_roundtrip_for_both_address_families；invalid_window_mss_and_checksum_are_rejected；malformed_options_and_urgent_data_never_enter_state_machine；deterministic_truncated_and_mutated_packets_never_panic。
//! 测试内容：sack_options_have_independent_wire_layout_and_strict_bounds。
//! 测试内容：sack_malformed_options_rejected_after_valid_checksum。
//! 测试依据：RFC 9293 TCP 头部、选项及校验和约束；不可信输入不得 panic。
//! 前置条件：纯本地内存，不触网。
use super::*;
fn syn(v6: bool) -> Segment<'static> {
    Segment {
        source: if v6 {
            "[2001:db8::1]:12345"
        } else {
            "192.0.2.1:12345"
        }
        .parse()
        .unwrap(),
        destination: if v6 {
            "[2001:db8::2]:443"
        } else {
            "192.0.2.2:443"
        }
        .parse()
        .unwrap(),
        sequence: u32::MAX,
        acknowledgement: 0,
        window: 32768,
        flags: Flags(Flags::SYN),
        maximum_segment: Some(1200),
        sack_permitted: false,
        sacks: crate::tun::tcp::wire::Sacks::default(),
        payload: &[],
    }
}
#[test]
fn exact_fields_roundtrip_for_both_address_families() {
    for v6 in [false, true] {
        let expected = syn(v6);
        let packet = encode(&expected, 1500).unwrap();
        let ip = if v6 { 40 } else { 20 };
        assert_eq!(packet.bytes.len(), ip + 24);
        assert_eq!(&packet.bytes[ip + 20..], &[2, 4, 4, 176]);
        let actual = decode(&packet.bytes, 1500).unwrap();
        assert_eq!(
            (actual.source, actual.destination),
            (expected.source, expected.destination)
        );
        assert_eq!(
            (actual.sequence, actual.acknowledgement, actual.window),
            (u32::MAX, 0, 32768)
        );
        assert_eq!(actual.maximum_segment, Some(1200));
    }
}
#[test]
fn invalid_window_mss_and_checksum_are_rejected() {
    let mut input = syn(false);
    input.window = 65536;
    assert!(matches!(encode(&input, 1500), Err(Error::InvalidInput)));
    input.window = 32768;
    input.maximum_segment = Some(63);
    assert!(matches!(encode(&input, 1500), Err(Error::InvalidInput)));
    input.maximum_segment = Some(1200);
    input.flags = Flags(Flags::ACK);
    assert!(matches!(encode(&input, 1500), Err(Error::InvalidInput)));
    let mut packet = encode(&syn(false), 1500).unwrap();
    packet.bytes[24] ^= 1;
    assert!(decode(&packet.bytes, 1500).is_err());
}
#[test]
fn malformed_options_and_urgent_data_never_enter_state_machine() {
    let mut packet = encode(&syn(false), 1500).unwrap();
    for (position, value) in [(41, 3), (33, Flags::URG | Flags::SYN), (32, 0x61)] {
        let saved = packet.bytes[position];
        packet.bytes[position] = value;
        packet.bytes[36..38].fill(0);
        let sum = tun::checksum(
            &packet.bytes[20..],
            tun::pseudo_sum(&packet.bytes, 4, 6, 24),
        );
        packet.bytes[36..38].copy_from_slice(&sum.to_be_bytes());
        assert!(decode(&packet.bytes, 1500).is_err());
        packet.bytes[position] = saved;
    }
}
#[test]
fn deterministic_truncated_and_mutated_packets_never_panic() {
    let valid = encode(&syn(false), 1500).unwrap();
    for length in 0..=valid.bytes.len() {
        assert!(std::panic::catch_unwind(|| decode(&valid.bytes[..length], 1500)).is_ok());
    }
    for position in 0..valid.bytes.len() {
        for mask in [1, 8, 0x80, 0xff] {
            let mut bytes = valid.bytes.to_vec();
            bytes[position] ^= mask;
            assert!(std::panic::catch_unwind(|| decode(&bytes, 1500)).is_ok());
        }
    }
    assert!(decode(&[0; 9001], 9000).is_err());
}

#[test]
fn sack_options_have_independent_wire_layout_and_strict_bounds() {
    for v6 in [false, true] {
        let ip = if v6 { 40 } else { 20 };
        let mut input = syn(v6);
        input.sack_permitted = true;
        let output = encode(&input, 1500).unwrap();
        assert_eq!(&output.bytes[ip + 24..ip + 28], &[4, 2, 0, 0]);
        assert!(decode(&output.bytes, 1500).unwrap().sack_permitted);
        input.flags = Flags(Flags::ACK);
        input.maximum_segment = None;
        input.sack_permitted = false;
        for (l, r) in [(u32::MAX - 2, 2), (10, 20), (30, 40), (50, 60)] {
            input.sacks.push(l, r).unwrap();
        }
        let output = encode(&input, 1500).unwrap();
        assert_eq!(output.bytes.len(), ip + 56);
        assert_eq!(&output.bytes[ip + 20..ip + 22], &[5, 34]);
        assert_eq!(
            &output.bytes[ip + 22..ip + 30],
            &[255, 255, 255, 253, 0, 0, 0, 2]
        );
        assert_eq!(decode(&output.bytes, 1500).unwrap().sacks, input.sacks);
        assert!(input.sacks.push(70, 80).is_err());
        input.flags = Flags(Flags::SYN);
        assert!(encode(&input, 1500).is_err());
        assert!(Sacks::default().push(1, 1).is_err());
        assert!(Sacks::default().push(0, 1 << 31).is_err());
        assert!(Sacks::default().push(100, 2).is_err());
    }
}

#[test]
fn sack_malformed_options_rejected_after_valid_checksum() {
    fn repaired(bytes: &mut [u8]) {
        bytes[36..38].fill(0);
        let mut pseudo = bytes[12..20].to_vec();
        pseudo.extend_from_slice(&[0, 6]);
        pseudo.extend_from_slice(&((bytes.len() - 20) as u16).to_be_bytes());
        pseudo.extend_from_slice(&bytes[20..]);
        let mut sum: u32 = pseudo
            .chunks(2)
            .map(|b| (u32::from(b[0]) << 8) + u32::from(*b.get(1).unwrap_or(&0)))
            .sum();
        while sum > 65535 {
            sum = (sum & 65535) + (sum >> 16);
        }
        bytes[36..38].copy_from_slice(&(!(sum as u16)).to_be_bytes());
    }
    let mut input = syn(false);
    input.maximum_segment = None;
    input.sack_permitted = true;
    let valid = encode(&input, 1500).unwrap();
    for (index, value) in [(41, 3), (33, Flags::ACK), (42, 4), (43, 2)] {
        let mut b = valid.bytes.to_vec();
        b[index] = value;
        // 单独修改重复选项的两个字节。
        if index == 42 {
            b[43] = 2;
        }
        if index == 43 {
            b[42] = 4;
        }
        repaired(&mut b);
        assert!(decode(&b, 1500).is_err());
    }
    input.sack_permitted = false;
    input.flags = Flags(Flags::ACK);
    input.sacks.push(10, 20).unwrap();
    let valid = encode(&input, 1500).unwrap();
    for (index, value) in [
        (41, 2),
        (41, 9),
        (41, 18),
        (33, Flags::SYN),
        (33, 0),
        (49, 10),
    ] {
        let mut b = valid.bytes.to_vec();
        b[index] = value;
        if index == 49 {
            b[49] = 10;
        } // 右边界等于左边界。
        repaired(&mut b);
        assert!(decode(&b, 1500).is_err());
    }
}
