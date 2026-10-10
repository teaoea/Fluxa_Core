//! TUN 数据包基础层：只检查数据包，不建立系统隧道或转发连接。
//! 无数据包队列，不持有调用方内存；分片和扩展头必须明确拒绝。
use crate::error::{Error, Result};
use serde::Serialize;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

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
mod tests {
    use super::*;
    fn packet(version: u8, protocol: u8) -> Vec<u8> {
        let ip = if version == 4 { 20 } else { 40 };
        let transport = if protocol == 6 { 20 } else { 8 };
        let mut bytes = vec![0; ip + transport + 3];
        bytes[0] = if version == 4 { 0x45 } else { 0x60 };
        if version == 4 {
            let len = bytes.len() as u16;
            bytes[2..4].copy_from_slice(&len.to_be_bytes());
            bytes[8] = 64;
            bytes[9] = protocol;
            bytes[12..16].copy_from_slice(&[192, 0, 2, 1]);
            bytes[16..20].copy_from_slice(&[198, 51, 100, 2]);
            let check = checksum(&bytes[..20], 0);
            bytes[10..12].copy_from_slice(&check.to_be_bytes());
        } else {
            bytes[4..6].copy_from_slice(&((transport + 3) as u16).to_be_bytes());
            bytes[6] = protocol;
            bytes[7] = 64;
            bytes[23] = 1;
            bytes[39] = 2;
        }
        bytes[ip..ip + 2].copy_from_slice(&12345_u16.to_be_bytes());
        bytes[ip + 2..ip + 4].copy_from_slice(&53_u16.to_be_bytes());
        let check_offset = if protocol == 6 {
            bytes[ip + 12] = 0x50;
            ip + 16
        } else {
            bytes[ip + 4..ip + 6].copy_from_slice(&((transport + 3) as u16).to_be_bytes());
            ip + 6
        };
        bytes[ip + transport..].copy_from_slice(b"abc");
        let check = checksum(
            &bytes[ip..],
            pseudo_sum(&bytes, version, protocol, transport + 3),
        );
        bytes[check_offset..check_offset + 2].copy_from_slice(&check.to_be_bytes());
        bytes
    }
    #[test]
    fn ipv4_ipv6_tcp_udp_and_odd_payloads() {
        for version in [4, 6] {
            for protocol in [6, 17] {
                let info = parse(&packet(version, protocol), 1500).unwrap();
                assert_eq!(
                    (
                        info.version,
                        info.protocol,
                        info.source_port,
                        info.destination_port,
                        info.payload_bytes
                    ),
                    (version, protocol, 12345, 53, 3)
                );
            }
        }
    }
    #[test]
    fn every_truncation_and_checksum_corruption_is_rejected() {
        for version in [4, 6] {
            for protocol in [6, 17] {
                let bytes = packet(version, protocol);
                for length in 0..bytes.len() {
                    assert!(parse(&bytes[..length], 1500).is_err());
                }
                let mut corrupt = bytes.clone();
                *corrupt.last_mut().unwrap() ^= 1;
                assert_eq!(parse(&corrupt, 1500), Err(Error::InvalidInput));
                let mut trailing = bytes;
                trailing.push(0);
                assert!(parse(&trailing, 1500).is_err());
            }
        }
    }
    #[test]
    fn fragments_options_extensions_and_unknown_protocols_are_rejected() {
        let mut fragment = packet(4, 17);
        fragment[6] = 0x20;
        fragment[10..12].fill(0);
        let check = checksum(&fragment[..20], 0);
        fragment[10..12].copy_from_slice(&check.to_be_bytes());
        assert_eq!(parse(&fragment, 1500), Err(Error::Unsupported));
        for next in [0, 43, 44, 50, 51, 58, 59, 60, 255] {
            let mut bytes = packet(6, 17);
            bytes[6] = next;
            assert_eq!(parse(&bytes, 1500), Err(Error::Unsupported));
        }
    }
    #[test]
    fn udp_checksum_omission_only_allowed_for_ipv4() {
        for version in [4, 6] {
            let mut bytes = packet(version, 17);
            let ip = if version == 4 { 20 } else { 40 };
            bytes[ip + 6..ip + 8].fill(0);
            assert_eq!(parse(&bytes, 1500).is_ok(), version == 4);
        }
    }
    #[test]
    fn session_bounds_close_and_rejection_do_not_change_counters() {
        assert!(PacketSession::new(1279).is_err());
        assert!(PacketSession::new(9001).is_err());
        let mut session = PacketSession::new(1500).unwrap();
        let bytes = packet(4, 17);
        assert_eq!(session.forward(&bytes), Err(Error::Unsupported));
        assert!(session.inspect(&vec![0; 1501]).is_err());
        assert_eq!(session.counters(), (0, 0));
        session.inspect(&bytes).unwrap();
        assert_eq!(session.counters(), (1, bytes.len() as u64));
        session.close();
        session.close();
        assert_eq!(session.inspect(&bytes), Err(Error::InvalidState));
        assert_eq!(session.forward(&bytes), Err(Error::InvalidState));
    }
    #[test]
    fn invalid_transport_lengths_and_ip_headers_are_rejected() {
        for version in [4, 6] {
            let ip = if version == 4 { 20 } else { 40 };
            let mut udp = packet(version, 17);
            udp[ip + 4..ip + 6].copy_from_slice(&8_u16.to_be_bytes());
            assert_eq!(parse(&udp, 1500), Err(Error::InvalidInput));
            for offset in [0x40, 0xf0] {
                let mut tcp = packet(version, 6);
                tcp[ip + 12] = offset;
                assert_eq!(parse(&tcp, 1500), Err(Error::InvalidInput));
            }
            let mut expired = packet(version, 17);
            expired[if version == 4 { 8 } else { 7 }] = 0;
            assert_eq!(parse(&expired, 1500), Err(Error::InvalidInput));
        }
        let mut options = packet(4, 17);
        options.splice(20..20, [1, 1, 1, 0]);
        options[0] = 0x46;
        let len = options.len() as u16;
        options[2..4].copy_from_slice(&len.to_be_bytes());
        options[10..12].fill(0);
        let check = checksum(&options[..24], 0);
        options[10..12].copy_from_slice(&check.to_be_bytes());
        assert_eq!(parse(&options, 1500), Err(Error::Unsupported));
    }

    #[test]
    fn counter_overflow_is_atomic() {
        let mut session = PacketSession::new(1500).unwrap();
        session.packets = u64::MAX;
        assert_eq!(session.inspect(&packet(4, 17)), Err(Error::ResourceLimit));
        assert_eq!(session.counters(), (u64::MAX, 0));
        session.packets = 0;
        session.bytes = u64::MAX;
        assert_eq!(session.inspect(&packet(4, 17)), Err(Error::ResourceLimit));
        assert_eq!(session.counters(), (0, u64::MAX));
    }

    #[test]
    fn arbitrary_bounded_bytes_never_panic() {
        let mut seed = 1_u64;
        for length in 0..=MAX_MTU {
            let bytes: Vec<_> = (0..length)
                .map(|_| {
                    seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                    (seed >> 32) as u8
                })
                .collect();
            let _ = parse(&bytes, MAX_MTU);
        }
    }
}
