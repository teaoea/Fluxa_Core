//! TCP 序号、确认、窗口、重传及双向关闭；独立于网络 I/O，便于故障与回绕测试。
use super::{
    buffer::{CAPACITY, Receive, Transmit},
    wire::{self, Flags, Segment, before, distance},
};
use crate::{
    error::{Error, Result},
    tun::Packet,
};
use std::{
    collections::VecDeque,
    net::SocketAddr,
    time::{Duration, Instant},
};
const INITIAL_RETRANSMIT: Duration = Duration::from_secs(1);
const MAXIMUM_RETRANSMIT: Duration = Duration::from_secs(60);
const MAXIMUM_RETRIES: u8 = 5;
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(30);
const MAXIMUM_FLIGHT_SEGMENTS: usize = 64;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Phase {
    SynReceived,
    Established,
    CloseWait,
    FinWaitOne,
    FinWaitTwo,
    Closing,
    LastAck,
    TimeWait,
    Closed,
}
#[derive(Clone, Copy)]
enum SentKind {
    Syn,
    Data(usize),
    Fin,
}
struct Sent {
    sequence: u32,
    kind: SentKind,
    sent_at: Option<Instant>,
    retries: u8,
    sacked: bool,
    repaired: bool,
}
impl Sent {
    fn length(&self) -> usize {
        match self.kind {
            SentKind::Data(size) => size,
            SentKind::Syn | SentKind::Fin => 1,
        }
    }
}
pub struct State {
    source: SocketAddr,
    destination: SocketAddr,
    peer_initial: u32,
    handshake_until: Instant,
    initial: u32,
    acknowledged: u32,
    next: u32,
    receive: Receive,
    transmit: Transmit,
    outstanding: VecDeque<Sent>,
    peer_window: usize,
    sack_enabled: bool,
    recovery_end: u32,
    window_sequence: u32,
    window_acknowledgement: u32,
    maximum_segment: usize,
    mtu: usize,
    established: bool,
    acknowledgement_pending: bool,
    eof: bool,
    finish_sequence: Option<u32>,
    finish_acknowledged: bool,
    closed: bool,
    passive_close: bool,
    time_wait_until: Option<Instant>,
    time_wait: Duration,
    retransmit: Duration,
    base_retransmit: Duration,
    smoothed_round_trip: Option<Duration>,
    round_trip_variation: Duration,
    congestion_window: usize,
    slow_start_threshold: usize,
    duplicate_acknowledgements: usize,
    fast_retransmit: bool,
    recovery: bool,
    last_probe: Instant,
}
impl State {
    pub fn new(
        syn: &Segment<'_>,
        initial: u32,
        mtu: usize,
        time_wait: Duration,
        now: Instant,
    ) -> Result<Self> {
        if !(crate::tun::MIN_MTU..=crate::tun::MAX_MTU).contains(&mtu)
            || time_wait.is_zero()
            || time_wait > Duration::from_secs(120)
            || syn.window > u16::MAX as usize
            || syn.source.port() == 0
            || syn.destination.port() == 0
            || syn.source.ip().is_unspecified()
            || syn.destination.ip().is_unspecified()
            || syn.source.ip().is_multicast()
            || syn.destination.ip().is_multicast()
            || syn.source.ip() == std::net::IpAddr::V4(std::net::Ipv4Addr::BROADCAST)
            || syn.destination.ip() == std::net::IpAddr::V4(std::net::Ipv4Addr::BROADCAST)
            || syn.source.is_ipv4() != syn.destination.is_ipv4()
            || !syn.sacks.blocks().is_empty()
            || syn.maximum_segment.is_some_and(|value| value < 64)
        {
            return Err(Error::InvalidInput);
        }
        if !syn.flags.has(Flags::SYN)
            || syn.flags.has(Flags::ACK | Flags::FIN | Flags::RST)
            || !syn.payload.is_empty()
        {
            return Err(Error::Unsupported);
        }
        let maximum_segment = usize::from(syn.maximum_segment.unwrap_or(if syn.source.is_ipv4() {
            536
        } else {
            1220
        }))
        // 为四块 SACK 预留最多 36 字节，重传无需重新切分发送记录。
        .min(
            mtu - if syn.source.is_ipv4() { 40 } else { 60 }
                - if syn.sack_permitted { 36 } else { 0 },
        );
        let mut outstanding = VecDeque::with_capacity(MAXIMUM_FLIGHT_SEGMENTS);
        outstanding.push_back(Sent {
            sequence: initial,
            kind: SentKind::Syn,
            sent_at: None,
            retries: 0,
            sacked: false,
            repaired: false,
        });
        Ok(Self {
            source: syn.destination,
            destination: syn.source,
            peer_initial: syn.sequence,
            handshake_until: now + HANDSHAKE_TIMEOUT,
            initial,
            acknowledged: initial,
            next: initial.wrapping_add(1),
            receive: Receive::new(syn.sequence.wrapping_add(1)),
            transmit: Transmit::new(initial.wrapping_add(1)),
            outstanding,
            peer_window: syn.window,
            sack_enabled: syn.sack_permitted,
            recovery_end: initial,
            window_sequence: syn.sequence,
            window_acknowledgement: initial,
            maximum_segment,
            mtu,
            established: false,
            acknowledgement_pending: false,
            eof: false,
            finish_sequence: None,
            finish_acknowledged: false,
            closed: false,
            passive_close: false,
            time_wait_until: None,
            time_wait,
            retransmit: INITIAL_RETRANSMIT,
            base_retransmit: INITIAL_RETRANSMIT,
            smoothed_round_trip: None,
            round_trip_variation: Duration::ZERO,
            congestion_window: (10 * maximum_segment).min(CAPACITY),
            slow_start_threshold: CAPACITY,
            duplicate_acknowledgements: 0,
            fast_retransmit: false,
            recovery: false,
            last_probe: now,
        })
    }
    pub fn handshake_deadline(&self) -> Instant {
        self.handshake_until
    }
    pub fn phase(&self) -> Phase {
        if self.closed {
            return Phase::Closed;
        }
        if !self.established {
            return Phase::SynReceived;
        }
        if self.time_wait_until.is_some() {
            return Phase::TimeWait;
        }
        match (
            self.finish_sequence.is_some(),
            self.finish_acknowledged,
            self.receive.finished,
        ) {
            (false, _, false) => Phase::Established,
            (false, _, true) => Phase::CloseWait,
            (true, false, false) => Phase::FinWaitOne,
            (true, true, false) => Phase::FinWaitTwo,
            (true, false, true) => {
                if self.passive_close {
                    Phase::LastAck
                } else {
                    Phase::Closing
                }
            }
            (true, true, true) => Phase::TimeWait,
        }
    }
    pub fn pending_input(&self) -> usize {
        self.receive.pending()
    }
    pub fn input(&self, maximum: usize) -> zeroize::Zeroizing<Vec<u8>> {
        self.receive.copy_pending(maximum)
    }
    pub fn consume_input(&mut self, count: usize) -> Result<()> {
        self.receive.consume(count)?;
        self.acknowledgement_pending = true;
        Ok(())
    }
    pub fn input_finished(&self) -> bool {
        self.receive.finished && self.receive.pending() == 0
    }
    pub fn output_capacity(&self) -> usize {
        if self.established && !self.eof && !self.closed {
            self.transmit.available()
        } else {
            0
        }
    }
    pub fn output(&mut self, bytes: &[u8]) -> Result<()> {
        if !self.established || self.eof {
            return Err(Error::InvalidState);
        }
        self.transmit.push(bytes)
    }
    pub fn output_finished(&mut self) {
        self.eof = true;
    }
    pub fn process(&mut self, segment: &Segment<'_>, now: Instant) -> Result<()> {
        if segment.source != self.destination
            || segment.destination != self.source
            || segment.window > u16::MAX as usize
        {
            return Err(Error::InvalidInput);
        }
        if self.closed {
            return Err(Error::InvalidState);
        }
        if segment.flags.has(Flags::RST) {
            if segment.sequence == self.receive.next() {
                self.closed = true;
            } else {
                self.acknowledgement_pending = true;
            }
            return Ok(());
        }
        if segment.flags.has(Flags::SYN) {
            if !self.established
                && segment.sequence == self.peer_initial
                && !segment.flags.has(Flags::ACK)
                && segment.payload.is_empty()
            {
                // 立即重发仍保留原发送记录，必须消耗重试预算并遵守握手期限。
                self.fast_retransmit = true;
            } else {
                self.acknowledgement_pending = true;
            }
            return Ok(());
        }
        let next_receive = self.receive.next();
        let length = segment.payload.len() + usize::from(segment.flags.has(Flags::FIN));
        let sequence_acceptable = if length == 0 {
            segment.sequence == next_receive
                || (!before(segment.sequence, next_receive)
                    && distance(segment.sequence, next_receive)
                        < usize::from(self.receive.window()))
        } else {
            // 旧重复包回应 ACK；它不能更新发送确认或窗口。
            !before(segment.sequence, next_receive)
                && distance(segment.sequence, next_receive) < usize::from(self.receive.window())
                || (before(segment.sequence, next_receive)
                    && before(next_receive, segment.sequence.wrapping_add(length as u32)))
        };
        if !sequence_acceptable {
            self.acknowledgement_pending = true;
            return Ok(());
        }
        if !segment.flags.has(Flags::ACK) {
            return Ok(());
        }
        let acknowledgement = segment.acknowledgement;
        if before(self.next, acknowledgement) {
            self.acknowledgement_pending = true;
            return Ok(());
        }
        if !self.established && acknowledgement != self.initial.wrapping_add(1) {
            self.acknowledgement_pending = true;
            return Ok(());
        }
        let previous_window = self.peer_window;
        if before(self.window_sequence, segment.sequence)
            || (self.window_sequence == segment.sequence
                && !before(acknowledgement, self.window_acknowledgement))
        {
            self.peer_window = segment.window;
            self.window_sequence = segment.sequence;
            self.window_acknowledgement = acknowledgement;
        }
        if before(self.acknowledged, acknowledgement) {
            let mut data = 0;
            let mut sample = None;
            let mut ambiguous = false;
            while let Some(sent) = self.outstanding.front_mut() {
                let end = sent.sequence.wrapping_add(sent.length() as u32);
                if !before(acknowledgement, end) {
                    let sent = self.outstanding.pop_front().ok_or(Error::Internal)?;
                    ambiguous |= sent.retries != 0;
                    if sample.is_none() && sent.retries == 0 {
                        sample = sent.sent_at.map(|at| now.saturating_duration_since(at));
                    }
                    match sent.kind {
                        SentKind::Syn => self.established = true,
                        SentKind::Data(size) => data += size,
                        SentKind::Fin => self.finish_acknowledged = true,
                    }
                } else {
                    if before(sent.sequence, acknowledgement) {
                        let count = distance(acknowledgement, sent.sequence);
                        if let SentKind::Data(size) = sent.kind {
                            data += count;
                            sent.sequence = acknowledgement;
                            sent.kind = SentKind::Data(size - count);
                        }
                    }
                    break;
                }
            }
            self.transmit.acknowledge(data)?;
            self.acknowledged = acknowledgement;
            self.duplicate_acknowledgements = 0;
            // RFC 6298 / Karn：一次累计 ACK 最多采样一次，重传歧义不更新 RTT。
            if !ambiguous && let Some(sample) = sample {
                self.observe_round_trip(sample);
            }
            self.retransmit = self.base_retransmit;
            if self.recovery && (!self.sack_enabled || !before(acknowledgement, self.recovery_end))
            {
                self.congestion_window = self.slow_start_threshold;
                self.recovery = false;
            } else if data != 0 && !self.recovery {
                let increment = if self.congestion_window < self.slow_start_threshold {
                    data.min(self.maximum_segment)
                } else {
                    (self.maximum_segment * self.maximum_segment / self.congestion_window).max(1)
                };
                self.congestion_window = (self.congestion_window + increment).min(CAPACITY);
            }
        } else if acknowledgement == self.acknowledged
            && segment.payload.is_empty()
            && previous_window == self.peer_window
            && !self.outstanding.is_empty()
        {
            self.duplicate_acknowledgements =
                self.duplicate_acknowledgements.saturating_add(1).min(4);
            if self.duplicate_acknowledgements == 3 && !self.sack_enabled {
                self.slow_start_threshold =
                    (distance(self.next, self.acknowledged) / 2).max(2 * self.maximum_segment);
                self.congestion_window =
                    (self.slow_start_threshold + 3 * self.maximum_segment).min(CAPACITY);
                self.fast_retransmit = true;
                self.recovery = true;
            } else if self.recovery && !self.sack_enabled {
                self.congestion_window =
                    (self.congestion_window + self.maximum_segment).min(CAPACITY);
            }
        }
        if self.sack_enabled && self.established && acknowledgement == self.acknowledged {
            self.observe_sacks(&segment.sacks);
            if !self.recovery && self.sack_losses().iter().any(|lost| *lost) {
                self.slow_start_threshold =
                    (distance(self.next, self.acknowledged) / 2).max(2 * self.maximum_segment);
                self.congestion_window = self.slow_start_threshold.min(CAPACITY);
                self.recovery = true;
                self.recovery_end = self.next;
            }
        }
        if self.established && (!segment.payload.is_empty() || segment.flags.has(Flags::FIN)) {
            self.receive.accept(
                segment.sequence,
                segment.payload,
                segment.flags.has(Flags::FIN),
            );
            self.acknowledgement_pending = true;
        }
        if self.receive.finished && self.finish_acknowledged && self.time_wait_until.is_none() {
            self.time_wait_until = Some(now + self.time_wait);
        }
        Ok(())
    }
    /// SACK 只是提示；只标记完全覆盖的已发送数据记录，不释放正文或推进累计 ACK。
    fn observe_sacks(&mut self, sacks: &wire::Sacks) {
        for &(left, right) in sacks.blocks() {
            if before(left, self.acknowledged) || before(self.next, right) {
                continue;
            }
            for sent in &mut self.outstanding {
                if matches!(sent.kind, SentKind::Data(_))
                    && !before(sent.sequence, left)
                    && !before(right, sent.sequence.wrapping_add(sent.length() as u32))
                {
                    sent.sacked = true;
                }
            }
        }
    }
    /// 逆序累计更高序号的选择确认，一次遍历最多 64 个发送记录。
    fn sack_losses(&self) -> [bool; MAXIMUM_FLIGHT_SEGMENTS] {
        let mut losses = [false; MAXIMUM_FLIGHT_SEGMENTS];
        let mut blocks = 0;
        let mut bytes = 0;
        for (index, sent) in self.outstanding.iter().enumerate().rev() {
            losses[index] = !sent.sacked
                && !sent.repaired
                && matches!(sent.kind, SentKind::Data(_))
                && (blocks >= 3
                    || bytes >= 3 * self.maximum_segment
                    || (index == 0 && self.duplicate_acknowledgements >= 3));
            if sent.sacked {
                blocks += 1;
                bytes += sent.length();
            }
        }
        losses
    }
    fn sack_retry(&self) -> Option<usize> {
        if !self.sack_enabled || !self.recovery {
            return None;
        }
        // 有界 pipe 估算：已 SACK 或判为丢失且未重传的不再占 pipe；重传仍计入。
        let losses = self.sack_losses();
        let pipe: usize = self
            .outstanding
            .iter()
            .enumerate()
            .filter(|(i, s)| !s.sacked && (s.repaired || !losses[*i]))
            .map(|(_, s)| s.length())
            .sum();
        (0..self.outstanding.len()).find(|&i| {
            let first_fast = i == 0
                && self.duplicate_acknowledgements >= 3
                && !self.outstanding.iter().any(|s| s.repaired);
            losses[i]
                && (pipe + self.outstanding[i].length() <= self.congestion_window || first_fast)
        })
    }
    fn observe_round_trip(&mut self, sample: Duration) {
        let sample = sample.min(MAXIMUM_RETRANSMIT);
        if let Some(previous) = self.smoothed_round_trip {
            self.round_trip_variation =
                (self.round_trip_variation * 3 + previous.abs_diff(sample)) / 4;
            self.smoothed_round_trip = Some((previous * 7 + sample) / 8);
        } else {
            self.smoothed_round_trip = Some(sample);
            self.round_trip_variation = sample / 2;
        }
        let smoothed = self.smoothed_round_trip.unwrap_or(sample);
        self.base_retransmit = (smoothed
            + (self.round_trip_variation * 4).max(Duration::from_millis(1)))
        .clamp(INITIAL_RETRANSMIT, MAXIMUM_RETRANSMIT);
    }
    fn packet(
        &self,
        sequence: u32,
        flags: u8,
        maximum_segment: Option<u16>,
        payload: &[u8],
    ) -> Result<Packet> {
        wire::encode(
            &Segment {
                source: self.source,
                destination: self.destination,
                sequence,
                acknowledgement: self.receive.next(),
                flags: Flags(flags),
                window: usize::from(self.receive.window()),
                maximum_segment,
                sack_permitted: self.sack_enabled && flags & Flags::SYN != 0,
                sacks: if self.sack_enabled
                    && self.established
                    && flags & (Flags::SYN | Flags::RST) == 0
                {
                    self.receive.sacks()
                } else {
                    wire::Sacks::default()
                },
                payload,
            },
            self.mtu,
        )
    }
    pub fn deadline(&self, now: Instant) -> Option<Instant> {
        if self.closed {
            return None;
        }
        if self.acknowledgement_pending || self.fast_retransmit || self.sack_retry().is_some() {
            return Some(now);
        }
        if let Some(until) = self.time_wait_until {
            return Some(until);
        }
        if self.established {
            let allowed = self
                .peer_window
                .min(self.congestion_window)
                .saturating_sub(distance(self.next, self.acknowledged));
            if allowed != 0
                && self.outstanding.len() < MAXIMUM_FLIGHT_SEGMENTS
                && (self.transmit.unsent() != 0
                    || (self.eof && self.transmit.unsent() == 0 && self.finish_sequence.is_none()))
            {
                return Some(now);
            }
        }
        let retransmit = self
            .outstanding
            .front()
            .map(|sent| sent.sent_at.map_or(now, |at| at + self.retransmit));
        let probe = (self.peer_window == 0 && (self.transmit.unsent() != 0 || self.eof))
            .then_some(self.last_probe + INITIAL_RETRANSMIT);
        retransmit
            .into_iter()
            .chain(probe)
            .chain((!self.established).then_some(self.handshake_until))
            .min()
    }
    pub fn reset(&self) -> Result<Packet> {
        self.packet(
            if self.established { self.next } else { 0 },
            Flags::RST | Flags::ACK,
            None,
            &[],
        )
    }
    /// 每次最多产生一个包；调用者先预留队列槽，再提交状态变化。
    pub fn emit(&mut self, now: Instant) -> Result<Option<Packet>> {
        if self.closed {
            return Ok(None);
        }
        if !self.established && now >= self.handshake_until {
            return Err(Error::NetworkTimeout);
        }
        if self.time_wait_until.is_some_and(|until| now >= until) {
            self.closed = true;
            return Ok(None);
        }
        let timer_due = self.outstanding.front().is_some_and(|sent| {
            sent.sent_at.is_none()
                || now.saturating_duration_since(sent.sent_at.unwrap_or(now)) >= self.retransmit
        });
        let retry_index = if timer_due || self.fast_retransmit {
            Some(0)
        } else {
            self.sack_retry()
        };
        if let Some(index) = retry_index
            && let Some(sent) = self.outstanding.get(index)
        {
            if sent.retries >= MAXIMUM_RETRIES {
                return Err(Error::NetworkTimeout);
            }
            let sequence = sent.sequence;
            let (flags, mss, payload) = match sent.kind {
                SentKind::Syn => (
                    Flags::SYN | Flags::ACK,
                    Some(self.maximum_segment as u16),
                    zeroize::Zeroizing::new(Vec::new()),
                ),
                SentKind::Data(size) => (
                    Flags::ACK | Flags::PSH,
                    None,
                    self.transmit.copy(sequence, size)?,
                ),
                SentKind::Fin => (
                    Flags::FIN | Flags::ACK,
                    None,
                    zeroize::Zeroizing::new(Vec::new()),
                ),
            };
            let packet = self.packet(sequence, flags, mss, &payload)?;
            let timed = timer_due && sent.sent_at.is_some() && !self.fast_retransmit;
            if timed {
                // 对端可以撤回 SACK；RTO 清除全部提示，不能永久跳过旧数据。
                for entry in &mut self.outstanding {
                    entry.sacked = false;
                    entry.repaired = false;
                }
                self.duplicate_acknowledgements = 0;
            }
            let sent = self.outstanding.get_mut(index).ok_or(Error::Internal)?;
            if sent.sent_at.is_some() {
                sent.retries += 1;
            }
            sent.sent_at = Some(now);
            sent.repaired = !timed && self.sack_enabled && matches!(sent.kind, SentKind::Data(_));
            self.fast_retransmit = false;
            if timed {
                if matches!(sent.kind, SentKind::Syn) {
                    self.base_retransmit = Duration::from_secs(3);
                }
                self.retransmit = (self.retransmit * 2).min(MAXIMUM_RETRANSMIT);
                self.slow_start_threshold =
                    (distance(self.next, self.acknowledged) / 2).max(2 * self.maximum_segment);
                self.congestion_window = self.maximum_segment;
                self.recovery = false;
            }
            self.acknowledgement_pending = false;
            return Ok(Some(packet));
        }
        if self.acknowledgement_pending {
            let packet = self.packet(self.next, Flags::ACK, None, &[])?;
            self.acknowledgement_pending = false;
            return Ok(Some(packet));
        }
        if !self.established || self.time_wait_until.is_some() {
            return Ok(None);
        }
        let flight = distance(self.next, self.acknowledged);
        let allowed = self
            .peer_window
            .min(self.congestion_window)
            .saturating_sub(flight);
        if allowed != 0
            && self.transmit.unsent() != 0
            && self.outstanding.len() < MAXIMUM_FLIGHT_SEGMENTS
        {
            let count = self
                .maximum_segment
                .min(allowed)
                .min(self.transmit.unsent());
            let sequence = self.transmit.base.wrapping_add(self.transmit.sent as u32);
            let payload = self.transmit.copy(sequence, count)?;
            let packet = self.packet(sequence, Flags::ACK | Flags::PSH, None, &payload)?;
            self.outstanding.push_back(Sent {
                sequence,
                kind: SentKind::Data(count),
                sent_at: Some(now),
                retries: 0,
                sacked: false,
                repaired: false,
            });
            self.transmit.sent += count;
            self.next = self.next.wrapping_add(count as u32);
            return Ok(Some(packet));
        }
        if allowed != 0
            && self.eof
            && self.transmit.unsent() == 0
            && self.finish_sequence.is_none()
            && self.outstanding.len() < MAXIMUM_FLIGHT_SEGMENTS
        {
            let sequence = self.next;
            let packet = self.packet(sequence, Flags::FIN | Flags::ACK, None, &[])?;
            self.outstanding.push_back(Sent {
                sequence,
                kind: SentKind::Fin,
                sent_at: Some(now),
                retries: 0,
                sacked: false,
                repaired: false,
            });
            self.passive_close = self.receive.finished;
            self.finish_sequence = Some(sequence);
            self.next = self.next.wrapping_add(1);
            return Ok(Some(packet));
        }
        if self.peer_window == 0
            && (self.transmit.unsent() != 0 || self.eof)
            && now.duration_since(self.last_probe) >= INITIAL_RETRANSMIT
        {
            // 已确认序号之前的一个字节仅探测窗口，不交付新应用字节或推进发送序号。
            let packet = self.packet(self.acknowledged.wrapping_sub(1), Flags::ACK, None, &[0])?;
            self.last_probe = now;
            return Ok(Some(packet));
        }
        Ok(None)
    }
}
#[cfg(test)]
#[path = "../../../tests/test_tun_tcp_state.rs"]
mod test_tun_tcp_state;
