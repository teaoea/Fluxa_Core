//! 隔离 Linux 系统 TCP 互通夹具：标准输入/输出传递长度分帧 IP 包，不创建网络接口。
//! 网络命名空间与 TUN 由独立 Python 平台测试管理，业务逻辑仍复用 Rust 会话。
use nodus_core::{
    dns::{Config as DnsConfig, Resolver, Server, fake_ip},
    error::{Error, Result},
    outbound::{Limits, Manager},
    routing::{Config, Policy, Router},
    tun::{
        session::{Options, Session},
        tcp, udp,
    },
};
use std::{
    io::{Read, Write},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
#[tokio::main]
async fn main() -> Result<()> {
    let port: u16 = std::env::args()
        .nth(1)
        .ok_or(Error::InvalidInput)?
        .parse()
        .map_err(|_| Error::InvalidInput)?;
    if port == 0 {
        return Err(Error::InvalidInput);
    }
    let resolver = Arc::new(Resolver::new(DnsConfig {
        servers: vec![Server::Udp {
            address: format!("127.0.0.1:{port}"),
        }],
        timeout_ms: 1000,
        dnssec: false,
        fake_ip: Some(fake_ip::Config {
            ipv4_range: "198.18.0.0/16".into(),
            ipv6_range: "fd00:198:18::/64".into(),
            capacity: 16,
            exclude: vec![],
        }),
        service: None,
    })?);
    let manager = Arc::new(Manager::default());
    manager.configure_network(
        Some(resolver.clone()),
        Some(Arc::new(Router::compile(Config {
            fallback: Policy::Direct,
            ..Config::default()
        })?)),
        Limits::default(),
    )?;
    let session = Arc::new(Session::new(
        manager,
        Options {
            udp: udp::Options {
                dns_endpoints: vec!["192.0.2.53:53".parse().map_err(|_| Error::Internal)?],
                ..udp::Options::default()
            },
            tcp: tcp::Options {
                input_capacity: 16,
                time_wait_ms: 1000,
                ..tcp::Options::default()
            },
        },
    )?);
    let name = "connect.ipv4only.example.test";
    let v4 = resolver
        .allocate_fake_ip(nodus_core::dns::Query {
            name: name.into(),
            query_type: "A".into(),
        })?
        .addresses;
    let v6 = resolver
        .allocate_fake_ip(nodus_core::dns::Query {
            name: name.into(),
            query_type: "AAAA".into(),
        })?
        .addresses;
    eprintln!("READY {} {}", v4[0], v6[0]);
    let done = Arc::new(AtomicBool::new(false));
    let reader = session.clone();
    let stopped = done.clone();
    let runtime = tokio::runtime::Handle::current();
    let input = tokio::task::spawn_blocking(move || {
        let mut input = std::io::stdin().lock();
        let mut length = [0; 2];
        while input.read_exact(&mut length).is_ok() {
            let length = usize::from(u16::from_be_bytes(length));
            if !(20..=9000).contains(&length) {
                break;
            }
            let mut bytes = zeroize::Zeroizing::new(vec![0; length]);
            if input.read_exact(&mut bytes).is_err() {
                break;
            }
            if let Err(error) = runtime.block_on(reader.submit(&bytes, None)) {
                eprintln!("数据包拒绝：{error}");
            }
        }
        stopped.store(true, Ordering::Release);
        let _ = runtime.block_on(reader.close());
    });
    while !done.load(Ordering::Acquire) {
        match session.receive() {
            Ok(Some(packet)) => {
                let mut output = std::io::stdout().lock();
                output
                    .write_all(&(packet.bytes.len() as u16).to_be_bytes())
                    .map_err(Error::from)?;
                output.write_all(&packet.bytes).map_err(Error::from)?;
                output.flush().map_err(Error::from)?;
            }
            Ok(None) => tokio::time::sleep(std::time::Duration::from_millis(1)).await,
            Err(Error::InvalidState) if done.load(Ordering::Acquire) => break,
            Err(error) => eprintln!("会话错误：{error}"),
        }
    }
    input.await.map_err(|_| Error::Internal)?;
    Ok(())
}
