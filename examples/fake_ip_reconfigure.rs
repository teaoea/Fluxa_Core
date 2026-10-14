//! H1 可运行示例：退役解析器释放后变更池范围，重新绑定且不复用历史地址。
//! 仅验证 Rust 会话与映射，不开启系统 VPN，不请求 DNS 上游。
use libfluxa_core::{
    dns::{Config, Query, Resolver, Server, fake_ip},
    outbound::{Limits, Manager},
    tun::udp::{Options, Session},
};
use std::sync::Arc;
fn open(range: &str) -> (Arc<Resolver>, Arc<Manager>, Session) {
    let resolver = Arc::new(
        Resolver::new(Config {
            servers: vec![Server::Udp {
                address: "127.0.0.1:5353".into(),
            }],
            timeout_ms: 100,
            dnssec: false,
            service: None,
            fake_ip: Some(fake_ip::Config {
                ipv4_range: range.into(),
                ipv6_range: "fd00:198:ffff::/64".into(),
                capacity: 2,
                exclude: vec![],
            }),
        })
        .unwrap(),
    );
    let manager = Arc::new(Manager::default());
    manager
        .configure_network(Some(resolver.clone()), None, Limits::default())
        .unwrap();
    let session = Session::new(
        manager.clone(),
        Options {
            dns_endpoints: vec!["192.0.2.53:53".parse().unwrap()],
            ..Options::default()
        },
    )
    .unwrap();
    (resolver, manager, session)
}
fn main() {
    let (old, manager, session) = open("198.18.0.0/16");
    let old_ip = old
        .allocate_fake_ip(Query {
            name: "old.example".into(),
            query_type: "AAAA".into(),
        })
        .unwrap()
        .addresses[0]
        .clone();
    session.close().unwrap();
    drop((session, manager, old));
    let (new, _manager, session) = open("198.18.0.0/15");
    let current = new
        .allocate_fake_ip(Query {
            name: "new.example".into(),
            query_type: "AAAA".into(),
        })
        .unwrap()
        .addresses[0]
        .clone();
    assert_ne!(old_ip, current);
    assert!(new.reverse_fake_ip(&old_ip).unwrap().is_none());
    session.close().unwrap();
    println!("区间变更后重新绑定成功；旧地址 {old_ip} 未重新分配，新地址 {current}");
}
