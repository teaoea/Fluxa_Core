//! H1 独立进程回归：避免其他测试的存活重叠池改变本案例前置条件。
use libfluxa_core::{
    dns::{Config, Resolver, Server, fake_ip},
    outbound::{Limits, Manager},
    tun::udp::{Options, Session},
};
use std::sync::Arc;
/// H1 最小复现：释放旧解析器与会话后，扩大重叠区间仍能创建会话；旧地址不可重新归属。
#[tokio::test]
async fn retired_fake_ip_range_can_expand_and_reopen_tun() {
    let make = |range: &str| {
        Arc::new(
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
        )
    };
    let old = make("198.18.0.0/16");
    let setup_h1 = |dns: Arc<Resolver>| {
        let manager = Arc::new(Manager::default());
        manager
            .configure_network(Some(dns), None, Limits::default())
            .unwrap();
        let session = Session::new(
            manager.clone(),
            Options {
                dns_endpoints: vec!["192.0.2.53:53".parse().unwrap()],
                ..Options::default()
            },
        )
        .unwrap();
        (manager, session)
    };
    let (manager, session) = setup_h1(old.clone());
    let old_ip = old
        .allocate_fake_ip(libfluxa_core::dns::Query {
            name: "retired-h1.test".into(),
            query_type: "A".into(),
        })
        .unwrap()
        .addresses[0]
        .clone();
    session.close().unwrap();
    drop(session);
    drop(manager);
    drop(old);
    let new = make("198.18.0.0/15");
    let (_manager, session) = setup_h1(new.clone());
    let current = new
        .allocate_fake_ip(libfluxa_core::dns::Query {
            name: "current-h1.test".into(),
            query_type: "A".into(),
        })
        .unwrap()
        .addresses[0]
        .clone();
    assert_ne!(old_ip, current);
    assert!(new.reverse_fake_ip(&old_ip).unwrap().is_none());
    session.close().unwrap();
}
