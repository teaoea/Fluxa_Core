//! 自研 VLESS、REALITY 与 Vision 协议；不包含第三方代理内核。
pub mod config;
mod reality;
mod stream;
mod vision;
mod vless;
pub use stream::{Stream, connect};
mod crypto;
pub mod fingerprint;
mod inner_tls;

mod hybrid;

mod datagram;
pub use datagram::{Datagram, connect_udp};

/// 当前固定服务端互通的 XUDP 单报文上限。
pub fn datagram_limit() -> usize {
    datagram::MAX_PAYLOAD
}

pub mod quic;
