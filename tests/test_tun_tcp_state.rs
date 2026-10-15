//! 测试文件：tests/test_tun_tcp_state.rs
//! 测试类型：单元测试、TCP 边界回归测试
//! 测试对象：src/tun/tcp/state.rs
//! 测试内容：syn_retransmission_and_acknowledgement_validation；receive_reordering_duplicate_and_window_update；segmentation_partial_ack_and_retransmit_preserve_payload；zero_window_probe_and_reopen_does_not_consume_data；half_close_keeps_return_data_then_fin_and_time_wait；reset_requires_expected_sequence_and_far_packet_cannot_release_tx；duplicate_ack_triggers_fast_retransmit_without_waiting_for_timer；exponential_retry_budget_exhausts_with_explicit_timeout；active_half_close_accepts_final_return_bytes_and_fin；round_trip_estimate_adapts_and_retransmission_is_not_a_sample；finish_waits_for_bounded_flight_ledger_space；duplicate_syn_consumes_retry_budget_and_handshake_has_absolute_deadline。
//! 测试依据：TCP 序号、窗口和有界资源契约；RFC 9293；RFC 6298。
//! 前置条件：本地内存及确定性虚拟时钟，无外部服务。
use super::*;
fn segment(
    sequence: u32,
    acknowledgement: u32,
    flags: u8,
    window: usize,
    payload: &[u8],
) -> Segment<'_> {
    Segment {
        source: "192.0.2.1:12345".parse().unwrap(),
        destination: "127.0.0.1:8888".parse().unwrap(),
        sequence,
        acknowledgement,
        flags: Flags(flags),
        window,
        maximum_segment: None,
        payload,
    }
}
fn ready(window: usize) -> (State, Instant) {
    let now = Instant::now();
    let syn = segment(100, 0, Flags::SYN, window, &[]);
    let mut state = State::new(&syn, 500, 1500, Duration::from_millis(50), now).unwrap();
    let packet = state.emit(now).unwrap().unwrap();
    let response = wire::decode(&packet.bytes, 1500).unwrap();
    assert_eq!((response.sequence, response.acknowledgement), (500, 101));
    state
        .process(&segment(101, 501, Flags::ACK, window, &[]), now)
        .unwrap();
    assert_eq!(state.phase(), Phase::Established);
    (state, now)
}
#[test]
fn syn_retransmission_and_acknowledgement_validation() {
    let now = Instant::now();
    let mut state = State::new(
        &segment(u32::MAX, 0, Flags::SYN, 4096, &[]),
        u32::MAX,
        1500,
        Duration::from_secs(1),
        now,
    )
    .unwrap();
    let first = state.emit(now).unwrap().unwrap();
    let retry = state.emit(now + Duration::from_secs(1)).unwrap().unwrap();
    assert_eq!(first.bytes, retry.bytes);
    state
        .process(&segment(0, 100, Flags::ACK, 4096, &[]), now)
        .unwrap();
    assert_eq!(state.phase(), Phase::SynReceived);
    state
        .process(&segment(0, 0, Flags::ACK, 4096, &[]), now)
        .unwrap();
    assert_eq!(state.phase(), Phase::Established);
}
#[test]
fn receive_reordering_duplicate_and_window_update() {
    let (mut state, now) = ready(4096);
    state
        .process(&segment(104, 501, Flags::ACK, 4096, b"def"), now)
        .unwrap();
    assert_eq!(state.pending_input(), 0);
    state
        .process(&segment(101, 501, Flags::ACK, 4096, b"abc"), now)
        .unwrap();
    assert_eq!(&*state.input(20), b"abcdef");
    state
        .process(&segment(101, 501, Flags::ACK, 4096, b"XXXXXX"), now)
        .unwrap();
    assert_eq!(&*state.input(20), b"abcdef");
    state.consume_input(6).unwrap();
    assert_eq!(state.pending_input(), 0);
    let packet = state.emit(now).unwrap().unwrap();
    let ack = wire::decode(&packet.bytes, 1500).unwrap();
    assert_eq!(ack.acknowledgement, 107);
    assert_eq!(ack.window, CAPACITY);
}
#[test]
fn segmentation_partial_ack_and_retransmit_preserve_payload() {
    let (mut state, now) = ready(64);
    state.output(&[9; 200]).unwrap();
    let first = state.emit(now).unwrap().unwrap();
    assert_eq!(wire::decode(&first.bytes, 1500).unwrap().payload, &[9; 64]);
    assert!(state.emit(now).unwrap().is_none());
    state
        .process(&segment(101, 533, Flags::ACK, 64, &[]), now)
        .unwrap();
    let retry = state.emit(now + Duration::from_secs(1)).unwrap().unwrap();
    let decoded = wire::decode(&retry.bytes, 1500).unwrap();
    assert_eq!(decoded.sequence, 533);
    assert_eq!(decoded.payload, &[9; 32]);
}
#[test]
fn zero_window_probe_and_reopen_does_not_consume_data() {
    let (mut state, now) = ready(0);
    state.output(b"tail").unwrap();
    assert!(state.emit(now).unwrap().is_none());
    let probe = state.emit(now + Duration::from_secs(1)).unwrap().unwrap();
    assert_eq!(wire::decode(&probe.bytes, 1500).unwrap().sequence, 500);
    assert_eq!(state.next, 501);
    state
        .process(&segment(101, 501, Flags::ACK, 100, &[]), now)
        .unwrap();
    let data = state.emit(now).unwrap().unwrap();
    let decoded = wire::decode(&data.bytes, 1500).unwrap();
    assert_eq!(decoded.sequence, 501);
    assert_eq!(decoded.payload, b"tail");
}
#[test]
fn half_close_keeps_return_data_then_fin_and_time_wait() {
    let (mut state, now) = ready(4096);
    state
        .process(
            &segment(101, 501, Flags::ACK | Flags::FIN, 4096, b"request"),
            now,
        )
        .unwrap();
    assert_eq!(state.phase(), Phase::CloseWait);
    assert!(!state.input_finished());
    state.consume_input(7).unwrap();
    assert!(state.input_finished());
    state.output(b"response").unwrap();
    state.output_finished();
    let ack = state.emit(now).unwrap().unwrap();
    assert_eq!(wire::decode(&ack.bytes, 1500).unwrap().acknowledgement, 109);
    let data = state.emit(now).unwrap().unwrap();
    assert_eq!(
        wire::decode(&data.bytes, 1500).unwrap().payload,
        b"response"
    );
    let fin = state.emit(now).unwrap().unwrap();
    assert!(
        wire::decode(&fin.bytes, 1500)
            .unwrap()
            .flags
            .has(Flags::FIN)
    );
    assert_eq!(state.phase(), Phase::LastAck);
    state
        .process(&segment(109, 510, Flags::ACK, 4096, &[]), now)
        .unwrap();
    assert_eq!(state.phase(), Phase::TimeWait);
    state.emit(now + Duration::from_millis(60)).unwrap();
    assert_eq!(state.phase(), Phase::Closed);
}
#[test]
fn reset_requires_expected_sequence_and_far_packet_cannot_release_tx() {
    let (mut state, now) = ready(4096);
    state.output(b"kept").unwrap();
    state.emit(now).unwrap();
    state
        .process(&segment(9_000_000, 505, Flags::ACK, 4096, &[]), now)
        .unwrap();
    assert_eq!(state.acknowledged, 501);
    state
        .process(&segment(999, 0, Flags::RST, 4096, &[]), now)
        .unwrap();
    assert_eq!(state.phase(), Phase::Established);
    state
        .process(&segment(101, 0, Flags::RST, 4096, &[]), now)
        .unwrap();
    assert_eq!(state.phase(), Phase::Closed);
}

#[test]
fn duplicate_ack_triggers_fast_retransmit_without_waiting_for_timer() {
    let (mut state, now) = ready(4096);
    state.output(&[7; 1200]).unwrap();
    let first = state.emit(now).unwrap().unwrap();
    for _ in 0..3 {
        state
            .process(&segment(101, 501, Flags::ACK, 4096, &[]), now)
            .unwrap();
    }
    let retried = state.emit(now).unwrap().unwrap();
    assert_eq!(first.bytes, retried.bytes);
}
#[test]
fn exponential_retry_budget_exhausts_with_explicit_timeout() {
    // 重传指数退避在已建立的数据阶段验证；握手另受独立 30 秒硬期限限制。
    let (mut state, now) = ready(4096);
    state.output(b"retry budget").unwrap();
    state.emit(now).unwrap().unwrap();
    let mut elapsed = 0;
    for delay in [1, 2, 4, 8, 16] {
        elapsed += delay;
        state
            .emit(now + Duration::from_secs(elapsed))
            .unwrap()
            .unwrap();
    }
    assert!(matches!(
        state.emit(now + Duration::from_secs(elapsed + 32)),
        Err(Error::NetworkTimeout)
    ));
}
#[test]
fn active_half_close_accepts_final_return_bytes_and_fin() {
    let (mut state, now) = ready(4096);
    state.output_finished();
    let fin = state.emit(now).unwrap().unwrap();
    assert!(
        wire::decode(&fin.bytes, 1500)
            .unwrap()
            .flags
            .has(Flags::FIN)
    );
    state
        .process(&segment(101, 502, Flags::ACK, 4096, &[]), now)
        .unwrap();
    assert_eq!(state.phase(), Phase::FinWaitTwo);
    state
        .process(
            &segment(101, 502, Flags::ACK | Flags::FIN, 4096, b"tail"),
            now,
        )
        .unwrap();
    assert_eq!(&*state.input(10), b"tail");
    assert_eq!(state.phase(), Phase::TimeWait);
    let ack = state.emit(now).unwrap().unwrap();
    assert_eq!(wire::decode(&ack.bytes, 1500).unwrap().acknowledgement, 106);
}

#[test]
fn round_trip_estimate_adapts_and_retransmission_is_not_a_sample() {
    let (mut state, now) = ready(4096);
    state.output(b"abc").unwrap();
    state.emit(now).unwrap().unwrap();
    state
        .process(
            &segment(101, 504, Flags::ACK, 4096, &[]),
            now + Duration::from_secs(3),
        )
        .unwrap();
    assert!(state.base_retransmit >= Duration::from_secs(3));
    let estimate = state.smoothed_round_trip;
    let sent = now + Duration::from_secs(4);
    state.output(b"def").unwrap();
    state.emit(sent).unwrap().unwrap();
    let retry = sent + state.base_retransmit;
    state.emit(retry).unwrap().unwrap();
    state
        .process(
            &segment(101, 507, Flags::ACK, 4096, &[]),
            retry + Duration::from_millis(1),
        )
        .unwrap();
    assert_eq!(state.smoothed_round_trip, estimate);
    assert_eq!(state.retransmit, state.base_retransmit);
}

#[test]
fn finish_waits_for_bounded_flight_ledger_space() {
    let (mut state, now) = ready(CAPACITY);
    state.maximum_segment = 64;
    state.congestion_window = CAPACITY;
    state.output(&[3; 64 * MAXIMUM_FLIGHT_SEGMENTS]).unwrap();
    state.output_finished();
    for _ in 0..MAXIMUM_FLIGHT_SEGMENTS {
        state.emit(now).unwrap().unwrap();
    }
    assert_eq!(state.outstanding.len(), MAXIMUM_FLIGHT_SEGMENTS);
    // FIN 同样占用序号和在途记录，不能在满额时扩容或制造即时空转。
    assert!(state.emit(now).unwrap().is_none());
    assert!(state.deadline(now).unwrap() > now);
    state
        .process(&segment(101, 565, Flags::ACK, CAPACITY, &[]), now)
        .unwrap();
    let packet = state.emit(now).unwrap().unwrap();
    assert!(
        wire::decode(&packet.bytes, 1500)
            .unwrap()
            .flags
            .has(Flags::FIN)
    );
    assert_eq!(state.outstanding.len(), MAXIMUM_FLIGHT_SEGMENTS);
}

#[test]
fn duplicate_syn_consumes_retry_budget_and_handshake_has_absolute_deadline() {
    let now = Instant::now();
    let syn = segment(100, 0, Flags::SYN, 4096, &[]);
    let mut state = State::new(&syn, 500, 1500, Duration::from_secs(1), now).unwrap();
    state.emit(now).unwrap().unwrap();
    for _ in 0..MAXIMUM_RETRIES {
        state.process(&syn, now).unwrap();
        state.emit(now).unwrap().unwrap();
    }
    state.process(&syn, now).unwrap();
    assert!(matches!(state.emit(now), Err(Error::NetworkTimeout)));
    let mut state = State::new(&syn, 500, 1500, Duration::from_secs(1), now).unwrap();
    state.emit(now).unwrap().unwrap();
    // 无效 ACK 与重复 SYN 都不能取得新的握手授权时间。
    state
        .process(
            &segment(101, 900, Flags::ACK, 4096, &[]),
            now + Duration::from_secs(29),
        )
        .unwrap();
    state.process(&syn, now + Duration::from_secs(29)).unwrap();
    assert!(matches!(
        state.emit(now + Duration::from_secs(30)),
        Err(Error::NetworkTimeout)
    ));
}
