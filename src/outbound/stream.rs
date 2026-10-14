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
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
type Cancellation = Pin<Box<dyn Future<Output = ()> + Send>>;
fn cancellation(connection: &Connection) -> Cancellation {
    let mut receiver = connection.cancel.subscribe();
    Box::pin(async move {
        let _ = receiver.wait_for(|closed| *closed).await;
    })
}
pub struct ManagedStream {
    stream: super::StreamSlot,
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
        let _operation = self.connection.operation().map_err(io::Error::other)?;
        self.cancelled(cx, true)?;
        let before = buffer.filled().len();
        let slot = self.stream.clone();
        let mut stream = slot.lock().map_err(|_| io::Error::other(Error::Internal))?;
        let stream = stream
            .as_mut()
            .ok_or_else(|| io::Error::new(io::ErrorKind::ConnectionAborted, Error::Cancelled))?;
        let result = Pin::new(stream).poll_read(cx, buffer);
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
        let _operation = self.connection.operation().map_err(io::Error::other)?;
        self.cancelled(cx, false)?;
        let slot = self.stream.clone();
        let mut stream = slot.lock().map_err(|_| io::Error::other(Error::Internal))?;
        let stream = stream
            .as_mut()
            .ok_or_else(|| io::Error::new(io::ErrorKind::ConnectionAborted, Error::Cancelled))?;
        match Pin::new(stream).poll_write(cx, bytes) {
            Poll::Ready(Ok(count)) => Poll::Ready(self.account(count, true).map(|()| count)),
            Poll::Ready(Err(error)) => {
                self.connection.close();
                Poll::Ready(Err(error))
            }
            Poll::Pending => Poll::Pending,
        }
    }
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let _operation = self.connection.operation().map_err(io::Error::other)?;
        self.cancelled(cx, false)?;
        let slot = self.stream.clone();
        let mut stream = slot.lock().map_err(|_| io::Error::other(Error::Internal))?;
        let stream = stream
            .as_mut()
            .ok_or_else(|| io::Error::new(io::ErrorKind::ConnectionAborted, Error::Cancelled))?;
        Pin::new(stream).poll_flush(cx)
    }
    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let _operation = self.connection.operation().map_err(io::Error::other)?;
        self.cancelled(cx, false)?;
        let slot = self.stream.clone();
        let mut stream = slot.lock().map_err(|_| io::Error::other(Error::Internal))?;
        let stream = stream
            .as_mut()
            .ok_or_else(|| io::Error::new(io::ErrorKind::ConnectionAborted, Error::Cancelled))?;
        Pin::new(stream).poll_shutdown(cx)
    }
}
impl Drop for ManagedStream {
    fn drop(&mut self) {
        let resource = self.owner.upgrade().and_then(|state| match state.lock() {
            Ok(mut state) => state.registry.take(self.id),
            Err(_) => {
                crate::error::diagnostics::record(
                    Error::Internal,
                    crate::error::diagnostics::DiagnosticSite::Cleanup,
                );
                None
            }
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
            // Arc::get_mut 只用于独占修改 Rust 数据，不判断业务占用权。
            // 身份和租约另由 Authority 裁决；真实流仍登记于内核资源槽。
            let mut guard = connection.resources.lock().map_err(|_| Error::Internal)?;
            let resources = guard.as_mut().ok_or(Error::InvalidState)?;
            let resources = Arc::get_mut(resources).ok_or(Error::InvalidState)?;
            let socket = std::mem::replace(&mut resources.socket, Socket::Detached);
            let Socket::Tcp { reader, writer } = socket else {
                return Err(Error::Unsupported);
            };
            let stream = Arc::new(Mutex::new(Some(
                reader.into_inner().unsplit(writer.into_inner()),
            )));
            resources.detached = Some(stream.clone());
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
            let _ = crate::error::diagnostics::observe(
                self.close(id).await,
                crate::error::diagnostics::DiagnosticSite::Cleanup,
            );
        }
        result
    }
}

#[cfg(test)]
mod lifecycle_tests {
    use super::*;
    /// 外部持有流对象与旧快照，关闭仍应释放真实 TCP 并禁止恢复读写。
    #[tokio::test]
    async fn retained_stream_does_not_own_socket_after_session_close() {
        use crate::{
            dns::{Config, Resolver, Server, fake_ip},
            routing::{Config as Routes, Mode, Router},
            tun::udp::{Options, Session},
        };
        use tokio::{
            io::{AsyncReadExt, AsyncWriteExt},
            net::TcpListener,
            time::{Duration, timeout},
        };
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let manager = Arc::new(Manager::default());
        let resolver = Arc::new(
            Resolver::new(Config {
                servers: vec![Server::Udp {
                    address: "127.0.0.1:5353".into(),
                }],
                timeout_ms: 100,
                dnssec: false,
                service: None,
                fake_ip: Some(fake_ip::Config {
                    ipv4_range: "198.19.96.0/24".into(),
                    ipv6_range: "fd00:cafe:198::/64".into(),
                    capacity: 2,
                    exclude: vec![],
                }),
            })
            .unwrap(),
        );
        manager
            .configure_network(
                Some(resolver.clone()),
                Some(Arc::new(
                    Router::compile(Routes {
                        mode: Mode::Direct,
                        ..Routes::default()
                    })
                    .unwrap(),
                )),
                crate::outbound::Limits::default(),
            )
            .unwrap();
        let options = Options {
            dns_endpoints: vec!["192.0.2.53:53".parse().unwrap()],
            ..Options::default()
        };
        let session = Session::new(manager.clone(), options.clone()).unwrap();
        let snapshot = manager.network_snapshot().unwrap();
        let mut stream = manager
            .connect_stream("127.0.0.1", listener.local_addr().unwrap().port(), None)
            .await
            .unwrap();
        let (mut remote, _) = listener.accept().await.unwrap();
        stream.write_all(b"x").await.unwrap();
        let mut bytes = [0];
        assert_eq!(remote.read(&mut bytes).await.unwrap(), 1);
        timeout(Duration::from_secs(2), session.close())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            timeout(Duration::from_secs(2), remote.read(&mut bytes))
                .await
                .unwrap()
                .unwrap(),
            0
        );
        assert!(stream.write_all(b"revoked").await.is_err());
        assert!(snapshot.resolver.as_ref().unwrap().clear().is_err());
        let next = Session::new(manager.clone(), options).unwrap();
        next.close().await.unwrap();
        assert!(session.receive().is_err());
    }
}
