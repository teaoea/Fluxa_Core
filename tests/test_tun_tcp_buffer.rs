//! 测试文件：tests/test_tun_tcp_buffer.rs
//! 测试类型：单元测试、TCP 边界回归测试
//! 测试对象：src/tun/tcp/buffer.rs
//! 测试内容：reordered_duplicate_overlap_wrap_and_finish；fixed_window_zero_and_open_after_consume；transmit_preserves_unacked_bytes_and_wraps。
//! 测试内容：sack_blocks_merge_recent_first_and_survive_wrap；sack_bitmap_matches_independent_presence_model。
//! 测试内容：sack_never_reports_bytes_beyond_known_fin。
//! 测试内容：sack_bitmap_word_and_ring_boundaries_after_consume。
//! 测试依据：TCP 序号、窗口和有界资源契约；RFC 9293；RFC 6298。
//! 前置条件：本地内存及确定性虚拟时钟，无外部服务。
use super::*;
#[test]
fn reordered_duplicate_overlap_wrap_and_finish() {
    let mut buffer = Receive::new(u32::MAX - 2);
    buffer.accept(1, b"ef", true);
    assert_eq!(buffer.pending(), 0);
    buffer.accept(u32::MAX - 2, b"abcd", false);
    assert_eq!(&*buffer.copy_pending(20), b"abcdef");
    assert!(buffer.finished);
    assert_eq!(buffer.next(), 4);
    buffer.accept(u32::MAX - 2, b"XXXX", false);
    assert_eq!(&*buffer.copy_pending(20), b"abcdef");
    buffer.consume(6).unwrap();
    assert_eq!(buffer.pending(), 0);
    assert!(buffer.consume(1).is_err());
}
#[test]
fn fixed_window_zero_and_open_after_consume() {
    let mut buffer = Receive::new(10);
    buffer.accept(10, &vec![1; CAPACITY], false);
    assert_eq!(buffer.window(), 0);
    buffer.accept(10u32.wrapping_add(CAPACITY as u32), b"x", false);
    buffer.consume(100).unwrap();
    assert_eq!(buffer.window(), 100);
    buffer.accept(10u32.wrapping_add(CAPACITY as u32), b"x", false);
    assert_eq!(buffer.copy_pending(CAPACITY).last(), Some(&b'x'));
}
#[test]
fn transmit_preserves_unacked_bytes_and_wraps() {
    let mut buffer = Transmit::new(u32::MAX);
    buffer.push(b"abcdef").unwrap();
    buffer.sent = 4;
    buffer.acknowledge(2).unwrap();
    assert_eq!(buffer.base, 1);
    assert_eq!(&*buffer.copy(1, 4).unwrap(), b"cdef");
    assert!(buffer.acknowledge(3).is_err());
}

#[test]
fn sack_blocks_merge_recent_first_and_survive_wrap() {
    let mut buffer = Receive::new(u32::MAX - 9);
    let base = u32::MAX - 9;
    buffer.accept(base.wrapping_add(20), b"abcd", false);
    buffer.accept(base.wrapping_add(40), b"efgh", false);
    let blocks = buffer.sacks();
    assert_eq!(
        blocks.blocks(),
        &[
            (base.wrapping_add(40), base.wrapping_add(44)),
            (base.wrapping_add(20), base.wrapping_add(24))
        ]
    );
    buffer.accept(base.wrapping_add(24), &[1; 16], false);
    assert_eq!(
        buffer.sacks().blocks(),
        &[(base.wrapping_add(20), base.wrapping_add(44))]
    );
    buffer.accept(base, &[2; 20], false);
    assert!(buffer.sacks().blocks().is_empty());
    buffer.consume(44).unwrap();
    for offset in [2, 4, 6, 8, 10, 12] {
        buffer.accept(buffer.next().wrapping_add(offset), b"z", false);
    }
    let result = buffer.sacks();
    assert_eq!(result.blocks().len(), 4);
    assert_eq!(result.blocks()[0].0, buffer.next().wrapping_add(12));
}

#[test]
fn sack_bitmap_matches_independent_presence_model() {
    let base = u32::MAX - 100;
    let mut buffer = Receive::new(base);
    let mut present = vec![false; CAPACITY];
    let mut rng = 17u32;
    for _ in 0..2000 {
        rng = rng.wrapping_mul(1664525).wrapping_add(1013904223);
        let start = (rng as usize) % (CAPACITY - 100);
        let count = (rng as usize >> 16) % 100 + 1;
        buffer.accept(base.wrapping_add(start as u32), &vec![1; count], false);
        present[start..start + count].fill(true);
        let next = present.iter().position(|p| !*p).unwrap_or(CAPACITY);
        assert_eq!(buffer.pending(), next);
        for &(left, right) in buffer.sacks().blocks() {
            let from = left.wrapping_sub(base) as usize;
            let to = right.wrapping_sub(base) as usize;
            assert!(from > next && from < to && to <= CAPACITY);
            assert!(present[from..to].iter().all(|p| *p));
            assert!(!present[from - 1]);
            assert!(to == CAPACITY || !present[to]);
        }
    }
}

#[test]
fn sack_never_reports_bytes_beyond_known_fin() {
    let mut b = Receive::new(100);
    b.accept(150, b"past-fin", false);
    b.accept(120, b"before", true);
    assert_eq!(b.sacks().blocks(), &[(120, 126)]);
}

#[test]
fn sack_bitmap_word_and_ring_boundaries_after_consume() {
    let mut b = Receive::new(0);
    b.accept(0, &[1; 135], false);
    b.consume(135).unwrap();
    b.accept(200, &[1; 130], false);
    b.accept(135 + CAPACITY as u32 - 100, &[1; 100], false);
    assert_eq!(
        b.sacks().blocks(),
        &[
            (135 + CAPACITY as u32 - 100, 135 + CAPACITY as u32),
            (200, 330)
        ]
    );
}
