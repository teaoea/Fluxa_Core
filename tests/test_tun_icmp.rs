//! 测试文件：tests/test_tun_icmp.rs
//! 测试类型：单元测试、控制报文信任边界回归
//! 测试对象：src/tun/icmp.rs
//! 测试内容：path_requires_recent_exact_quote_and_only_decreases。
//! 测试内容：path_requires_recent_exact_quote_and_only_decreases；independent_icmp_errors_validate_checksum_quote_direction_and_bounds；path_lifetime_and_ipv6_fragment_quote_are_bounded。
//! 测试内容：old_udp_quote_cannot_adjust_new_flow_with_same_endpoints_and_body。
//! 测试依据：RFC 1191、RFC 8201、RFC 5927；未知引用不得修改路径。
//! 前置条件：纯内存，无外部网络。
use super::*;
#[test]
fn path_requires_recent_exact_quote_and_only_decreases() {
    let packet = crate::tun::packet::encode_udp(
        "192.0.2.1:53".parse().unwrap(),
        "192.0.2.2:12345".parse().unwrap(),
        &[0; 1400],
        1500,
    )
    .unwrap();
    let q = quote(&packet.bytes).unwrap();
    let now = Instant::now();
    let mut path = Path::new(1500, true).unwrap();
    let notice = Notice {
        quote: q,
        kind: Kind::TooBig(576),
    };
    assert!(path.apply(notice, now).is_err());
    path.remember(&packet.bytes, now);
    path.apply(notice, now).unwrap();
    assert_eq!(path.mtu, 576);
    path.apply(
        Notice {
            kind: Kind::TooBig(1400),
            ..notice
        },
        now,
    )
    .unwrap();
    assert_eq!(path.mtu, 576);
    assert!(path.apply(notice, now + Duration::from_secs(30)).is_err());
    assert_eq!(
        reduced(
            1500,
            Notice {
                kind: Kind::TooBig(0),
                ..notice
            }
        ),
        Some(1006)
    );
}

fn sum(bytes: &[u8]) -> u16 {
    let mut total: u32 = bytes
        .chunks(2)
        .map(|x| u32::from(x[0]) * 256 + u32::from(*x.get(1).unwrap_or(&0)))
        .sum();
    while total > 65535 {
        total = (total & 65535) + (total >> 16);
    }
    !(total as u16)
}
/// 独立构造路由器错误；不使用生产 ICMP 编码器制造测试向量。
fn error(v6: bool, kind: u8, code: u8, mtu: u32, quoted: &[u8]) -> Vec<u8> {
    let header = if v6 { 40 } else { 20 };
    let mut out = vec![0; header + 8 + quoted.len()];
    out[0] = if v6 { 0x60 } else { 0x45 };
    if v6 {
        let len = out.len() - 40;
        out[4..6].copy_from_slice(&(len as u16).to_be_bytes());
        out[6] = 58;
        out[7] = 64;
        out[8..24].copy_from_slice(
            &"2001:db8::fe"
                .parse::<std::net::Ipv6Addr>()
                .unwrap()
                .octets(),
        );
        out[24..40].copy_from_slice(&quoted[8..24]);
    } else {
        let len = out.len();
        out[2..4].copy_from_slice(&(len as u16).to_be_bytes());
        out[8] = 64;
        out[9] = 1;
        out[12..16].copy_from_slice(&[192, 0, 2, 254]);
        out[16..20].copy_from_slice(&quoted[12..16]);
        let c = sum(&out[..20]);
        out[10..12].copy_from_slice(&c.to_be_bytes());
    }
    out[header] = kind;
    out[header + 1] = code;
    out[header + 4..header + 8].copy_from_slice(&mtu.to_be_bytes());
    out[header + 8..].copy_from_slice(quoted);
    let check = if v6 {
        let mut data = out[8..40].to_vec();
        data.extend_from_slice(&((out.len() - 40) as u32).to_be_bytes());
        data.extend_from_slice(&[0, 0, 0, 58]);
        data.extend_from_slice(&out[40..]);
        sum(&data)
    } else {
        sum(&out[20..])
    };
    out[header + 2..header + 4].copy_from_slice(&check.to_be_bytes());
    out
}
#[test]
fn independent_icmp_errors_validate_checksum_quote_direction_and_bounds() {
    for v6 in [false, true] {
        let source = if v6 {
            "[2001:db8::1]:53"
        } else {
            "192.0.2.1:53"
        }
        .parse()
        .unwrap();
        let destination = if v6 {
            "[2001:db8::2]:12345"
        } else {
            "192.0.2.2:12345"
        }
        .parse()
        .unwrap();
        let packet = crate::tun::packet::encode_udp(source, destination, &[1; 1400], 1500).unwrap();
        let n = if v6 { 48 } else { 28 };
        let encoded = error(
            v6,
            if v6 { 2 } else { 3 },
            if v6 { 0 } else { 4 },
            if v6 { 1200 } else { 576 },
            &packet.bytes[..n],
        );
        let notice = decode(&encoded).unwrap();
        assert_eq!(notice.quote.source, source);
        assert_eq!(notice.quote.destination, destination);
        assert_eq!(notice.quote.protocol, 17);
        assert_eq!(reduced(1500, notice), Some(if v6 { 1280 } else { 576 }));
        for length in 0..encoded.len() {
            assert!(decode(&encoded[..length]).is_err());
        }
        let mut bad = encoded.clone();
        *bad.last_mut().unwrap() ^= 1;
        assert!(decode(&bad).is_err());
        let bad = error(
            v6,
            if v6 { 2 } else { 3 },
            if v6 { 0 } else { 4 },
            1200,
            &packet.bytes[..n - 1],
        );
        assert!(decode(&bad).is_err());
        for (kind, code) in if v6 {
            [(1, 3), (3, 0)]
        } else {
            [(3, 1), (11, 0)]
        } {
            let p = error(v6, kind, code, 0, &packet.bytes[..n]);
            assert!(matches!(
                decode(&p).unwrap().kind,
                Kind::Unreachable | Kind::TimeExceeded
            ));
        }
    }
}
#[test]
fn path_lifetime_and_ipv6_fragment_quote_are_bounded() {
    let now = Instant::now();
    let mut path = Path::new(1500, true).unwrap();
    let datagram = crate::tun::fragment::encode(
        "[2001:db8::1]:53".parse().unwrap(),
        "[2001:db8::2]:12345".parse().unwrap(),
        &[1; 2000],
        1500,
    )
    .unwrap();
    let first = datagram.0.front().unwrap();
    let quote = quote(&first.bytes).unwrap();
    path.remember(&first.bytes, now);
    path.apply(
        Notice {
            quote,
            kind: Kind::TooBig(1280),
        },
        now,
    )
    .unwrap();
    assert_eq!(path.mtu, 1280);
    assert!(!path.expired(now + Duration::from_secs(599)));
    assert!(path.expired(now + Duration::from_secs(600)));
    assert!(super::quote(&datagram.0.back().unwrap().bytes).is_err());
}

#[test]
fn old_udp_quote_cannot_adjust_new_flow_with_same_endpoints_and_body() {
    for v6 in [false, true] {
        let now = Instant::now();
        let source = if v6 {
            "[2001:db8::1]:53"
        } else {
            "192.0.2.1:53"
        }
        .parse()
        .unwrap();
        let destination = if v6 {
            "[2001:db8::2]:12345"
        } else {
            "192.0.2.2:12345"
        }
        .parse()
        .unwrap();
        let mut old = Path::new(1500, !v6).unwrap();
        let mut a = crate::tun::packet::encode_udp(source, destination, &[1; 1400], 1500).unwrap();
        old.stamp(&mut a, now);
        let mut current = Path::new(1500, !v6).unwrap();
        let mut b = crate::tun::packet::encode_udp(source, destination, &[1; 1400], 1500).unwrap();
        current.stamp(&mut b, now);
        let first = quote(&a.bytes).unwrap();
        let second = quote(&b.bytes).unwrap();
        assert_ne!(first, second);
        assert!(
            current
                .apply(
                    Notice {
                        quote: first,
                        kind: Kind::TooBig(1280)
                    },
                    now
                )
                .is_err()
        );
        assert_eq!(current.mtu, 1500);
        current
            .apply(
                Notice {
                    quote: second,
                    kind: Kind::TooBig(1280),
                },
                now,
            )
            .unwrap();
        assert_eq!(current.mtu, 1280);
        // 修改标识不能破坏真实 IP/UDP 校验和。
        assert!(crate::tun::parse(&b.bytes, 1500).is_ok());
    }
}
