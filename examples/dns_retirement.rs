//! 本地可运行示例：DNS 解析中替换配置，已准入建连完成，旧引用不能发起新查询。
//! 只使用回环 UDP/TCP，不访问外网、不修改系统 DNS、不接管 VPN。
use fluxa_core::{
    dns::{Config, Query, Resolver, Server},
    error::Error,
    outbound::{DomainTarget, Family, Manager},
    routing::{Policy, Transport},
};
use hickory_resolver::proto::{
    op::{Message, MessageType},
    rr::{RData, Record, rdata::A},
};
use std::{sync::Arc, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, UdpSocket},
    sync::oneshot,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let udp = UdpSocket::bind("127.0.0.1:0").await?;
    let resolver = Arc::new(Resolver::new(Config {
        servers: vec![Server::Udp {
            address: udp.local_addr()?.to_string(),
        }],
        timeout_ms: 2000,
        dnssec: false,
        fake_ip: None,
        service: None,
    })?);
    let tcp = TcpListener::bind("127.0.0.1:0").await?;
    let port = tcp.local_addr()?.port();
    let (arrived_tx, arrived) = oneshot::channel();
    let (release, released) = oneshot::channel();
    let server = tokio::spawn(async move {
        let mut buffer = [0; 4096];
        let (len, peer) = udp.recv_from(&mut buffer).await.unwrap();
        let mut reply = Message::from_vec(&buffer[..len]).unwrap();
        arrived_tx.send(()).unwrap();
        released.await.unwrap();
        reply.metadata.message_type = MessageType::Response;
        reply.metadata.recursion_available = true;
        reply.add_answer(Record::from_rdata(
            reply.queries[0].name.clone(),
            30,
            RData::A(A(std::net::Ipv4Addr::LOCALHOST)),
        ));
        udp.send_to(&reply.to_vec().unwrap(), peer).await.unwrap();
        let (mut socket, _) = tcp.accept().await.unwrap();
        let mut bytes = [0; 2];
        socket.read_exact(&mut bytes).await.unwrap();
        socket.write_all(&bytes).await.unwrap();
    });
    let manager = Arc::new(Manager::default());
    manager.configure_dns(Some(resolver.clone()))?;
    let old_snapshot = manager.network_snapshot()?;
    let own = manager.clone();
    let connecting = tokio::spawn(async move {
        own.connect_configured_domain(DomainTarget {
            domain: "retire.example.test".into(),
            port,
            transport: Transport::Tcp,
            policy: Policy::Direct,
            timeout_ms: 3000,
            family: Family::Ipv4,
        })
        .await
    });
    tokio::time::timeout(Duration::from_secs(2), arrived).await??;
    // 同步返回只表示发布新配置且旧准入已关闭，不声称后台清理已经完成。
    manager.configure_dns(None)?;
    assert_eq!(
        resolver
            .resolve(Query {
                name: "new.example.test".into(),
                query_type: "A".into()
            })
            .await
            .unwrap_err(),
        Error::InvalidState
    );
    release.send(()).unwrap();
    let opened = connecting.await??;
    manager.write(opened.id, b"ok", 1000).await?;
    assert_eq!(&*manager.read(opened.id, 2, 1000).await?.bytes, b"ok");
    manager.close(opened.id).await?;
    server.await?;
    // 此时旧事务已结束；显式关闭只用于示例退出前等待清理，不作为配置提交的必要步骤。
    // 若仍有在途事务，主动 close 会取消它们，不能用它模拟“只等待退休”。
    resolver.close().await?;
    assert!(old_snapshot.resolver.is_some());
    println!("旧引用保留，新查询被拒绝；配置替换中的建连及后续真实 TCP 读写成功");
    Ok(())
}
