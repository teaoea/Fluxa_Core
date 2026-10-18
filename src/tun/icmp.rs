//! ICMP 错误输入边界；引用报文只能定位已经存在的流，不能创建出站或更改路由。
use super::{MAX_MTU, checksum, pseudo_sum, word};
use crate::error::{Error, Result};
use std::{
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    time::{Duration, Instant},
};
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Quote {
    pub source: SocketAddr,
    pub destination: SocketAddr,
    pub protocol: u8,
    pub size: usize,
    pub tag: [u8; 8],
    identifier: u32,
}
#[derive(Clone, Copy, Debug)]
pub(super) enum Kind {
    TooBig(usize),
    Unreachable,
    TimeExceeded,
}
#[derive(Clone, Copy, Debug)]
pub(super) struct Notice {
    pub quote: Quote,
    pub kind: Kind,
}
fn address(bytes: &[u8], v6: bool, source: bool) -> Result<IpAddr> {
    let offset = if v6 {
        if source { 8 } else { 24 }
    } else if source {
        12
    } else {
        16
    };
    let ip = if v6 {
        IpAddr::V6(Ipv6Addr::from(
            <[u8; 16]>::try_from(&bytes[offset..offset + 16]).map_err(|_| Error::InvalidInput)?,
        ))
    } else {
        IpAddr::V4(Ipv4Addr::new(
            bytes[offset],
            bytes[offset + 1],
            bytes[offset + 2],
            bytes[offset + 3],
        ))
    };
    if ip.is_unspecified() || ip.is_multicast() || ip == IpAddr::V4(Ipv4Addr::BROADCAST) {
        return Err(Error::InvalidInput);
    }
    Ok(ip)
}
/// 引用允许按标准只包含 IP 头和前八个传输字节；不使用普通完整包解析器读取截短正文。
pub(super) fn quote(bytes: &[u8]) -> Result<Quote> {
    let v6 = match bytes.first().ok_or(Error::InvalidInput)? >> 4 {
        4 => false,
        6 => true,
        _ => return Err(Error::InvalidInput),
    };
    let ip = if v6 { 40 } else { 20 };
    if bytes.len() < ip + 8 {
        return Err(Error::InvalidInput);
    }
    let size = if v6 {
        usize::from(word(bytes, 4)) + 40
    } else {
        usize::from(word(bytes, 2))
    };
    if size < ip + 8 || size > MAX_MTU {
        return Err(Error::InvalidInput);
    }
    let mut protocol = bytes[if v6 { 6 } else { 9 }];
    let mut offset = ip;
    let identifier = if v6 {
        if protocol == 44 {
            if bytes.len() < 56 || size < 56 || word(bytes, 42) & 0xfffe != 0 {
                return Err(Error::InvalidInput);
            }
            protocol = bytes[40];
            offset = 48;
            u32::from_be_bytes(bytes[44..48].try_into().map_err(|_| Error::InvalidInput)?)
        } else {
            u32::from_be_bytes(bytes[..4].try_into().map_err(|_| Error::InvalidInput)?) & 0xfffff
        }
    } else {
        if bytes[0] != 0x45 || checksum(&bytes[..20], 0) != 0 || word(bytes, 6) & 0x9fff != 0 {
            return Err(Error::InvalidInput);
        }
        u32::from(word(bytes, 4))
    };
    if !matches!(protocol, 6 | 17) {
        return Err(Error::Unsupported);
    }
    let source = SocketAddr::new(address(bytes, v6, true)?, word(bytes, offset));
    let destination = SocketAddr::new(address(bytes, v6, false)?, word(bytes, offset + 2));
    if source.port() == 0 || destination.port() == 0 {
        return Err(Error::InvalidInput);
    }
    Ok(Quote {
        source,
        destination,
        protocol,
        size,
        identifier,
        tag: bytes[offset..offset + 8]
            .try_into()
            .map_err(|_| Error::InvalidInput)?,
    })
}
/// 输入已经经过 IP 重组/扩展头规范化；ICMPv6 校验和包含规范化后的伪首部。
pub(super) fn decode(bytes: &[u8]) -> Result<Notice> {
    let v6 = bytes.first().ok_or(Error::InvalidInput)? >> 4 == 6;
    let ip = if v6 { 40 } else { 20 };
    if bytes.len() < ip + 8 || bytes.len() > MAX_MTU {
        return Err(Error::InvalidInput);
    }
    let protocol = if v6 { 58 } else { 1 };
    if bytes[if v6 { 6 } else { 9 }] != protocol {
        return Err(Error::Unsupported);
    }
    if (!v6
        && (bytes[0] != 0x45
            || usize::from(word(bytes, 2)) != bytes.len()
            || checksum(&bytes[..20], 0) != 0
            || word(bytes, 6) & 0x3fff != 0))
        || (v6 && usize::from(word(bytes, 4)) + 40 != bytes.len())
    {
        return Err(Error::InvalidInput);
    }
    if bytes[if v6 { 7 } else { 8 }] == 0 {
        return Err(Error::InvalidInput);
    }
    address(bytes, v6, true)?;
    let destination = address(bytes, v6, false)?;
    if checksum(
        &bytes[ip..],
        if v6 {
            pseudo_sum(bytes, 6, 58, bytes.len() - 40)
        } else {
            0
        },
    ) != 0
    {
        return Err(Error::InvalidInput);
    }
    let kind = match (v6, bytes[ip], bytes[ip + 1]) {
        (false, 3, 4) => Kind::TooBig(usize::from(word(bytes, ip + 6))),
        (true, 2, 0) => Kind::TooBig(u32::from_be_bytes(
            bytes[ip + 4..ip + 8]
                .try_into()
                .map_err(|_| Error::InvalidInput)?,
        ) as usize),
        (false, 3, 0..=15) | (true, 1, 0..=7) => Kind::Unreachable,
        (false, 11, 0..=1) | (true, 3, 0..=1) => Kind::TimeExceeded,
        _ => return Err(Error::Unsupported),
    };
    let quote = quote(&bytes[ip + 8..])?;
    if quote.source.is_ipv6() != v6 || destination != quote.source.ip() {
        return Err(Error::InvalidInput);
    }
    Ok(Notice { quote, kind })
}
/// 只降低路径估计；IPv4 旧式零 MTU 使用 RFC 1191 平台值，IPv6 不低于 1280。
pub(super) fn reduced(current: usize, notice: Notice) -> Option<usize> {
    let Kind::TooBig(mut value) = notice.kind else {
        return None;
    };
    if value == 0 && notice.quote.source.is_ipv4() {
        value = [8166, 4352, 2002, 1492, 1006, 508, 296, 68]
            .into_iter()
            .find(|x| *x < notice.quote.size)?;
    }
    let minimum = if notice.quote.source.is_ipv4() {
        68
    } else {
        1280
    };
    value = value.max(minimum);
    (value < current && value < notice.quote.size).then_some(value)
}
/// 每个 UDP 活动流保留四个最近引用；没有会话级无限增长的目的地址缓存。
pub(super) struct Path {
    pub mtu: usize,
    identity: u32,
    recent: [Option<(Quote, Instant)>; 4],
    lowered: Option<Instant>,
}
const _: () = assert!(std::mem::size_of::<Path>() <= 1024);
impl Path {
    pub fn new(mtu: usize, ipv4: bool) -> Result<Self> {
        Ok(Self {
            mtu,
            identity: super::fragment::flow_identity(ipv4)?,
            recent: [None; 4],
            lowered: None,
        })
    }
    /// 非分片 UDP 用流内标识区分同四元组的不同生命周期；不保留敏感正文。
    pub fn stamp(&mut self, packet: &mut super::Packet, now: Instant) {
        let b = &mut packet.bytes;
        if b[0] >> 4 == 4 && word(b, 6) & 0x3fff == 0 {
            b[4..6].copy_from_slice(&(self.identity as u16).to_be_bytes());
            b[10..12].fill(0);
            let value = checksum(&b[..20], 0);
            b[10..12].copy_from_slice(&value.to_be_bytes());
        } else if b[0] >> 4 == 6 && b[6] != 44 {
            b[1] = (b[1] & 0xf0) | ((self.identity >> 16) as u8 & 15);
            b[2] = (self.identity >> 8) as u8;
            b[3] = self.identity as u8;
        }
        self.remember(b, now);
    }
    pub fn remember(&mut self, bytes: &[u8], now: Instant) {
        if let Ok(quote) = quote(bytes) {
            self.recent.rotate_right(1);
            self.recent[0] = Some((quote, now));
        }
    }
    pub fn expired(&self, now: Instant) -> bool {
        self.lowered
            .is_none_or(|at| now.saturating_duration_since(at) >= Duration::from_secs(600))
            && self
                .recent
                .iter()
                .flatten()
                .all(|(_, t)| now.saturating_duration_since(*t) >= Duration::from_secs(30))
    }
    pub fn apply(&mut self, notice: Notice, now: Instant) -> Result<()> {
        if !self.recent.iter().flatten().any(|(q, t)| {
            *q == notice.quote && now.saturating_duration_since(*t) < Duration::from_secs(30)
        }) {
            return Err(Error::InvalidInput);
        }
        if let Some(mtu) = reduced(self.mtu, notice) {
            self.mtu = mtu;
            self.lowered = Some(now);
        }
        Ok(())
    }
}
#[cfg(test)]
#[path = "../../tests/test_tun_icmp.rs"]
mod test_tun_icmp;
