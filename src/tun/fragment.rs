//! 本端生成的 UDP 返回报文分片；不作为路由器清除外部报文的 DF。
use super::{MAX_MTU, MIN_MTU, Packet, checksum, packet::encode_udp};
use crate::error::{Error, Result};
use std::{
    collections::VecDeque,
    net::SocketAddr,
    sync::Mutex,
    time::{Duration, Instant},
};

/// 一槽保存一个完整数据报；只有全部包构造成功后才能交付队列。
pub(crate) struct Datagram(pub(super) VecDeque<Packet>);
#[derive(Default)]
struct Identifiers {
    next: u64,
    last: Option<Instant>,
}
static IPV4: Mutex<Identifiers> = Mutex::new(Identifiers {
    next: 0,
    last: None,
});
static IPV6: Mutex<Identifiers> = Mutex::new(Identifiers {
    next: 0,
    last: None,
});
impl Identifiers {
    fn take(&mut self, maximum: u64, now: Instant) -> Result<u32> {
        if self.next > maximum {
            // 全局序列耗尽后，必须经历至少 120 秒静默期；关闭会话不重置序列。
            if self
                .last
                .is_none_or(|last| now.saturating_duration_since(last) < Duration::from_secs(120))
            {
                return Err(Error::ResourceLimit);
            }
            self.next = 0;
        }
        let id = self.next as u32;
        self.next += 1;
        self.last = Some(now);
        Ok(id)
    }
}

/// 分片及 UDP 流共用不复用标识；耗尽后要求静默期，避免旧引用误关联到新流。
pub(super) fn identity(ipv4: bool) -> Result<u32> {
    (if ipv4 { &IPV4 } else { &IPV6 })
        .lock()
        .map_err(|_| Error::Internal)?
        .take(
            if ipv4 {
                u16::MAX as u64
            } else {
                u32::MAX as u64
            },
            Instant::now(),
        )
}

pub(super) fn flow_identity(ipv4: bool) -> Result<u32> {
    if ipv4 {
        return identity(true);
    }
    static LABELS: Mutex<Identifiers> = Mutex::new(Identifiers {
        next: 0,
        last: None,
    });
    LABELS
        .lock()
        .map_err(|_| Error::Internal)?
        .take(0xfffff, Instant::now())
}

/// 完整报文最多 9000 字节，MTU 至少 1280，最多产生八片。
pub(crate) fn encode(
    source: SocketAddr,
    destination: SocketAddr,
    payload: &[u8],
    mtu: usize,
) -> Result<Datagram> {
    if !(MIN_MTU..=MAX_MTU).contains(&mtu) {
        return Err(Error::InvalidInput);
    }
    encode_path(source, destination, payload, mtu)
}
/// 经路径反馈降低的 IPv4 MTU 可以小于接口配置下限；仍保留最多八片与总字节预算。
pub(crate) fn encode_path(
    source: SocketAddr,
    destination: SocketAddr,
    payload: &[u8],
    mtu: usize,
) -> Result<Datagram> {
    if !(if source.is_ipv4() { 68 } else { MIN_MTU }..=MAX_MTU).contains(&mtu) {
        return Err(Error::InvalidInput);
    }
    let packet = encode_udp(source, destination, payload, MAX_MTU)?;
    if packet.bytes.len() <= mtu {
        return Ok(Datagram(VecDeque::from([packet])));
    }
    let ipv4 = source.is_ipv4();
    let ip = if ipv4 { 20 } else { 40 };
    let header = if ipv4 { 20 } else { 48 };
    let chunk = (mtu - header) / 8 * 8;
    let transport = &packet.bytes[ip..];
    let count = transport.len().div_ceil(chunk);
    if count > 8 {
        return Err(Error::ResourceLimit);
    }
    let id = identity(ipv4)?;
    let mut packets = VecDeque::with_capacity(count);
    for (index, body) in transport.chunks(chunk).enumerate() {
        let offset = index * chunk;
        let more = index + 1 < count;
        packets.push_back(Packet::build(header + body.len(), |bytes| {
            bytes[..ip].copy_from_slice(&packet.bytes[..ip]);
            if ipv4 {
                bytes[2..4].copy_from_slice(&((header + body.len()) as u16).to_be_bytes());
                bytes[4..6].copy_from_slice(&(id as u16).to_be_bytes());
                bytes[6..8].copy_from_slice(
                    &((offset as u16 / 8) | if more { 0x2000 } else { 0 }).to_be_bytes(),
                );
                bytes[10..12].fill(0);
                let sum = checksum(&bytes[..20], 0);
                bytes[10..12].copy_from_slice(&sum.to_be_bytes());
            } else {
                bytes[4..6].copy_from_slice(&((8 + body.len()) as u16).to_be_bytes());
                bytes[6] = 44;
                bytes[40] = 17;
                bytes[42..44].copy_from_slice(&((offset as u16) | u16::from(more)).to_be_bytes());
                bytes[44..48].copy_from_slice(&id.to_be_bytes());
            }
            bytes[header..].copy_from_slice(body);
        })?);
    }
    Ok(Datagram(packets))
}

#[cfg(test)]
#[path = "../../tests/test_tun_fragment.rs"]
mod test_tun_fragment;
