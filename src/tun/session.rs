//! 同一宿主会话分派 TCP/UDP；原 UDP 接口保持兼容，所有协议处理仍在 Rust。
use super::{Packet, tcp, udp};
use crate::{
    error::{Error, Result},
    outbound::Manager,
};
use serde::Deserialize;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
#[derive(Clone, Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct Options {
    pub udp: udp::Options,
    pub tcp: tcp::Options,
}
pub struct Session {
    udp: Arc<udp::Session>,
    tcp: tcp::Session,
    next_tcp: AtomicBool,
}
impl Session {
    pub fn new(manager: Arc<Manager>, options: Options) -> Result<Self> {
        options.tcp.validate()?;
        let udp = Arc::new(udp::Session::new(manager, options.udp)?);
        let tcp = tcp::Session::new(udp.clone(), options.tcp)?;
        Ok(Self {
            udp,
            tcp,
            next_tcp: AtomicBool::new(true),
        })
    }
    pub async fn submit(&self, bytes: &[u8], generation: Option<u64>) -> Result<()> {
        let info = super::parse(bytes, self.udp.options().mtu)?;
        match info.protocol {
            6 => self.tcp.submit(bytes, generation),
            17 => self.udp.submit(bytes, generation).await,
            _ => Err(Error::Unsupported),
        }
    }
    /// 交替优先检查两队列；高频 UDP 返回不能饿死 TCP，队列为空不阻塞调用方。
    pub fn receive(&self) -> Result<Option<Packet>> {
        if self.next_tcp.fetch_xor(true, Ordering::Relaxed) {
            self.tcp
                .receive()?
                .map_or_else(|| self.udp.receive(), |packet| Ok(Some(packet)))
        } else {
            self.udp
                .receive()?
                .map_or_else(|| self.tcp.receive(), |packet| Ok(Some(packet)))
        }
    }
    pub(crate) fn same_identity(&self, other: &Self) -> bool {
        self.udp.same_identity(&other.udp)
    }
    pub async fn close(&self) -> Result<()> {
        self.tcp.close().await
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        let _ = crate::error::diagnostics::observe(
            self.udp.request_close(),
            crate::error::diagnostics::DiagnosticSite::Cleanup,
        );
    }
}
