//! 测试文件：tests/test_tun_ip.rs
//! 测试类型：单元测试、分片与扩展头边界及真实 UDP 集成回归
//! 测试对象：src/tun/ip.rs、TUN 会话分派
//! 测试内容：dual_stack_out_of_order_reassembly；overlap_poison_and_absolute_expiry；limits_generations_and_revision；ipv6_options_routing_and_atomic_fragment；truncation_and_mutation_never_panic；fragmented_udp_uses_real_egress_and_close_revokes_input。
//! 测试内容：canceled_unpolled_cleanup_and_global_budget；malformed_headers_first_fragment_and_conflicting_metadata。
//! 测试内容：ipv6_fragment_boundary_preserves_extension_constraints。
//! 测试依据：RFC 791、8200、5722、7112；首片完整传输头、重叠废弃、代次隔离和有界资源契约。
//! 前置条件：本机 UDP 回环，不接管系统网络、不使用外部服务。
use super::*;
fn udp(v6: bool, destination_port: u16) -> Vec<u8> {
    let header = if v6 { 40 } else { 20 };
    let mut bytes = vec![0; header + 32];
    bytes[0] = if v6 { 0x60 } else { 0x45 };
    if v6 {
        bytes[4..6].copy_from_slice(&32u16.to_be_bytes());
        bytes[6] = 17;
        bytes[7] = 64;
        bytes[8..24].copy_from_slice(
            &"2001:db8::1"
                .parse::<std::net::Ipv6Addr>()
                .unwrap()
                .octets(),
        );
        bytes[24..40].copy_from_slice(&"::1".parse::<std::net::Ipv6Addr>().unwrap().octets());
    } else {
        bytes[2..4].copy_from_slice(&52u16.to_be_bytes());
        bytes[4..6].copy_from_slice(&42u16.to_be_bytes());
        bytes[8] = 64;
        bytes[9] = 17;
        bytes[12..16].copy_from_slice(&[192, 0, 2, 1]);
        bytes[16..20].copy_from_slice(&[127, 0, 0, 1]);
        let sum = independent_checksum(&bytes[..20]);
        bytes[10..12].copy_from_slice(&sum.to_be_bytes());
    }
    bytes[header..header + 2].copy_from_slice(&5000u16.to_be_bytes());
    bytes[header + 2..header + 4].copy_from_slice(&destination_port.to_be_bytes());
    bytes[header + 4..header + 6].copy_from_slice(&32u16.to_be_bytes());
    bytes[header + 8..].copy_from_slice(b"abcdefghijklmnopqrstuvwx");
    let mut pseudo = if v6 {
        bytes[8..40].to_vec()
    } else {
        bytes[12..20].to_vec()
    };
    pseudo.extend_from_slice(&[0, 17, 0, 32]);
    pseudo.extend_from_slice(&bytes[header..]);
    let sum = independent_checksum(&pseudo);
    bytes[header + 6..header + 8].copy_from_slice(&sum.to_be_bytes());
    bytes
}
fn independent_checksum(bytes: &[u8]) -> u16 {
    let mut sum: u32 = bytes
        .chunks(2)
        .map(|p| (u32::from(p[0]) << 8) + u32::from(*p.get(1).unwrap_or(&0)))
        .sum();
    while sum > 65535 {
        sum = (sum & 65535) + (sum >> 16);
    }
    !(sum as u16)
}
fn fragment(full: &[u8], offset: usize, end: usize, more: bool, id: u32) -> Vec<u8> {
    let v6 = full[0] >> 4 == 6;
    let header = if v6 { 40 } else { 20 };
    let mut out = full[..header].to_vec();
    if v6 {
        out[6] = 44;
        out[4..6].copy_from_slice(&((end - offset + 8) as u16).to_be_bytes());
        out.extend_from_slice(&[17, 0]);
        out.extend_from_slice(&((offset as u16) | u16::from(more)).to_be_bytes());
        out.extend_from_slice(&id.to_be_bytes());
    } else {
        out[2..4].copy_from_slice(&((end - offset + 20) as u16).to_be_bytes());
        out[4..6].copy_from_slice(&(id as u16).to_be_bytes());
        out[6..8]
            .copy_from_slice(&((offset as u16 / 8) | if more { 0x2000 } else { 0 }).to_be_bytes());
        out[10..12].fill(0);
        let sum = independent_checksum(&out);
        out[10..12].copy_from_slice(&sum.to_be_bytes());
    }
    out.extend_from_slice(&full[header + offset..header + end]);
    out
}
#[test]
fn dual_stack_out_of_order_reassembly() {
    for v6 in [false, true] {
        let full = udp(v6, 443);
        let mut table = Reassembly::default();
        let now = Instant::now();
        assert!(
            table
                .submit(&fragment(&full, 16, 32, false, 42), 1500, None, now)
                .unwrap()
                .is_none()
        );
        let first = fragment(&full, 0, 16, true, 42);
        let packet = table.submit(&first, 1500, None, now).unwrap().unwrap();
        let info = crate::tun::parse(packet.bytes(), 9000).unwrap();
        assert_eq!(
            (
                info.protocol,
                info.source_port,
                info.destination_port,
                info.payload_bytes
            ),
            (17, 5000, 443, 24)
        );
        assert_eq!(
            &packet.bytes()[packet.bytes().len() - 24..],
            b"abcdefghijklmnopqrstuvwx"
        );
        assert!(table.entries.is_empty());
    }
}
#[test]
fn overlap_poison_and_absolute_expiry() {
    for v6 in [false, true] {
        let full = udp(v6, 443);
        let first = fragment(&full, 0, 16, true, 42);
        let last = fragment(&full, 16, 32, false, 42);
        let now = Instant::now();
        let mut table = Reassembly::default();
        table.submit(&first, 1500, None, now).unwrap();
        assert!(matches!(
            table.submit(&first, 1500, None, now + Duration::from_secs(59)),
            Err(Error::InvalidInput)
        ));
        assert!(table.entries.values().all(|entry| entry.storage.is_none()));
        assert!(matches!(
            table.submit(&last, 1500, None, now + Duration::from_secs(59)),
            Err(Error::InvalidInput)
        ));
        // 重复包不能延长 60 秒截止时间。
        table.submit(&last, 1500, None, now + LIFETIME).unwrap();
        assert!(
            table
                .submit(&first, 1500, None, now + LIFETIME)
                .unwrap()
                .is_some()
        );
    }
}
#[test]
fn limits_generations_and_revision() {
    let full = udp(false, 443);
    let first = fragment(&full, 0, 16, true, 42);
    let last = fragment(&full, 16, 32, false, 42);
    let now = Instant::now();
    let mut table = Reassembly::default();
    table.submit(&first, 1500, Some(1), now).unwrap();
    assert!(table.submit(&last, 1500, Some(2), now).unwrap().is_none());
    table.revision(2);
    assert!(table.entries.is_empty());
    for id in 0..MAX_ENTRIES as u32 {
        table
            .submit(&fragment(&full, 0, 16, true, id), 1500, None, now)
            .unwrap();
    }
    assert!(matches!(
        table.submit(&fragment(&full, 0, 16, true, 99), 1500, None, now),
        Err(Error::ResourceLimit)
    ));
    table.clear();
    assert!(table.entries.is_empty());
    let mut too_far = fragment(&full, 16, 32, false, 42);
    too_far[6..8].copy_from_slice(&1124u16.to_be_bytes());
    too_far[10..12].fill(0);
    let sum = independent_checksum(&too_far[..20]);
    too_far[10..12].copy_from_slice(&sum.to_be_bytes());
    assert!(matches!(
        table.submit(&too_far, 1500, None, now),
        Err(Error::ResourceLimit)
    ));
}
#[test]
fn ipv6_options_routing_and_atomic_fragment() {
    let full = udp(true, 443);
    let now = Instant::now();
    let mut table = Reassembly::default();
    let mut extended = full[..40].to_vec();
    extended[6] = 0;
    extended[4..6].copy_from_slice(&40u16.to_be_bytes());
    extended.extend_from_slice(&[17, 0, 1, 4, 0, 0, 0, 0]);
    extended.extend_from_slice(&full[40..]);
    let normalized = table.submit(&extended, 1500, None, now).unwrap().unwrap();
    assert_eq!(normalized.bytes(), full);
    extended[42] = 0x80;
    assert!(matches!(
        table.submit(&extended, 1500, None, now),
        Err(Error::Unsupported)
    ));
    extended[6] = 43;
    extended[42] = 4;
    extended[43] = 1;
    assert!(matches!(
        table.submit(&extended, 1500, None, now),
        Err(Error::Unsupported)
    ));
    let first = fragment(&full, 0, 16, true, 42);
    table.submit(&first, 1500, None, now).unwrap();
    let atomic = fragment(&full, 0, 32, false, 42);
    assert_eq!(
        table
            .submit(&atomic, 1500, None, now)
            .unwrap()
            .unwrap()
            .bytes(),
        full
    );
    assert_eq!(table.entries.len(), 1);
    assert!(
        table
            .submit(&fragment(&full, 16, 32, false, 42), 1500, None, now)
            .unwrap()
            .is_some()
    );
}
#[test]
fn truncation_and_mutation_never_panic() {
    for v6 in [false, true] {
        let full = fragment(&udp(v6, 443), 0, 16, true, 42);
        for length in 0..full.len() {
            assert!(
                std::panic::catch_unwind(|| Reassembly::default()
                    .submit(&full[..length], 1500, None, Instant::now())
                    .is_err())
                .unwrap()
            );
        }
        for position in 0..full.len() {
            for mask in [1, 8, 128, 255] {
                let mut changed = full.clone();
                changed[position] ^= mask;
                assert!(
                    std::panic::catch_unwind(|| {
                        let _ = Reassembly::default().submit(&changed, 1500, None, Instant::now());
                    })
                    .is_ok()
                );
            }
        }
    }
}
#[tokio::test]
async fn fragmented_udp_uses_real_egress_and_close_revokes_input() {
    use crate::{
        outbound::{Limits, Manager},
        routing::{Config, Router},
        tun::{session, udp},
    };
    use std::sync::Arc;
    for v6 in [false, true] {
        let echo = tokio::net::UdpSocket::bind(if v6 { "[::1]:0" } else { "127.0.0.1:0" })
            .await
            .unwrap();
        let manager = Arc::new(Manager::default());
        manager
            .configure_network(
                None,
                Some(Arc::new(
                    Router::compile(Config {
                        fallback: crate::routing::Policy::Direct,
                        ..Config::default()
                    })
                    .unwrap(),
                )),
                Limits::default(),
            )
            .unwrap();
        let session = session::Session::new(
            manager,
            session::Options {
                udp: udp::Options::default(),
                ..Default::default()
            },
        )
        .unwrap();
        let full = udp(v6, echo.local_addr().unwrap().port());
        session
            .submit(&fragment(&full, 16, 32, false, 42), None)
            .await
            .unwrap();
        let mut data = [0; 64];
        assert!(
            tokio::time::timeout(Duration::from_millis(20), echo.recv_from(&mut data))
                .await
                .is_err()
        );
        session
            .submit(&fragment(&full, 0, 16, true, 42), None)
            .await
            .unwrap();
        let (count, peer) = tokio::time::timeout(Duration::from_secs(2), echo.recv_from(&mut data))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(&data[..count], b"abcdefghijklmnopqrstuvwx");
        echo.send_to(&data[..count], peer).await.unwrap();
        let response = tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                if let Some(packet) = session.receive().unwrap() {
                    break packet;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        assert_eq!(
            &response.bytes[response.bytes.len() - 24..],
            b"abcdefghijklmnopqrstuvwx"
        );
        session
            .submit(&fragment(&full, 0, 16, true, 99), None)
            .await
            .unwrap();
        assert!(MEMORY.usage().0 >= ENTRY_BYTES);
        let (first, second) = tokio::join!(session.close(), session.close());
        first.unwrap();
        second.unwrap();
        assert_eq!(
            MEMORY.usage().0,
            0,
            "关闭完成必须释放未完成重组，外部仍保留会话引用"
        );
        assert!(matches!(
            session.submit(&full, None).await,
            Err(Error::InvalidState) | Err(Error::Cancelled)
        ));
    }
}

#[test]
fn canceled_unpolled_cleanup_and_global_budget() {
    use std::sync::{Arc, Mutex};
    let now = Instant::now();
    let full = udp(false, 443);
    let first = fragment(&full, 0, 16, true, 42);
    let table = Arc::new(Mutex::new(Reassembly::default()));
    let exhausted = MEMORY.reserve(2 * 1024 * 1024).unwrap();
    assert!(matches!(
        table.lock().unwrap().submit(&first, 1500, None, now),
        Err(Error::ResourceBudgetExceeded { .. })
    ));
    assert!(table.lock().unwrap().entries.is_empty());
    drop(exhausted);
    table
        .lock()
        .unwrap()
        .submit(&first, 1500, None, now)
        .unwrap();
    assert_eq!(MEMORY.usage().0, ENTRY_BYTES);
    let cleanup = Cleanup(table.clone());
    let future = async move {
        cleanup.expire(now).unwrap();
    };
    // 不需要轮询异步任务，取消本身必须释放所有权。
    drop(future);
    assert_eq!(MEMORY.usage().0, 0);
    assert!(table.lock().unwrap().entries.is_empty());
}

#[test]
fn malformed_headers_first_fragment_and_conflicting_metadata() {
    let now = Instant::now();
    for v6 in [false, true] {
        let full = udp(v6, 443);
        let first = fragment(&full, 0, 16, true, 42);
        let mut table = Reassembly::default();
        table.submit(&first, 1500, None, now).unwrap();
        let mut conflicting = fragment(&full, 16, 32, false, 42);
        conflicting[if v6 { 7 } else { 8 }] = 63;
        if !v6 {
            conflicting[10..12].fill(0);
            let sum = independent_checksum(&conflicting[..20]);
            conflicting[10..12].copy_from_slice(&sum.to_be_bytes());
        }
        assert!(matches!(
            table.submit(&conflicting, 1500, None, now),
            Err(Error::InvalidInput)
        ));
        assert!(table.entries.values().all(|entry| entry.storage.is_none()));
        table.clear();
        let mut tcp_first = fragment(&full, 0, 16, true, 42);
        if v6 {
            tcp_first[40] = 6;
        } else {
            tcp_first[9] = 6;
            tcp_first[10..12].fill(0);
            let sum = independent_checksum(&tcp_first[..20]);
            tcp_first[10..12].copy_from_slice(&sum.to_be_bytes());
        }
        assert!(matches!(
            table.submit(&tcp_first, 1500, None, now),
            Err(Error::InvalidInput)
        ));
        assert!(table.entries.is_empty());
    }
}

#[test]
fn ipv6_fragment_boundary_preserves_extension_constraints() {
    let now = Instant::now();
    let full = udp(true, 443);
    let mut wrong_order = fragment(&full, 0, 16, true, 42);
    wrong_order[40] = 0;
    assert!(matches!(
        Reassembly::default().submit(&wrong_order, 1500, None, now),
        Err(Error::InvalidInput)
    ));
    // 分片前 8 字节 Hop-by-Hop + Fragment 8 字节 + 分片后 248 字节选项共 264。
    let mut long_chain = full[..40].to_vec();
    long_chain[6] = 0;
    long_chain[4..6].copy_from_slice(&272u16.to_be_bytes());
    long_chain.extend_from_slice(&[44, 0, 1, 4, 0, 0, 0, 0]);
    long_chain.extend_from_slice(&[60, 0, 0, 1, 0, 0, 0, 42]);
    let mut destination = vec![0; 248];
    destination[0] = 17;
    destination[1] = 30;
    long_chain.extend_from_slice(&destination);
    long_chain.extend_from_slice(&full[40..48]);
    assert!(matches!(
        Reassembly::default().submit(&long_chain, 1500, None, now),
        Err(Error::ResourceLimit)
    ));
    // 相同标识的分片不能悄悄更换未分片扩展头内容。
    let with_options = |offset, end, more| {
        let raw = fragment(&full, offset, end, more, 42);
        let mut out = raw[..40].to_vec();
        out[6] = 0;
        out[4..6].copy_from_slice(&((raw.len() - 32) as u16).to_be_bytes());
        out.extend_from_slice(&[44, 0, 0x20, 2, 0, 1, 0, 0]);
        out.extend_from_slice(&raw[40..]);
        out
    };
    let mut table = Reassembly::default();
    table
        .submit(&with_options(0, 16, true), 1500, None, now)
        .unwrap();
    let mut altered = with_options(16, 32, false);
    altered[45] = 2;
    assert!(matches!(
        table.submit(&altered, 1500, None, now),
        Err(Error::InvalidInput)
    ));
    assert!(table.entries.values().all(|entry| entry.storage.is_none()));
}
