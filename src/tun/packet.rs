//! TCP/UDP 返回包共用的所有权及总预算；消费、错误和取消通过 RAII 归还。
use super::{MAX_MTU, MIN_MTU, checksum, pseudo_sum};
use crate::{
    budget::{ByteBudget, ByteLease},
    error::{Error, Result},
};
use std::net::{IpAddr, SocketAddr};
use zeroize::Zeroizing;
static PACKETS: ByteBudget = ByteBudget::new(4 * 1024 * 1024);
/// 输出拥有正文和预算；宿主消费/释放后归还，不借用输入包内存。
pub struct Packet {
    pub bytes: Zeroizing<Vec<u8>>,
    _lease: ByteLease,
}
impl Packet {
    pub(crate) fn copy(bytes: &[u8]) -> Result<Self> {
        Self::build(bytes.len(), |output| output.copy_from_slice(bytes))
    }

    /// 总预算先于分配；调用者只得到同步受限切片，不保留正文裸指针。
    pub(crate) fn build(size: usize, fill: impl FnOnce(&mut [u8])) -> Result<Self> {
        if size > MAX_MTU {
            return Err(Error::ResourceLimit);
        }
        let lease = PACKETS.reserve(size)?;
        let mut bytes = Zeroizing::new(vec![0; size]);
        fill(&mut bytes);
        Ok(Self {
            bytes,
            _lease: lease,
        })
    }
}

/// 构造无分片 UDP/IP 包；校验和与解析器共用工具，超 MTU 整包拒绝。
pub fn encode_udp(
    source: SocketAddr,
    destination: SocketAddr,
    payload: &[u8],
    mtu: usize,
) -> Result<Packet> {
    let ip = match (source.ip(), destination.ip()) {
        (IpAddr::V4(_), IpAddr::V4(_)) => 20,
        (IpAddr::V6(_), IpAddr::V6(_)) => 40,
        _ => return Err(Error::InvalidInput),
    };
    let size = payload
        .len()
        .checked_add(ip + 8)
        .ok_or(Error::ResourceLimit)?;
    if !(MIN_MTU..=MAX_MTU).contains(&mtu) || source.port() == 0 || destination.port() == 0 {
        return Err(Error::InvalidInput);
    }
    if size > mtu {
        return Err(Error::ResourceLimit);
    }
    let lease = PACKETS.reserve(size)?;
    let mut bytes = Zeroizing::new(vec![0; size]);
    match (source.ip(), destination.ip()) {
        (IpAddr::V4(src), IpAddr::V4(dst)) => {
            bytes[0] = 0x45;
            bytes[2..4].copy_from_slice(&(size as u16).to_be_bytes());
            bytes[6] = 0x40;
            bytes[8] = 64;
            bytes[9] = 17;
            bytes[12..16].copy_from_slice(&src.octets());
            bytes[16..20].copy_from_slice(&dst.octets());
            let sum = checksum(&bytes[..20], 0);
            bytes[10..12].copy_from_slice(&sum.to_be_bytes());
        }
        (IpAddr::V6(src), IpAddr::V6(dst)) => {
            bytes[0] = 0x60;
            bytes[4..6].copy_from_slice(&((size - 40) as u16).to_be_bytes());
            bytes[6] = 17;
            bytes[7] = 64;
            bytes[8..24].copy_from_slice(&src.octets());
            bytes[24..40].copy_from_slice(&dst.octets());
        }
        _ => return Err(Error::InvalidInput),
    }
    bytes[ip..ip + 2].copy_from_slice(&source.port().to_be_bytes());
    bytes[ip + 2..ip + 4].copy_from_slice(&destination.port().to_be_bytes());
    bytes[ip + 4..ip + 6].copy_from_slice(&((size - ip) as u16).to_be_bytes());
    bytes[ip + 8..].copy_from_slice(payload);
    let sum = checksum(
        &bytes[ip..],
        pseudo_sum(&bytes, if ip == 20 { 4 } else { 6 }, 17, size - ip),
    );
    bytes[ip + 6..ip + 8].copy_from_slice(&(if sum == 0 { u16::MAX } else { sum }).to_be_bytes());
    Ok(Packet {
        bytes,
        _lease: lease,
    })
}
