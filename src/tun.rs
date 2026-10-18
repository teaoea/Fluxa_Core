//! TUN 数据包基础与有界 UDP 转发；不建立系统隧道。
//! 不持有调用方内存；会话先执行有界重组，无状态诊断仍只接受基础完整包。
mod fragment;
mod icmp;
mod ip;
mod packet;
pub use packet::Packet;
pub mod session;
pub mod tcp;
pub mod udp;
use crate::error::{Error, Result};
use serde::Serialize;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

pub const MAX_MTU: usize = 9000;
pub const MIN_MTU: usize = 1280;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PacketInfo {
    pub version: u8,
    pub source: String,
    pub destination: String,
    pub protocol: u8,
    pub source_port: u16,
    pub destination_port: u16,
    pub packet_bytes: usize,
    pub payload_bytes: usize,
}

impl PacketInfo {
    /// 转发端点共用校验；诊断解析本身仍可展示零端口等不可转发输入。
    pub(crate) fn endpoints(&self) -> Result<(SocketAddr, SocketAddr)> {
        let source = SocketAddr::new(
            self.source.parse().map_err(|_| Error::InvalidInput)?,
            self.source_port,
        );
        let destination = SocketAddr::new(
            self.destination.parse().map_err(|_| Error::InvalidInput)?,
            self.destination_port,
        );
        for endpoint in [source, destination] {
            if endpoint.port() == 0
                || endpoint.ip().is_unspecified()
                || endpoint.ip().is_multicast()
                || endpoint.ip() == IpAddr::V4(Ipv4Addr::BROADCAST)
            {
                return Err(Error::InvalidInput);
            }
        }
        Ok((source, destination))
    }
}

/// 会话保存计数与 MTU，不保存正文；close 后不能重新打开或继续使用。
pub struct PacketSession {
    mtu: usize,
    closed: bool,
    packets: u64,
    bytes: u64,
}
impl PacketSession {
    pub fn new(mtu: usize) -> Result<Self> {
        if !(MIN_MTU..=MAX_MTU).contains(&mtu) {
            return Err(Error::InvalidInput);
        }
        Ok(Self {
            mtu,
            closed: false,
            packets: 0,
            bytes: 0,
        })
    }
    pub fn inspect(&mut self, packet: &[u8]) -> Result<PacketInfo> {
        if self.closed {
            return Err(Error::InvalidState);
        }
        let info = parse(packet, self.mtu)?;
        let packets = self.packets.checked_add(1).ok_or(Error::ResourceLimit)?;
        let bytes = self
            .bytes
            .checked_add(packet.len() as u64)
            .ok_or(Error::ResourceLimit)?;
        self.packets = packets;
        self.bytes = bytes;
        Ok(info)
    }
    pub fn counters(&self) -> (u64, u64) {
        (self.packets, self.bytes)
    }
    pub fn close(&mut self) {
        self.closed = true;
    }
    /// 保留未来数据包转发入口；本阶段不得吞包、回显或伪造返回包。
    pub fn forward(&self, _packet: &[u8]) -> Result<()> {
        Err(if self.closed {
            Error::InvalidState
        } else {
            Error::Unsupported
        })
    }
}

/// 调用方提供完整 IP 包，不含 Ethernet 或 utun 的地址族前缀。
pub fn parse(packet: &[u8], mtu: usize) -> Result<PacketInfo> {
    if !(MIN_MTU..=MAX_MTU).contains(&mtu) {
        return Err(Error::InvalidInput);
    }
    if packet.len() > mtu {
        return Err(Error::ResourceLimit);
    }
    let version = packet.first().ok_or(Error::InvalidInput)? >> 4;
    let (source, destination, protocol, header_bytes) = match version {
        4 => {
            if packet.len() < 20 {
                return Err(Error::InvalidInput);
            }
            let header = usize::from(packet[0] & 15) * 4;
            if header < 20 || packet.len() < header || word(packet, 2) as usize != packet.len() {
                return Err(Error::InvalidInput);
            }
            // 只校验 IP 头，不接受保留位、过期 TTL、分片或 IPv4 选项。
            if checksum(&packet[..header], 0) != 0
                || packet[8] == 0
                || word(packet, 6) & 0x8000 != 0
            {
                return Err(Error::InvalidInput);
            }
            if word(packet, 6) & 0x3fff != 0 || header != 20 {
                return Err(Error::Unsupported);
            }
            (
                IpAddr::V4(Ipv4Addr::new(
                    packet[12], packet[13], packet[14], packet[15],
                )),
                IpAddr::V4(Ipv4Addr::new(
                    packet[16], packet[17], packet[18], packet[19],
                )),
                packet[9],
                header,
            )
        }
        6 => {
            if packet.len() < 40 || word(packet, 4) as usize + 40 != packet.len() || packet[7] == 0
            {
                return Err(Error::InvalidInput);
            }
            let src: [u8; 16] = packet[8..24].try_into().map_err(|_| Error::InvalidInput)?;
            let dst: [u8; 16] = packet[24..40].try_into().map_err(|_| Error::InvalidInput)?;
            (
                IpAddr::V6(Ipv6Addr::from(src)),
                IpAddr::V6(Ipv6Addr::from(dst)),
                packet[6],
                40,
            )
        }
        _ => return Err(Error::Unsupported),
    };
    if protocol != 6 && protocol != 17 {
        return Err(Error::Unsupported);
    }
    let segment = &packet[header_bytes..];
    let minimum = if protocol == 6 { 20 } else { 8 };
    if segment.len() < minimum {
        return Err(Error::InvalidInput);
    }
    let transport_header = if protocol == 6 {
        let length = usize::from(segment[12] >> 4) * 4;
        if length < 20 || length > segment.len() {
            return Err(Error::InvalidInput);
        }
        length
    } else {
        if word(segment, 4) as usize != segment.len() {
            return Err(Error::InvalidInput);
        }
        8
    };
    let checksum_offset = if protocol == 6 { 16 } else { 6 };
    let omitted = protocol == 17 && word(segment, checksum_offset) == 0;
    if (omitted && version == 6)
        || (!omitted
            && checksum(
                segment,
                pseudo_sum(packet, version, protocol, segment.len()),
            ) != 0)
    {
        return Err(Error::InvalidInput);
    }
    Ok(PacketInfo {
        version,
        source: source.to_string(),
        destination: destination.to_string(),
        protocol,
        source_port: word(segment, 0),
        destination_port: word(segment, 2),
        packet_bytes: packet.len(),
        payload_bytes: segment.len() - transport_header,
    })
}
// 所有偏移仅在父函数验证最小长度后读取；不使用裸指针或不安全索引。
fn word(bytes: &[u8], offset: usize) -> u16 {
    u16::from_be_bytes([bytes[offset], bytes[offset + 1]])
}
fn sum(bytes: &[u8]) -> u32 {
    bytes
        .chunks(2)
        .map(|pair| u32::from(pair[0]) * 256 + u32::from(*pair.get(1).unwrap_or(&0)))
        .sum()
}
fn pseudo_sum(packet: &[u8], version: u8, protocol: u8, length: usize) -> u32 {
    let addresses = if version == 4 {
        &packet[12..20]
    } else {
        &packet[8..40]
    };
    sum(addresses) + u32::from(protocol) + length as u32
}
fn checksum(bytes: &[u8], initial: u32) -> u16 {
    let mut value = initial + sum(bytes);
    while value >> 16 != 0 {
        value = (value & 0xffff) + (value >> 16);
    }
    !(value as u16)
}

#[cfg(test)]
#[path = "../tests/test_tun.rs"]
mod test_tun;
