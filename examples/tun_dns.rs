//! 可运行的 TUN DNS 合成响应示例；不创建系统隧道，合成查询不访问上游。
use fluxa_core::{
    dns::{Config, Resolver, Server, fake_ip},
    outbound::{Limits, Manager},
    routing::{Config as Routing, Policy, Router},
    tun::{
        parse,
        udp::{Options, Session, encode},
    },
};
use hickory_resolver::proto::{
    op::{Message, Query},
    rr::{Name, RecordType},
};
use std::sync::Arc;
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manager = Arc::new(Manager::default());
    manager.configure_network(
        Some(Arc::new(Resolver::new(Config {
            servers: vec![Server::Udp {
                address: "1.1.1.1:53".into(),
            }],
            timeout_ms: 1000,
            dnssec: false,
            service: None,
            fake_ip: Some(fake_ip::Config {
                ipv4_range: "198.18.0.0/16".into(),
                ipv6_range: "fd00:198:18::/64".into(),
                capacity: 32,
                exclude: vec![],
            }),
        })?)),
        Some(Arc::new(Router::compile(Routing {
            fallback: Policy::Direct,
            ..Routing::default()
        })?)),
        Limits::default(),
    )?;
    let session = Session::new(
        manager,
        Options {
            dns_endpoints: vec!["198.19.0.53:53".parse()?],
            ..Options::default()
        },
    )?;
    let mut query = Message::query();
    query.metadata.id = 1;
    query.metadata.recursion_desired = true;
    query.add_query(Query::query(
        Name::from_ascii("example.com")?,
        RecordType::A,
    ));
    let request = encode(
        "192.0.2.1:12345".parse()?,
        "198.19.0.53:53".parse()?,
        &query.to_vec()?,
        1500,
    )?;
    session.submit(&request.bytes, None).await?;
    let packet = session.receive()?.ok_or("DNS 返回包缺失")?;
    let info = parse(&packet.bytes, 1500)?;
    let answer = Message::from_vec(&packet.bytes[packet.bytes.len() - info.payload_bytes..])?;
    println!("合成 DNS 响应：{:?}", answer.answers);
    session.close().await?;
    Ok(())
}
