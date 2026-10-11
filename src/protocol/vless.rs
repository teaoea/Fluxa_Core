//! VLESS v0 TCP 请求编码；不接受 UDP、Mux 或未知附加字段。
use crate::{
    error::{Error, Result},
    network::normalize_host,
};
use std::net::IpAddr;
pub fn request(uuid: &[u8; 16], host: &str, port: u16) -> Result<Vec<u8>> {
    if port == 0 {
        return Err(Error::InvalidInput);
    }
    let host = normalize_host(host)?;
    let flow = b"xtls-rprx-vision";
    let mut out = Vec::with_capacity(64 + host.len());
    out.push(0);
    out.extend(uuid);
    out.push((flow.len() + 2) as u8);
    // protobuf 字段 1：长度限定的 flow 字符串；不携带未定义配置。
    out.extend([0x0a, flow.len() as u8]);
    out.extend(flow);
    out.push(1);
    out.extend(port.to_be_bytes());
    match host.parse::<IpAddr>() {
        Ok(IpAddr::V4(ip)) => {
            out.push(1);
            out.extend(ip.octets());
        }
        Ok(IpAddr::V6(ip)) => {
            out.push(3);
            out.extend(ip.octets());
        }
        Err(_) => {
            out.extend([2, host.len() as u8]);
            out.extend(host.as_bytes());
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn address_encoding_and_flow() {
        let uuid = [7; 16];
        let v4 = request(&uuid, "192.0.2.4", 443).unwrap();
        assert_eq!(&v4[..18], &[&[0][..], &uuid[..], &[18][..]].concat());
        assert_eq!(&v4[18..36], b"\x0a\x10xtls-rprx-vision");
        assert_eq!(&v4[36..], &[1, 1, 187, 1, 192, 0, 2, 4]);
        let v6 = request(&uuid, "2001:db8::1", 80).unwrap();
        assert_eq!(v6[39], 3);
        assert_eq!(v6.len(), 56);
        let domain = request(&uuid, "ExAmPlE.com.", 80).unwrap();
        assert_eq!(&domain[39..], b"\x02\x0bexample.com");
        assert!(request(&uuid, "example.com", 0).is_err());
        assert!(request(&uuid, "bad host", 80).is_err());
    }
}
