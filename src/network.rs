//! 共用 CIDR 值类型：纯数字地址、对齐网段及 IPv4/IPv6 标签，不执行网络 I/O。
use crate::error::{Error, Result};
use std::net::IpAddr;
#[derive(Clone, Copy)]
pub(crate) struct Network {
    base: u128,
    mask: u128,
    v4: bool,
}
fn number(ip: IpAddr) -> (u128, bool) {
    match ip {
        IpAddr::V4(ip) => (u128::from(u32::from(ip)), true),
        IpAddr::V6(ip) => (u128::from(ip), false),
    }
}
impl Network {
    pub fn parse(value: &str) -> Result<Self> {
        if value.len() > 64 {
            return Err(Error::InvalidInput);
        }
        let (ip, prefix) = value.split_once('/').ok_or(Error::InvalidInput)?;
        let ip: IpAddr = ip.parse().map_err(|_| Error::InvalidInput)?;
        let prefix: u32 = prefix.parse().map_err(|_| Error::InvalidInput)?;
        let (base, v4) = number(ip);
        let width = if v4 { 32 } else { 128 };
        if prefix > width {
            return Err(Error::InvalidInput);
        }
        let full = if v4 { u128::from(u32::MAX) } else { u128::MAX };
        let mask = if prefix == 0 {
            0
        } else {
            (u128::MAX << (width - prefix)) & full
        };
        if base & mask != base {
            return Err(Error::InvalidInput);
        }
        Ok(Self { base, mask, v4 })
    }
    pub(crate) fn prefix(&self) -> u8 {
        self.mask.count_ones() as u8
    }
    pub(crate) fn key(&self) -> (u128, u8, bool) {
        (self.base, self.prefix(), self.v4)
    }
    pub(crate) fn canonical(&self) -> String {
        format!("{}/{}", self.first(), self.prefix())
    }
    pub fn first(&self) -> IpAddr {
        self.address(self.base)
    }
    pub fn last(&self) -> IpAddr {
        let full = if self.v4 {
            u128::from(u32::MAX)
        } else {
            u128::MAX
        };
        self.address(self.base | (full ^ self.mask))
    }
    fn address(&self, value: u128) -> IpAddr {
        if self.v4 {
            IpAddr::V4((value as u32).into())
        } else {
            IpAddr::V6(value.into())
        }
    }
    pub fn includes(&self, ip: IpAddr) -> bool {
        let (ip, v4) = number(ip);
        self.v4 == v4 && ip & self.mask == self.base
    }
}

/// HTTP 和路由复用域名规范化，数字地址仍由调用者决定是否接受。
pub fn normalize_host(host: &str) -> Result<String> {
    let host = host.strip_suffix('.').unwrap_or(host).to_ascii_lowercase();
    let host = host
        .strip_prefix('[')
        .and_then(|h| h.strip_suffix(']'))
        .unwrap_or(&host)
        .to_owned();
    if host.parse::<IpAddr>().is_ok() {
        return Ok(host);
    }
    if host.is_empty()
        || host.len() > 253
        || host.split('.').any(|label| {
            label.is_empty()
                || label.len() > 63
                || label.starts_with('-')
                || label.ends_with('-')
                || !label
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        })
    {
        return Err(Error::InvalidInput);
    }
    Ok(host)
}

/// HTTP、TLS 和代理字节流复用相同的安全双向接口。
pub(crate) trait Duplex:
    tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send
{
}
impl<T: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send> Duplex for T {}
pub(crate) type SecureStream = Box<dyn Duplex>;
