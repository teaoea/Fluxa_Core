//! 宿主消费者示例：只使用本机UDP夹具，Core生成事件，宿主读取自建队列并打印。
use hickory_resolver::proto::{
    op::{Message, MessageType},
    rr::{RData, Record, rdata::A},
};
use nodus_core::{
    configuration::LogFilter,
    dns::{Config, Query, Resolver, Server},
    outbound::Manager,
};
use std::sync::{Arc, mpsc::sync_channel};
#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let peer = tokio::net::UdpSocket::bind("127.0.0.1:0").await?;
    let address = peer.local_addr()?;
    let task = tokio::spawn(async move {
        let mut bytes = [0u8; 4096];
        let (size, source) = peer.recv_from(&mut bytes).await?;
        let request = Message::from_vec(&bytes[..size])?;
        let question = request.queries.first().ok_or("missing question")?.clone();
        let mut reply = Message::query();
        reply.metadata.id = request.metadata.id;
        reply.metadata.message_type = MessageType::Response;
        reply.metadata.recursion_available = true;
        reply.add_query(question.clone());
        reply.add_answer(Record::from_rdata(
            question.name,
            30,
            RData::A(A::new(192, 0, 2, 8)),
        ));
        peer.send_to(&reply.to_vec()?, source).await?;
        Ok::<_, Box<dyn std::error::Error + Send + Sync>>(())
    });
    let resolver = Arc::new(Resolver::new(Config {
        servers: vec![Server::Udp {
            address: address.to_string(),
        }],
        timeout_ms: 1000,
        dnssec: false,
        fake_ip: None,
        service: None,
    })?);
    let manager = Manager::default();
    let (tx, rx) = sync_channel(16);
    manager.set_log_output(tx)?;
    manager.set_log_level(LogFilter::Info)?;
    let answer = manager
        .resolve_dns_query(
            &resolver,
            Query {
                name: "log.local.test".into(),
                query_type: "A".into(),
            },
            true,
        )
        .await?;
    assert_eq!(answer.addresses, ["192.0.2.8"]);
    for event in rx.try_iter() {
        println!("{}", serde_json::to_string(&event)?);
    }
    resolver.close().await?;
    task.await?.map_err(|e| e.to_string())?;
    Ok(())
}
