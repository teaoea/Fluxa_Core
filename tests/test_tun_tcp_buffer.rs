//! 测试文件：tests/test_tun_tcp_buffer.rs
//! 测试类型：单元测试、TCP 边界回归测试
//! 测试对象：src/tun/tcp/buffer.rs
//! 测试内容：reordered_duplicate_overlap_wrap_and_finish；fixed_window_zero_and_open_after_consume；transmit_preserves_unacked_bytes_and_wraps。
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
