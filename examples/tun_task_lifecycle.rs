//! 本地 UDP 短流示例：顺序创建和结束 48 条流，不改系统网络或接管 VPN。
//! 内部任务节点回收和登记上限由私有单元测试断言；示例不充当 RSS/压力验收。
use fluxa_core::{
    outbound::{Limits, Manager},
    routing::{Config, Policy, Router},
    tun::udp::{Options, Session, encode},
};
use std::{net::SocketAddr, sync::Arc, time::Duration};
use tokio::net::UdpSocket;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manager = Arc::new(Manager::default());
    manager.configure_network(
        None,
        Some(Arc::new(Router::compile(Config {
            fallback: Policy::Direct,
            ..Config::default()
        })?)),
        Limits::default(),
    )?;
    let session = Session::new(
        manager.clone(),
        Options {
            idle_timeout_ms: 25,
            ..Options::default()
        },
    )?;
    let peer = UdpSocket::bind("127.0.0.1:0").await?;
    for index in 0..48_u16 {
        let source = SocketAddr::new("192.0.2.1".parse()?, 10000 + index);
        let packet = encode(source, peer.local_addr()?, b"ok", 1500)?;
        session.submit(&packet.bytes, None).await?;
        let mut bytes = [0; 2];
        let (_, address) =
            tokio::time::timeout(Duration::from_secs(2), peer.recv_from(&mut bytes)).await??;
        peer.send_to(&bytes, address).await?;
        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                if let Some(packet) = session.receive()? {
                    assert_eq!(&packet.bytes[packet.bytes.len() - 2..], b"ok");
                    break;
                }
                tokio::task::yield_now().await;
            }
            Ok::<_, fluxa_core::error::Error>(())
        })
        .await??;
        // 只读观察实际连接退出，不接管内核任务表或代替内核执行回收。
        tokio::time::timeout(Duration::from_secs(2), async {
            while !manager.status()?.sessions.is_empty() {
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
            Ok::<_, fluxa_core::error::Error>(())
        })
        .await??;
    }
    session.close().await?;
    assert!(manager.status()?.sessions.is_empty());
    println!("48 条 UDP 短流真实往返与退出完成；会话关闭完成，活动连接为零");
    Ok(())
}
