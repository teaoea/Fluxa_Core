//! 协议流所有者负责有界泵和取消；丢弃流立即中止泵并归还预算。
use super::{config::Node, inner_tls, reality, vision, vless};
use crate::{
    budget::{ByteBudget, ByteLease},
    error::{Error, Result},
};
use std::{
    io,
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, Poll},
};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, DuplexStream, ReadBuf},
    net::TcpStream,
    task::JoinHandle,
};
use zeroize::Zeroizing;
static MEMORY: ByteBudget = ByteBudget::new(16 * 1024 * 1024);
const RESERVATION: usize = 384 * 1024;
pub struct Stream {
    pipe: DuplexStream,
    task: JoinHandle<()>,
    failure: Arc<Mutex<Option<Error>>>,
    _lease: Arc<ByteLease>,
    direct: Arc<std::sync::atomic::AtomicU8>,
}
impl Stream {
    /// 仅返回切换状态，不包含报文、密钥或节点凭据。
    pub fn vision_direct(&self) -> (bool, bool) {
        let flags = self.direct.load(std::sync::atomic::Ordering::Acquire);
        (flags & 1 != 0, flags & 2 != 0)
    }
    fn check(&self) -> io::Result<()> {
        let failure = self
            .failure
            .lock()
            .map_err(|_| io::Error::other(Error::Internal))?;
        if let Some(error) = *failure {
            return Err(io::Error::other(error));
        }
        Ok(())
    }
}
impl AsyncRead for Stream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        self.check()?;
        let result = Pin::new(&mut self.pipe).poll_read(cx, buf);
        if result.is_ready() {
            self.check()?;
        }
        result
    }
}
impl AsyncWrite for Stream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        data: &[u8],
    ) -> Poll<io::Result<usize>> {
        self.check()?;
        Pin::new(&mut self.pipe).poll_write(cx, data)
    }
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.check()?;
        Pin::new(&mut self.pipe).poll_flush(cx)
    }
    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.check()?;
        Pin::new(&mut self.pipe).poll_shutdown(cx)
    }
}
impl Drop for Stream {
    fn drop(&mut self) {
        self.task.abort();
    }
}
fn failed(failure: &Arc<Mutex<Option<Error>>>, result: Result<()>) {
    if let Err(error) = result
        && let Ok(mut f) = failure.lock()
        && f.is_none()
    {
        *f = Some(error);
    }
}
/// 调用者承担 DNS、拨号期限和连接生命周期；协议不会自行回退其他出站。
pub async fn connect(socket: TcpStream, node: &Node, host: &str, port: u16) -> Result<Stream> {
    connect_request(socket, node, vless::request(&node.uuid, host, port)?).await
}
pub(super) async fn connect_mux(socket: TcpStream, node: &Node) -> Result<Stream> {
    connect_request(socket, node, vless::mux_request(&node.uuid)).await
}
async fn connect_request(mut socket: TcpStream, node: &Node, request: Vec<u8>) -> Result<Stream> {
    let lease = Arc::new(MEMORY.reserve(RESERVATION)?);
    let (mut rx, mut tx) = Box::pin(reality::handshake(&mut socket, node)).await?;
    let mut initial = Zeroizing::new(request);
    initial.extend(vision::frame(Some(&node.uuid), &[], 0)?);
    socket.write_all(&tx.seal(23, &initial)?).await?;
    let (pipe, pump) = tokio::io::duplex(16384);
    let (mut input, mut output) = tokio::io::split(pump);
    let (mut reader, mut writer) = socket.into_split();
    let failure = Arc::new(Mutex::new(None));
    let read_failure = Arc::clone(&failure);
    let write_failure = Arc::clone(&failure);
    let uuid = Zeroizing::new(node.uuid);
    let direct = Arc::new(std::sync::atomic::AtomicU8::new(0));
    let read_direct = Arc::clone(&direct);
    let write_direct = Arc::clone(&direct);
    let inner = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let read_inner = Arc::clone(&inner);
    let read_pump = async move {
        let result: Result<()> = async {
            let mut response = Zeroizing::new(Vec::new());
            let mut header = false;
            let mut decoder = vision::Decoder::new();
            let mut server = inner_tls::ServerHello::default();
            loop {
                if decoder.direct {
                    let mut bytes = Zeroizing::new(vec![0; 16384]);
                    let n = reader.read(&mut bytes).await?;
                    if n == 0 {
                        break;
                    }
                    output.write_all(&bytes[..n]).await?;
                    continue;
                }
                let Some(record) = reality::record(&mut reader).await? else {
                    if !header {
                        return Err(Error::InvalidState);
                    }
                    decoder.finish()?;
                    break;
                };
                let (kind, plain) = rx.open(&record)?;
                match kind {
                    21 => {
                        if plain.as_slice() == [1, 0] {
                            if !header {
                                return Err(Error::InvalidState);
                            }
                            decoder.finish()?;
                            break;
                        }
                        return Err(Error::Tls);
                    }
                    // 会话票据不启用；只接受无需回发的标准 KeyUpdate，其他控制明确拒绝。
                    22 => {
                        if plain.first() == Some(&4) {
                            if plain.len() < 4 || plain.len() > 65536 {
                                return Err(Error::Tls);
                            }
                            continue;
                        }
                        if plain.as_slice() == [24, 0, 0, 1, 0] {
                            rx.update()?;
                            continue;
                        }
                        return Err(Error::Unsupported);
                    }
                    23 => {}
                    _ => return Err(Error::Tls),
                }
                let payload = if !header {
                    if response.len() + plain.len() > 16640 {
                        return Err(Error::ResourceLimit);
                    }
                    response.extend(plain.iter());
                    if response.len() < 2 {
                        continue;
                    }
                    if response[0] != 0 {
                        return Err(Error::Unsupported);
                    }
                    let skip = 2 + usize::from(response[1]);
                    if response.len() < skip {
                        continue;
                    }
                    header = true;
                    let payload = response[skip..].to_vec();
                    response.clear();
                    payload
                } else {
                    plain.to_vec()
                };
                let data = decoder.feed(&payload, &uuid)?;
                if decoder.direct {
                    read_direct.fetch_or(1, std::sync::atomic::Ordering::Release);
                }
                if server.feed(&data)? {
                    read_inner.store(true, std::sync::atomic::Ordering::Release);
                }
                output.write_all(&data).await?;
            }
            output.shutdown().await?;
            Ok(())
        }
        .await;
        failed(&read_failure, result);
        result
    };
    let write_pump = async move {
        let mut bytes = Zeroizing::new(vec![0; 1792]);
        let mut remaining = 7u8;
        let mut framed = true;
        let mut raw = false;
        let mut boundary = inner_tls::Boundary::default();
        loop {
            let n = input.read(&mut bytes).await?;
            if n == 0 {
                break;
            }
            if raw {
                writer.write_all(&bytes[..n]).await?;
                continue;
            }
            let boundary_end = boundary.application_end(&bytes[..n]);
            let switch = framed
                && inner.load(std::sync::atomic::Ordering::Acquire)
                && boundary_end.is_some();
            let count = if switch {
                boundary_end.ok_or(Error::Internal)?
            } else {
                n
            };
            let payload = if framed {
                remaining = remaining.saturating_sub(1);
                // 等待已观察到的 TLS 1.3 应用记录；普通数据结束填充后仍留在外层 TLS。
                let command = if switch {
                    2
                } else if remaining == 0 && !inner.load(std::sync::atomic::Ordering::Acquire) {
                    1
                } else {
                    0
                };
                let data = vision::frame(None, &bytes[..count], command)?;
                if command != 0 {
                    framed = false;
                }
                if command == 2 {
                    raw = true;
                    write_direct.fetch_or(2, std::sync::atomic::Ordering::Release);
                }
                data
            } else {
                bytes[..n].to_vec()
            };
            writer.write_all(&tx.seal(23, &payload)?).await?;
            if raw && count < n {
                writer.write_all(&bytes[count..n]).await?;
            }
        }
        if !raw {
            writer.write_all(&tx.seal(21, &[1, 0])?).await?;
        }
        writer.shutdown().await?;
        Ok(())
    };
    let task_lease = Arc::clone(&lease);
    let task = tokio::spawn(async move {
        let _lease = task_lease;
        let result = tokio::try_join!(read_pump, write_pump).map(|_| ());
        failed(&write_failure, result);
    });
    Ok(Stream {
        pipe,
        task,
        failure,
        _lease: lease,
        direct,
    })
}
