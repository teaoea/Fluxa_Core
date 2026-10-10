//! 可安全交给 TLS/HTTP 的受管理 TCP 流；取消和资源所有权沿用共用连接。
use super::{Connection, Family, Manager, RoutedTarget, Socket, State};
use crate::{
    error::{Error, Result},
    routing::{Target, Transport},
};
use std::{
    future::Future,
    io,
    net::SocketAddr,
    pin::Pin,
    sync::{Arc, Mutex, Weak},
    task::{Context, Poll},
};
use tokio::{
    io::{AsyncRead, AsyncWrite, ReadBuf},
    net::TcpStream,
};
type Cancellation = Pin<Box<dyn Future<Output = ()> + Send>>;
fn cancellation(connection: &Connection) -> Cancellation {
    let mut receiver = connection.cancel.subscribe();
    Box::pin(async move {
        let _ = receiver.wait_for(|closed| *closed).await;
    })
}
pub struct ManagedStream {
    stream: TcpStream,
    connection: Arc<Connection>,
    owner: Weak<Mutex<State>>,
    id: u64,
    read_cancel: Cancellation,
    write_cancel: Cancellation,
}
impl ManagedStream {
    fn cancelled(&mut self, cx: &mut Context<'_>, read: bool) -> io::Result<()> {
        if *self.connection.cancel.borrow() {
            return Err(io::Error::new(
                io::ErrorKind::ConnectionAborted,
                Error::Cancelled,
            ));
        }
        let future = if read {
            &mut self.read_cancel
        } else {
            &mut self.write_cancel
        };
        if future.as_mut().poll(cx).is_ready() {
            self.connection.close();
            return Err(io::Error::new(
                io::ErrorKind::ConnectionAborted,
                Error::Cancelled,
            ));
        }
        Ok(())
    }
    fn account(&self, count: usize, upload: bool) -> io::Result<()> {
        self.connection
            .activity(count, upload, false)
            .map_err(io::Error::other)
    }
}
impl AsyncRead for ManagedStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        self.cancelled(cx, true)?;
        let before = buffer.filled().len();
        let result = Pin::new(&mut self.stream).poll_read(cx, buffer);
        match result {
            Poll::Ready(Ok(())) => Poll::Ready(self.account(buffer.filled().len() - before, false)),
            Poll::Ready(Err(error)) => {
                self.connection.close();
                Poll::Ready(Err(error))
            }
            Poll::Pending => Poll::Pending,
        }
    }
}
impl AsyncWrite for ManagedStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bytes: &[u8],
    ) -> Poll<io::Result<usize>> {
        self.cancelled(cx, false)?;
        match Pin::new(&mut self.stream).poll_write(cx, bytes) {
            Poll::Ready(Ok(count)) => Poll::Ready(self.account(count, true).map(|()| count)),
            Poll::Ready(Err(error)) => {
                self.connection.close();
                Poll::Ready(Err(error))
            }
            Poll::Pending => Poll::Pending,
        }
    }
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.cancelled(cx, false)?;
        Pin::new(&mut self.stream).poll_flush(cx)
    }
    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.cancelled(cx, false)?;
        Pin::new(&mut self.stream).poll_shutdown(cx)
    }
}
impl Drop for ManagedStream {
    fn drop(&mut self) {
        let resource = self.owner.upgrade().and_then(|state| {
            state
                .lock()
                .ok()
                .and_then(|mut state| state.registry.take(self.id))
        });
        if let Some(resource) = resource {
            resource.close();
        }
        self.connection.close();
    }
}
impl Manager {
    /// 原目标执行配置 DNS 与路由；显式本机 HTTP 中转仍受目标检查及连接预算约束。
    pub(crate) async fn connect_stream(
        &self,
        host: &str,
        port: u16,
        proxy_port: Option<u16>,
    ) -> Result<ManagedStream> {
        let host = crate::network::normalize_host(host)?;
        let ip = host.parse::<std::net::IpAddr>().ok();
        let request = RoutedTarget {
            target: Target {
                domain: ip.is_none().then_some(host),
                address: ip.map(|ip| ip.to_string()),
                port,
                transport: Transport::Tcp,
            },
            timeout_ms: 60000,
            family: Family::Auto,
            mapping_generation: None,
        };
        let hop = proxy_port.map(|port| SocketAddr::from(([127, 0, 0, 1], port)));
        if hop.is_some_and(|hop| hop.port() == 0) {
            return Err(Error::InvalidInput);
        }
        let opened = self.connect_configured_hop(request, hop).await?;
        let id = opened.connection.id;
        let result = (|| {
            let mut state = self.state.lock().map_err(|_| Error::Internal)?;
            let connection = state.registry.get_mut(id).ok_or(Error::InvalidState)?;
            // 私有转换发生在返回流之前，外部尚未取得句柄；额外所有者出现时明确失败。
            let unique = Arc::get_mut(connection).ok_or(Error::InvalidState)?;
            let socket = std::mem::replace(&mut unique.socket, Socket::Detached);
            let Socket::Tcp { reader, writer } = socket else {
                return Err(Error::Unsupported);
            };
            let stream = reader
                .into_inner()
                .reunite(writer.into_inner())
                .map_err(|_| Error::Internal)?;
            Ok(ManagedStream {
                stream,
                owner: Arc::downgrade(&self.state),
                id,
                read_cancel: cancellation(connection),
                write_cancel: cancellation(connection),
                connection: Arc::clone(connection),
            })
        })();
        if result.is_err() {
            let _ = self.close(id);
        }
        result
    }
}
