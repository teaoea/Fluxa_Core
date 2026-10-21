//! 本地可运行示例：复用解析器进行独立查询，关闭后旧引用明确失败。
//! 内部预算耗尽的确定性注入见 dns::runtime::tests，不向外部暴露资源管理权。
//! 不访问外网、不接管系统 VPN；回环服务仅返回 example.test 的测试地址。
use fluxa_core::{
    dns::{Config, Query, Resolver, Server},
    error::Error,
};
use hickory_resolver::proto::{
    op::{Message, MessageType},
    rr::{RData, Record, rdata::A},
};
use std::sync::Arc;
use tokio::net::UdpSocket;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let socket = UdpSocket::bind("127.0.0.1:0").await?;
    let address = socket.local_addr()?.to_string();
    let upstream = tokio::spawn(async move {
        let mut buffer = [0; 4096];
        let (length, peer) = socket.recv_from(&mut buffer).await?;
        let request = Message::from_vec(&buffer[..length])?;
        let mut response = Message::query();
        response.metadata.id = request.metadata.id;
        response.metadata.message_type = MessageType::Response;
        response.metadata.recursion_desired = true;
        response.metadata.recursion_available = true;
        for query in request.queries {
            response.add_query(query.clone());
            response.add_answer(Record::from_rdata(
                query.name,
                60,
                RData::A(A::new(192, 0, 2, 44)),
            ));
        }
        socket.send_to(&response.to_vec()?, peer).await?;
        Ok::<_, Box<dyn std::error::Error + Send + Sync>>(())
    });
    let resolver = Arc::new(Resolver::new(Config {
        servers: vec![Server::Udp { address }],
        timeout_ms: 1000,
        dnssec: false,
        fake_ip: None,
        service: None,
    })?);
    let query = || Query {
        name: "example.test".into(),
        query_type: "A".into(),
    };
    // 每次 resolve 是独立操作；瞬时 8001/8002 只拒绝当时操作，
    // 后续请求可重新准入。这里不伪造预算故障，也不做无界自动重试。
    let answer = resolver.resolve(query()).await?;
    assert_eq!(answer.addresses, ["192.0.2.44"]);
    assert!(resolver.resolve(query()).await?.from_cache);
    resolver.close().await?;
    assert_eq!(
        resolver.resolve(query()).await.unwrap_err(),
        Error::InvalidState
    );
    upstream
        .await?
        .map_err(|error| error as Box<dyn std::error::Error>)?;
    println!("独立查询与缓存成功；关闭后的旧解析器返回 9001 状态错误");
    Ok(())
}
