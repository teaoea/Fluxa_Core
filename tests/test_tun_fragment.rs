//! 测试文件：tests/test_tun_fragment.rs
//! 测试类型：单元测试、协议及资源边界回归
//! 测试对象：src/tun/fragment.rs
//! 测试内容：dual_stack_layout_checksum_and_reassembly；mtu_boundaries_and_single_packet_fast_path；identifier_exhaustion_and_quiet_period；invalid_inputs_and_complete_packet_limit。
//! 测试内容：partial_allocation_failure_returns_all_packet_budget。
//! 测试依据：RFC 791、8200、6864；八字节偏移、末片标记、完整 UDP 校验和及有界 ID 分配。
//! 前置条件：无外部服务、无需系统网络权限。
use super::*;
fn endpoints(v6: bool) -> (SocketAddr, SocketAddr) {
    if v6 {
        (
            "[2001:db8::1]:1000".parse().unwrap(),
            "[2001:db8::2]:2000".parse().unwrap(),
        )
    } else {
        (
            "192.0.2.1:1000".parse().unwrap(),
            "192.0.2.2:2000".parse().unwrap(),
        )
    }
}
fn sum(bytes: &[u8]) -> u16 {
    let mut total: u32 = bytes
        .chunks(2)
        .map(|p| (u32::from(p[0]) << 8) + u32::from(*p.get(1).unwrap_or(&0)))
        .sum();
    while total > 65535 {
        total = (total & 65535) + (total >> 16);
    }
    !(total as u16)
}
#[test]
fn dual_stack_layout_checksum_and_reassembly() {
    for v6 in [false, true] {
        let (src, dst) = endpoints(v6);
        let body: Vec<u8> = (0..8001).map(|i| (i % 251) as u8).collect();
        let fragments = encode(src, dst, &body, 1280).unwrap();
        assert_eq!(fragments.0.len(), 7);
        let mut transport = Vec::new();
        let mut identification = None;
        for (index, packet) in fragments.0.iter().enumerate() {
            let b = &packet.bytes;
            assert!(b.len() <= 1280);
            let (offset, more, id, header) = if v6 {
                assert_eq!((b[6], b[40], b[41]), (44, 17, 0));
                assert_eq!(u16::from_be_bytes([b[4], b[5]]) as usize, b.len() - 40);
                let field = u16::from_be_bytes([b[42], b[43]]);
                assert_eq!(field & 6, 0);
                (
                    (field & !7) as usize,
                    field & 1 != 0,
                    b[44..48].to_vec(),
                    48,
                )
            } else {
                assert_eq!(sum(&b[..20]), 0);
                assert_eq!(u16::from_be_bytes([b[2], b[3]]) as usize, b.len());
                let field = u16::from_be_bytes([b[6], b[7]]);
                assert_eq!(field & 0xc000, 0);
                (
                    ((field & 0x1fff) as usize) * 8,
                    field & 0x2000 != 0,
                    b[4..6].to_vec(),
                    20,
                )
            };
            if let Some(previous) = &identification {
                assert_eq!(previous, &id);
            } else {
                identification = Some(id);
            }
            assert_eq!(offset, transport.len());
            assert_eq!(more, index + 1 != fragments.0.len());
            if more {
                assert_eq!((b.len() - header) % 8, 0);
            }
            transport.extend_from_slice(&b[header..]);
        }
        assert_eq!(&transport[8..], body);
        assert_eq!(
            u16::from_be_bytes([transport[4], transport[5]]) as usize,
            transport.len()
        );
        let first = &fragments.0[0].bytes;
        let mut pseudo = if v6 {
            first[8..40].to_vec()
        } else {
            first[12..20].to_vec()
        };
        pseudo.extend_from_slice(&[0, 17]);
        pseudo.extend_from_slice(&(transport.len() as u16).to_be_bytes());
        pseudo.extend_from_slice(&transport);
        assert_eq!(sum(&pseudo), 0, "完整 UDP 校验和不能按单片重算");
        // 辅助回归：逆序交给既有重组器，独立字节预期仍为上方 body。
        let mut table = crate::tun::ip::Reassembly::default();
        let mut complete = None;
        for p in fragments.0.iter().rev() {
            if let Some(packet) = table.submit(&p.bytes, 1280, None, Instant::now()).unwrap() {
                complete = Some(packet.bytes().to_vec());
            }
        }
        let complete = complete.unwrap();
        assert_eq!(&complete[complete.len() - body.len()..], body);
    }
}
#[test]
fn mtu_boundaries_and_single_packet_fast_path() {
    for v6 in [false, true] {
        let (src, dst) = endpoints(v6);
        let header = if v6 { 48 } else { 28 };
        for mtu in [1280, 1281, 1500, 9000] {
            for size in [0, mtu - header, mtu - header + 1, 9000 - header] {
                if size + header > 9000 {
                    continue;
                }
                let output = encode(src, dst, &vec![7; size], mtu).unwrap();
                assert!(output.0.iter().all(|p| p.bytes.len() <= mtu));
                assert_eq!(output.0.len() == 1, size + header <= mtu);
                if output.0.len() == 1 {
                    assert_eq!(output.0[0].bytes.len(), size + header);
                }
            }
        }
    }
}
#[test]
fn identifier_exhaustion_and_quiet_period() {
    let now = Instant::now();
    let mut ids = Identifiers {
        next: 65535,
        last: None,
    };
    assert_eq!(ids.take(65535, now).unwrap(), 65535);
    assert!(matches!(
        ids.take(65535, now + Duration::from_secs(119)),
        Err(Error::ResourceLimit)
    ));
    assert_eq!(ids.take(65535, now + Duration::from_secs(120)).unwrap(), 0);
    let mut ids = Identifiers {
        next: u32::MAX as u64,
        last: None,
    };
    assert_eq!(ids.take(u32::MAX as u64, now).unwrap(), u32::MAX);
    assert!(ids.take(u32::MAX as u64, now).is_err());
    assert_eq!(
        ids.take(u32::MAX as u64, now + Duration::from_secs(120))
            .unwrap(),
        0
    );
}
#[test]
fn invalid_inputs_and_complete_packet_limit() {
    let (src, dst) = endpoints(false);
    for mtu in [0, 1279, 9001, usize::MAX] {
        assert!(matches!(
            encode(src, dst, b"", mtu),
            Err(Error::InvalidInput)
        ));
    }
    assert!(matches!(
        encode(src, dst, &vec![0; 8973], 1280),
        Err(Error::ResourceLimit)
    ));
    assert!(encode(src, endpoints(true).1, b"", 1280).is_err());
    assert!(encode(SocketAddr::new(src.ip(), 0), dst, b"", 1280).is_err());
}

#[test]
fn partial_allocation_failure_returns_all_packet_budget() {
    // 独立占满总预算，仅留 11000 字节：完整输入和第一片能分配，后续片必定失败。
    let mut held = Vec::new();
    let mut remaining = 4 * 1024 * 1024 - 11000;
    while remaining != 0 {
        let size = remaining.min(9000);
        held.push(Packet::build(size, |_| {}).unwrap());
        remaining -= size;
    }
    let (src, dst) = endpoints(false);
    assert!(matches!(
        encode(src, dst, &vec![1; 8001], 1280),
        Err(Error::ResourceBudgetExceeded { .. })
    ));
    // 任一已构造分片或原报文泄漏，9000 字节申请就会失败。
    let reclaimed = Packet::build(9000, |_| {}).unwrap();
    drop(reclaimed);
    drop(held);
}
