//! 单目标、单会话 XUDP；复用 REALITY/Vision，不提供任意 TCP Mux 或全锥复用。
use super::{Stream, config::Node, stream, vless};
use crate::{
    budget::{ByteBudget, ByteLease},
    error::{Error, Result},
};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt, ReadHalf, WriteHalf},
    net::TcpStream,
    sync::Mutex,
};
use zeroize::{Zeroize, Zeroizing};
/// 与固定独立服务端 XUDP 发送边界兼容；不将超限数据默默丢弃。
pub const MAX_PAYLOAD: usize = 8192 - 666;
const MAX_FRAME: usize = 512 + 8192 + 4;
static MEMORY: ByteBudget = ByteBudget::new(4 * 1024 * 1024);
struct Reader {
    stream: ReadHalf<Stream>,
    pending: Zeroizing<Vec<u8>>,
}
struct Writer {
    stream: WriteHalf<Stream>,
    first: bool,
}
pub struct Datagram {
    target: SocketAddr,
    failed: std::sync::atomic::AtomicBool,
    reader: Mutex<Reader>,
    writer: Mutex<Writer>,
    _lease: ByteLease,
}
pub async fn connect_udp(socket: TcpStream, node: &Node, target: SocketAddr) -> Result<Datagram> {
    if target.port() == 0 || target.ip().is_unspecified() || target.ip().is_multicast() {
        return Err(Error::InvalidInput);
    }
    let lease = MEMORY.reserve(32768)?;
    let stream = stream::connect_mux(socket, node).await?;
    let (reader, writer) = tokio::io::split(stream);
    Ok(Datagram {
        target,
        failed: std::sync::atomic::AtomicBool::new(false),
        reader: Mutex::new(Reader {
            stream: reader,
            pending: Zeroizing::new(Vec::new()),
        }),
        writer: Mutex::new(Writer {
            stream: writer,
            first: true,
        }),
        _lease: lease,
    })
}
impl Datagram {
    pub(crate) fn failed(&self) -> bool {
        self.failed.load(std::sync::atomic::Ordering::Acquire)
    }
    fn fail(&self, error: Error) -> Error {
        self.failed
            .store(true, std::sync::atomic::Ordering::Release);
        error
    }
    pub async fn send(&self, body: &[u8]) -> Result<usize> {
        if self.failed() {
            return Err(Error::InvalidState);
        }
        if body.is_empty() {
            return Err(Error::Unsupported);
        }
        if body.len() > MAX_PAYLOAD {
            return Err(Error::ResourceLimit);
        }
        let mut writer = self.writer.lock().await;
        let frame = encode(self.target, writer.first, body)?;
        writer
            .stream
            .write_all(&frame)
            .await
            .map_err(|error| self.fail(error.into()))?;
        writer
            .stream
            .flush()
            .await
            .map_err(|error| self.fail(error.into()))?;
        writer.first = false;
        Ok(body.len())
    }
    /// 分帧缓存保存在 Rust 所有者中，读取超时取消不会丢失已读首部而错位。
    pub async fn receive(&self, output: &mut [u8]) -> Result<usize> {
        let mut reader = self.reader.lock().await;
        let mut skipped = 0;
        loop {
            if let Some(frame) =
                decode(&reader.pending, self.target).map_err(|error| self.fail(error))?
            {
                let end = frame.consumed;
                if let Some((start, length)) = frame.payload {
                    if length > output.len() {
                        reader.pending[..end].zeroize();
                        reader.pending.drain(..end).for_each(drop);
                        return Err(Error::ResourceLimit);
                    }
                    output[..length].copy_from_slice(&reader.pending[start..start + length]);
                    reader.pending[..end].zeroize();
                    reader.pending.drain(..end).for_each(drop);
                    return Ok(length);
                }
                reader.pending[..end].zeroize();
                reader.pending.drain(..end).for_each(drop);
                skipped += 1;
                if skipped > 32 {
                    return Err(self.fail(Error::ResourceLimit));
                }
                continue;
            }
            let capacity = MAX_FRAME
                .checked_sub(reader.pending.len())
                .filter(|n| *n > 0)
                .ok_or(Error::ResourceLimit)?;
            let mut chunk = Zeroizing::new(vec![0; capacity.min(1024)]);
            let count = reader
                .stream
                .read(&mut chunk)
                .await
                .map_err(|error| self.fail(error.into()))?;
            if count == 0 {
                return Err(self.fail(Error::Network));
            }
            reader.pending.extend(&chunk[..count]);
        }
    }
}
fn encode(target: SocketAddr, first: bool, body: &[u8]) -> Result<Zeroizing<Vec<u8>>> {
    if body.is_empty() {
        return Err(Error::Unsupported);
    }
    if body.len() > MAX_PAYLOAD {
        return Err(Error::ResourceLimit);
    }
    let mut metadata = vec![0, 0, if first { 1 } else { 2 }, 1];
    if first {
        metadata.push(2);
        metadata.extend(vless::address(&target.ip().to_string(), target.port())?);
    }
    let mut frame = Zeroizing::new(Vec::with_capacity(metadata.len() + body.len() + 4));
    frame.extend((metadata.len() as u16).to_be_bytes());
    frame.extend(metadata);
    frame.extend((body.len() as u16).to_be_bytes());
    frame.extend(body);
    Ok(frame)
}
struct Frame {
    consumed: usize,
    payload: Option<(usize, usize)>,
}
fn decode(bytes: &[u8], target: SocketAddr) -> Result<Option<Frame>> {
    if bytes.len() < 2 {
        return Ok(None);
    }
    let size = usize::from(u16::from_be_bytes([bytes[0], bytes[1]]));
    if !(4..=512).contains(&size) {
        return Err(Error::InvalidInput);
    }
    let end = size + 2;
    if bytes.len() < end {
        return Ok(None);
    }
    let metadata = &bytes[2..end];
    if metadata[..2] != [0, 0] || metadata[3] & !1 != 0 {
        return Err(Error::ProtocolViolation);
    }
    match metadata[2] {
        2 => {}
        4 => {}
        3 => return Err(Error::Network),
        _ => return Err(Error::Unsupported),
    }
    if size > 4 && metadata[2] == 2 {
        if metadata[4] != 2 {
            return Err(Error::Unsupported);
        }
        let source = address(&metadata[5..])?;
        if source != target {
            return Err(Error::ProtocolViolation);
        }
    }
    if metadata[3] & 1 == 0 {
        return Ok(Some(Frame {
            consumed: end,
            payload: None,
        }));
    }
    if bytes.len() < end + 2 {
        return Ok(None);
    }
    let length = usize::from(u16::from_be_bytes([bytes[end], bytes[end + 1]]));
    if length > 8192 {
        return Err(Error::ResourceLimit);
    }
    if bytes.len() < end + 2 + length {
        return Ok(None);
    }
    Ok(Some(Frame {
        consumed: end + 2 + length,
        payload: (metadata[2] == 2 && length > 0).then_some((end + 2, length)),
    }))
}
fn address(bytes: &[u8]) -> Result<SocketAddr> {
    if bytes.len() < 3 {
        return Err(Error::InvalidInput);
    }
    let port = u16::from_be_bytes([bytes[0], bytes[1]]);
    let expected = match bytes[2] {
        1 => 7,
        3 => 19,
        _ => return Err(Error::Unsupported),
    };
    if bytes.len() != expected {
        return Err(Error::InvalidInput);
    }
    let address = match bytes[2] {
        1 => IpAddr::V4(Ipv4Addr::from(
            <[u8; 4]>::try_from(bytes.get(3..7).ok_or(Error::InvalidInput)?)
                .map_err(|_| Error::InvalidInput)?,
        )),
        3 => IpAddr::V6(Ipv6Addr::from(
            <[u8; 16]>::try_from(bytes.get(3..19).ok_or(Error::InvalidInput)?)
                .map_err(|_| Error::InvalidInput)?,
        )),
        _ => return Err(Error::Unsupported),
    };
    Ok(SocketAddr::new(address, port))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn independent_response_boundaries_and_peer_constraints() {
        let target = "192.0.2.4:53".parse().unwrap();
        let response = [
            0, 12, 0, 0, 2, 1, 2, 0, 53, 1, 192, 0, 2, 4, 0, 3, 0, 255, 1,
        ];
        for size in 0..response.len() {
            assert!(decode(&response[..size], target).unwrap().is_none());
        }
        let frame = decode(&response, target).unwrap().unwrap();
        assert_eq!(frame.payload, Some((16, 3)));
        assert_eq!(frame.consumed, response.len());
        assert!(decode(&response, "192.0.2.5:53".parse().unwrap()).is_err());
        assert!(encode(target, true, &[]).is_err());
        assert!(encode(target, false, &vec![0; MAX_PAYLOAD + 1]).is_err());
        assert_eq!(
            &*encode(target, true, &[1]).unwrap(),
            &[0, 12, 0, 0, 1, 1, 2, 0, 53, 1, 192, 0, 2, 4, 0, 1, 1]
        );
        assert_eq!(
            &*encode(target, false, &[2]).unwrap(),
            &[0, 4, 0, 0, 2, 1, 0, 1, 2]
        );
        for size in 0..=9000 {
            let bytes = vec![0xff; size];
            let _ = decode(&bytes, target);
        }
    }
}

#[cfg(test)]
mod error_semantics_tests {
    use super::*;
    #[test]
    fn invalid_session_and_peer_are_not_policy_rejects() {
        let target: SocketAddr = "192.0.2.1:443".parse().unwrap();
        let bad_session = [0, 4, 0, 1, 2, 0];
        assert!(matches!(
            decode(&bad_session, target),
            Err(Error::ProtocolViolation)
        ));
        let bad_flags = [0, 4, 0, 0, 2, 2];
        assert!(matches!(
            decode(&bad_flags, target),
            Err(Error::ProtocolViolation)
        ));
        // 会话 0、NEW 状态、DATA 标志，携带非绑定 IPv4 来源；不放宽目标检查。
        let wrong_peer = [0, 12, 0, 0, 2, 1, 2, 0x01, 0xbb, 1, 192, 0, 2, 2];
        assert!(matches!(
            decode(&wrong_peer, target),
            Err(Error::ProtocolViolation)
        ));
    }
}
