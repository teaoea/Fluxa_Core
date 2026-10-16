//! 自研 TCP 报文边界；先复用 IP/校验和验证，所有切片偏移由报文长度约束。
use crate::{
    error::{Error, Result},
    tun::{self, Packet},
};
use std::net::SocketAddr;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Flags(pub u8);
impl Flags {
    pub const FIN: u8 = 0x01;
    pub const SYN: u8 = 0x02;
    pub const RST: u8 = 0x04;
    pub const PSH: u8 = 0x08;
    pub const ACK: u8 = 0x10;
    pub const URG: u8 = 0x20;
    pub fn has(self, bit: u8) -> bool {
        self.0 & bit != 0
    }
}
/// TCP 选项最多容纳四个 SACK 区间，长度只由受限 push 修改。
#[derive(Clone, Copy, Default, Debug, Eq, PartialEq)]
pub struct Sacks {
    blocks: [(u32, u32); 4],
    count: u8,
}
impl Sacks {
    pub fn blocks(&self) -> &[(u32, u32)] {
        &self.blocks[..usize::from(self.count)]
    }
    pub fn push(&mut self, left: u32, right: u32) -> Result<()> {
        if self.count == 4 || right.wrapping_sub(left) == 1 << 31 || !before(left, right) {
            return Err(Error::InvalidInput);
        }
        self.blocks[usize::from(self.count)] = (left, right);
        self.count += 1;
        Ok(())
    }
}
pub struct Segment<'a> {
    pub source: SocketAddr,
    pub destination: SocketAddr,
    pub sequence: u32,
    pub acknowledgement: u32,
    pub window: usize,
    pub flags: Flags,
    pub maximum_segment: Option<u16>,
    pub sack_permitted: bool,
    pub sacks: Sacks,
    pub payload: &'a [u8],
}
pub fn decode(bytes: &[u8], mtu: usize) -> Result<Segment<'_>> {
    if !(tun::MIN_MTU..=tun::MAX_MTU).contains(&mtu) {
        return Err(Error::InvalidInput);
    }
    let info = tun::parse(bytes, tun::MAX_MTU)?;
    if info.protocol != 6 {
        return Err(Error::Unsupported);
    }
    let (source, destination) = info.endpoints()?;
    let ip = if info.version == 4 { 20 } else { 40 };
    let header = usize::from(bytes[ip + 12] >> 4) * 4;
    // 不支持 TCP 保留位、紧急数据与 TCP Fast Open 数据型 SYN；不能静默截断正文。
    if bytes[ip + 12] & 15 != 0 {
        return Err(Error::Unsupported);
    }
    let flags = Flags(bytes[ip + 13]);
    if flags.has(Flags::URG) {
        return Err(Error::Unsupported);
    }
    let mut maximum_segment = None;
    let mut sack_permitted = false;
    let mut sacks = Sacks::default();
    let mut offset = ip + 20;
    while offset < ip + header {
        let kind = bytes[offset];
        if kind == 0 {
            break;
        }
        if kind == 1 {
            offset += 1;
            continue;
        }
        let length = usize::from(*bytes.get(offset + 1).ok_or(Error::InvalidInput)?);
        if length < 2 || offset + length > ip + header {
            return Err(Error::InvalidInput);
        }
        if kind == 2 {
            if length != 4 || !flags.has(Flags::SYN) || maximum_segment.is_some() {
                return Err(Error::InvalidInput);
            }
            let size = u16::from_be_bytes([bytes[offset + 2], bytes[offset + 3]]);
            if size < 64 {
                return Err(Error::Unsupported);
            }
            maximum_segment = Some(size);
        }
        if kind == 4 {
            if length != 2 || !flags.has(Flags::SYN) || sack_permitted {
                return Err(Error::InvalidInput);
            }
            sack_permitted = true;
        }
        if kind == 5 {
            if flags.has(Flags::SYN)
                || !flags.has(Flags::ACK)
                || !(10..=34).contains(&length)
                || (length - 2) % 8 != 0
                || !sacks.blocks().is_empty()
            {
                return Err(Error::InvalidInput);
            }
            for block in bytes[offset + 2..offset + length].as_chunks::<8>().0 {
                sacks.push(
                    u32::from_be_bytes(block[..4].try_into().map_err(|_| Error::InvalidInput)?),
                    u32::from_be_bytes(block[4..].try_into().map_err(|_| Error::InvalidInput)?),
                )?;
            }
        }
        // 窗口扩大、时间戳和 ECN 尚未协商；未知选项不改变状态机。
        offset += length;
    }
    Ok(Segment {
        source,
        destination,
        sequence: u32::from_be_bytes(
            bytes[ip + 4..ip + 8]
                .try_into()
                .map_err(|_| Error::InvalidInput)?,
        ),
        acknowledgement: u32::from_be_bytes(
            bytes[ip + 8..ip + 12]
                .try_into()
                .map_err(|_| Error::InvalidInput)?,
        ),
        window: usize::from(u16::from_be_bytes([bytes[ip + 14], bytes[ip + 15]])),
        flags,
        maximum_segment,
        sack_permitted,
        sacks,
        payload: &bytes[ip + header..],
    })
}
pub fn encode(segment: &Segment<'_>, mtu: usize) -> Result<Packet> {
    let Segment {
        source,
        destination,
        sequence,
        acknowledgement,
        flags,
        window,
        maximum_segment,
        sack_permitted,
        sacks,
        payload,
    } = *segment;
    if window > u16::MAX as usize
        || maximum_segment.is_some_and(|size| size < 64 || !flags.has(Flags::SYN))
        || (sack_permitted && !flags.has(Flags::SYN))
        || (!sacks.blocks().is_empty() && (flags.has(Flags::SYN) || !flags.has(Flags::ACK)))
    {
        return Err(Error::InvalidInput);
    }
    use std::net::IpAddr;
    let ip = match (source.ip(), destination.ip()) {
        (IpAddr::V4(_), IpAddr::V4(_)) => 20,
        (IpAddr::V6(_), IpAddr::V6(_)) => 40,
        _ => return Err(Error::InvalidInput),
    };
    let mut options = [0u8; 40];
    let mut used = 0;
    if let Some(mss) = maximum_segment {
        options[..4].copy_from_slice(&[2, 4, (mss >> 8) as u8, mss as u8]);
        used = 4;
    }
    if sack_permitted {
        options[used..used + 2].copy_from_slice(&[4, 2]);
        used += 2;
    }
    if !sacks.blocks().is_empty() {
        options[used] = 5;
        options[used + 1] = (2 + sacks.blocks().len() * 8) as u8;
        used += 2;
        for (left, right) in sacks.blocks() {
            options[used..used + 4].copy_from_slice(&left.to_be_bytes());
            options[used + 4..used + 8].copy_from_slice(&right.to_be_bytes());
            used += 8;
        }
    }
    let header = 20 + used.div_ceil(4) * 4;
    let size = ip + header + payload.len();
    if !(tun::MIN_MTU..=tun::MAX_MTU).contains(&mtu)
        || source.port() == 0
        || destination.port() == 0
    {
        return Err(Error::InvalidInput);
    }
    if size > mtu {
        return Err(Error::ResourceLimit);
    }
    Packet::build(size, |bytes| {
        match (source.ip(), destination.ip()) {
            (IpAddr::V4(src), IpAddr::V4(dst)) => {
                bytes[0] = 0x45;
                bytes[2..4].copy_from_slice(&(size as u16).to_be_bytes());
                bytes[6] = 0x40;
                bytes[8] = 64;
                bytes[9] = 6;
                bytes[12..16].copy_from_slice(&src.octets());
                bytes[16..20].copy_from_slice(&dst.octets());
                let check = tun::checksum(&bytes[..20], 0);
                bytes[10..12].copy_from_slice(&check.to_be_bytes());
            }
            (IpAddr::V6(src), IpAddr::V6(dst)) => {
                bytes[0] = 0x60;
                bytes[4..6].copy_from_slice(&((size - 40) as u16).to_be_bytes());
                bytes[6] = 6;
                bytes[7] = 64;
                bytes[8..24].copy_from_slice(&src.octets());
                bytes[24..40].copy_from_slice(&dst.octets());
            }
            _ => return,
        }
        bytes[ip..ip + 2].copy_from_slice(&source.port().to_be_bytes());
        bytes[ip + 2..ip + 4].copy_from_slice(&destination.port().to_be_bytes());
        bytes[ip + 4..ip + 8].copy_from_slice(&sequence.to_be_bytes());
        bytes[ip + 8..ip + 12].copy_from_slice(&acknowledgement.to_be_bytes());
        bytes[ip + 12] = (header as u8 / 4) << 4;
        bytes[ip + 13] = flags.0;
        bytes[ip + 14..ip + 16].copy_from_slice(&(window as u16).to_be_bytes());
        bytes[ip + 20..ip + header].copy_from_slice(&options[..header - 20]);
        bytes[ip + header..].copy_from_slice(payload);
        let check = tun::checksum(
            &bytes[ip..],
            tun::pseudo_sum(bytes, if ip == 20 { 4 } else { 6 }, 6, size - ip),
        );
        bytes[ip + 16..ip + 18].copy_from_slice(&check.to_be_bytes());
    })
}
/// 序号差仅用于小于 2^31 的有界窗口，按 TCP 的模 2^32 语义比较。
pub fn before(left: u32, right: u32) -> bool {
    (left.wrapping_sub(right) as i32) < 0
}
pub fn distance(left: u32, right: u32) -> usize {
    left.wrapping_sub(right) as usize
}

#[cfg(test)]
#[path = "../../../tests/test_tun_tcp_wire.rs"]
mod test_tun_tcp_wire;
