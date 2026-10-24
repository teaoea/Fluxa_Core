//! 测试文件：tests/test_dns_logging.rs
//! 测试类型：单元测试、真实回环集成与日志隔离回归
//! 测试对象：src/dns/logging.rs、src/dns/mod.rs、src/dns/service.rs、src/outbound/resolution.rs
//! 测试内容：positive_negative_cache_events_keep_dns_semantics、upstream_failover_keeps_query_and_connection_identity、hosts_and_fake_ip_never_claim_real_upstream、shared_resolver_never_crosses_consumers、invalid_closed_and_cancelled_queries_keep_original_failures、udp_tcp_service_queries_use_the_same_event_pipeline、connection_dns_events_share_outbound_connection_id、backpressure_or_missing_consumer_never_changes_dns_results、panic_during_query_trace_is_failed_not_cancelled。
//! 测试依据：用户要求DNS查询/缓存/上游/错误实时输出，日志无历史、不改变解析、共享解析器隔离消费者且关联真实连接ID。
//! 前置条件：本地UDP/TCP夹具，不使用外部DNS、不改系统网络或写日志文件。
use super::*;
use crate::dns::{Config, Query, Resolver, Server};
use crate::{
    configuration::LogFilter,
    logging::{Event, Record},
    outbound::Manager,
};
use std::sync::{
    Arc,
    atomic::Ordering,
    mpsc::{Receiver, sync_channel},
};
fn consumer() -> (Arc<Manager>, Receiver<Record>) {
    let manager = Arc::new(Manager::default());
    let (tx, rx) = sync_channel(128);
    manager.set_log_output(tx).unwrap();
    manager.set_log_level(LogFilter::Info).unwrap();
    (manager, rx)
}
fn resolver(port: u16) -> Arc<Resolver> {
    Arc::new(
        Resolver::new(Config {
            servers: vec![Server::Udp {
                address: format!("127.0.0.1:{port}"),
            }],
            timeout_ms: 100,
            dnssec: false,
            fake_ip: None,
            service: None,
        })
        .unwrap(),
    )
}
fn query(name: &str) -> Query {
    Query {
        name: name.into(),
        query_type: "A".into(),
    }
}
fn records(rx: &Receiver<Record>) -> Vec<Record> {
    rx.try_iter().filter(|v| v.module == "dns").collect()
}
fn event(rows: &[Record], event: Event) -> &Record {
    rows.iter().find(|r| r.event == event).unwrap()
}
#[tokio::test]
async fn positive_negative_cache_events_keep_dns_semantics() {
    let peer = crate::test_fixture_https::dns::fixture().await;
    let resolver = resolver(peer.port);
    let (manager, rx) = consumer();
    for (name, rcode) in [("example.test", 0), ("nxdomain.example.test", 3)] {
        let answer = manager
            .resolve_dns_query(&resolver, query(name), true)
            .await
            .unwrap();
        assert_eq!(answer.rcode, rcode);
        assert!(!answer.from_cache);
        let first = records(&rx);
        let id = event(&first, Event::DnsQueryStarted)
            .context
            .dns
            .unwrap()
            .query_id;
        assert!(first.iter().all(|v| v.context.dns.unwrap().query_id == id));
        assert_eq!(
            event(&first, Event::DnsQueryCompleted)
                .context
                .dns
                .unwrap()
                .rcode,
            Some(rcode)
        );
        assert!(first.iter().all(|v| v.error_info.is_none()));
        let count = peer.count.load(Ordering::SeqCst);
        let cached = manager
            .resolve_dns_query(&resolver, query(name), true)
            .await
            .unwrap();
        assert!(cached.from_cache);
        assert_eq!(cached.rcode, rcode);
        assert_eq!(peer.count.load(Ordering::SeqCst), count);
        let second = records(&rx);
        assert_ne!(
            event(&second, Event::DnsQueryStarted)
                .context
                .dns
                .unwrap()
                .query_id,
            id
        );
        assert_eq!(
            event(&second, Event::DnsCacheHit)
                .context
                .dns
                .unwrap()
                .rcode,
            Some(rcode)
        );
        assert!(!second.iter().any(|v| v.event == Event::DnsUpstreamSelected));
    }
    resolver.close().await.unwrap();
}
#[tokio::test]
async fn upstream_failover_keeps_query_and_connection_identity() {
    let peer = crate::test_fixture_https::dns::fixture().await;
    let blackhole = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let first = blackhole.local_addr().unwrap().to_string();
    let resolver = Arc::new(
        Resolver::new(Config {
            servers: vec![
                Server::Udp {
                    address: first.clone(),
                },
                Server::Udp {
                    address: format!("127.0.0.1:{}", peer.port),
                },
            ],
            timeout_ms: 100,
            dnssec: false,
            fake_ip: None,
            service: None,
        })
        .unwrap(),
    );
    let (manager, rx) = consumer();
    let answer = resolver
        .resolve_logged(
            query("example.test"),
            false,
            None,
            Some(Scope::new(manager.logs.clone(), Some(900), Some(91))),
        )
        .await
        .unwrap();
    assert_eq!(answer.addresses, ["192.0.2.44"]);
    let rows = records(&rx);
    let id = rows[0].context.dns.unwrap().query_id;
    assert!(rows.iter().all(|v| v.context.connection_id == Some(900)
        && v.revision == Some(91)
        && v.context.dns.unwrap().query_id == id));
    let selected: Vec<_> = rows
        .iter()
        .filter(|v| v.event == Event::DnsUpstreamSelected)
        .map(|v| v.context.dns.unwrap().upstream_index.unwrap())
        .collect();
    assert_eq!(selected, [0, 1]);
    let failed = event(&rows, Event::DnsUpstreamFailed);
    assert_eq!(failed.error_info.unwrap().code, Error::DnsTimeout.code());
    assert_eq!(
        serde_json::to_value(failed.context.dns.unwrap()).unwrap()["upstream"],
        first
    );
    assert!(rows.iter().any(|v| v.event == Event::DnsQueryCompleted));
    resolver.close().await.unwrap();
}
#[tokio::test]
async fn hosts_and_fake_ip_never_claim_real_upstream() {
    let peer = crate::test_fixture_https::dns::fixture().await;
    let settings:crate::configuration::models::Dns=crate::routing::runtime::yaml::decode(&format!("{{enable: true, server: ['127.0.0.1:{}'], hosts: {{hosts.example.test: 192.0.2.8}}, fake-ip: {{enable: true, ipv4-range: 198.18.0.0/24, ipv6-range: 'fd00::/120'}}}}",peer.port)).unwrap();
    let resolver = Resolver::from_main(
        &settings,
        Default::default(),
        crate::https::HttpsClient::system().unwrap().tls_config(),
        None,
    )
    .await
    .unwrap()
    .unwrap();
    let (manager, rx) = consumer();
    for (name, expected) in [
        ("hosts.example.test", Event::DnsHostsHit),
        ("fake.example.test", Event::DnsSynthetic),
    ] {
        let answer = manager
            .resolve_dns_query(&resolver, query(name), false)
            .await
            .unwrap();
        let rows = records(&rx);
        assert!(rows.iter().any(|v| v.event == expected));
        assert!(
            !rows
                .iter()
                .any(|v| v.event == Event::DnsUpstreamSelected || v.event == Event::DnsCacheHit)
        );
        if expected == Event::DnsHostsHit {
            assert_eq!(answer.addresses, ["192.0.2.8"]);
        } else {
            assert_eq!(answer.validation, "synthetic");
        }
    }
    assert_eq!(peer.count.load(Ordering::SeqCst), 0);
    resolver.close().await.unwrap();
}
#[tokio::test]
async fn shared_resolver_never_crosses_consumers() {
    let peer = crate::test_fixture_https::dns::fixture().await;
    let resolver = resolver(peer.port);
    let (first, a) = consumer();
    let (second, b) = consumer();
    first
        .resolve_dns_query(&resolver, query("example.test"), true)
        .await
        .unwrap();
    assert!(!records(&a).is_empty());
    assert!(records(&b).is_empty());
    second
        .resolve_dns_query(&resolver, query("example.test"), true)
        .await
        .unwrap();
    assert!(records(&a).is_empty());
    assert!(records(&b).iter().any(|v| v.event == Event::DnsCacheHit));
    resolver.close().await.unwrap();
}
#[tokio::test]
async fn invalid_closed_and_cancelled_queries_keep_original_failures() {
    let peer = crate::test_fixture_https::dns::fixture().await;
    let resolver = resolver(peer.port);
    let (manager, rx) = consumer();
    assert!(matches!(
        manager
            .resolve_dns_query(&resolver, query("bad name"), true)
            .await,
        Err(Error::InvalidInput)
    ));
    let rows = records(&rx);
    assert_eq!(
        event(&rows, Event::DnsQueryFailed).error_info.unwrap().code,
        Error::InvalidInput.code()
    );
    assert!(!rows.iter().any(|v| v.event == Event::DnsUpstreamSelected));
    let own = resolver.clone();
    let caller = manager.clone();
    let worker = tokio::spawn(async move {
        caller
            .resolve_dns_query(&own, query("timeout.example.test"), true)
            .await
    });
    let mut rows = Vec::new();
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        loop {
            rows.extend(records(&rx));
            if rows.iter().any(|v| v.event == Event::DnsUpstreamSelected) {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    worker.abort();
    assert!(worker.await.unwrap_err().is_cancelled());
    rows.extend(records(&rx));
    assert_eq!(
        rows.iter()
            .filter(|v| v.event == Event::DnsQueryCancelled)
            .count(),
        1
    );
    assert!(!rows.iter().any(|v| v.event == Event::DnsQueryCompleted));
    resolver.close().await.unwrap();
    assert!(matches!(
        manager
            .resolve_dns_query(&resolver, query("example.test"), true)
            .await,
        Err(Error::InvalidState)
    ));
    assert_eq!(
        event(&records(&rx), Event::DnsQueryFailed)
            .error_info
            .unwrap()
            .code,
        Error::InvalidState.code()
    );
}
#[tokio::test]
async fn udp_tcp_service_queries_use_the_same_event_pipeline() {
    use hickory_resolver::proto::{
        op::Message,
        rr::{Name, RecordType},
    };
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let peer = crate::test_fixture_https::dns::fixture().await;
    let resolver = resolver(peer.port);
    let (manager, rx) = consumer();
    let mut service = crate::dns::service::Service::start_logged(
        crate::dns::service::Config {
            listen: "127.0.0.1:0".into(),
            allow_lan: false,
            allowed_clients: vec![],
        },
        resolver.clone(),
        Some(Scope::new(manager.logs.clone(), None, None)),
    )
    .await
    .unwrap();
    let status = service.status();
    let address = status.address;
    let mut request = Message::query();
    request.metadata.recursion_desired = true;
    request.add_query(hickory_resolver::proto::op::Query::query(
        Name::from_ascii("example.test.").unwrap(),
        RecordType::A,
    ));
    let packet = request.to_vec().unwrap();
    let udp = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
    udp.send_to(&packet, &address).await.unwrap();
    let mut buf = [0u8; 4096];
    let (size, _) =
        tokio::time::timeout(std::time::Duration::from_secs(2), udp.recv_from(&mut buf))
            .await
            .unwrap()
            .unwrap();
    assert!(!Message::from_vec(&buf[..size]).unwrap().answers.is_empty());
    let mut tcp = tokio::net::TcpStream::connect(&address).await.unwrap();
    tcp.write_u16(packet.len() as u16).await.unwrap();
    tcp.write_all(&packet).await.unwrap();
    let size = tcp.read_u16().await.unwrap();
    let mut reply = vec![0; size as usize];
    tcp.read_exact(&mut reply).await.unwrap();
    assert!(!Message::from_vec(&reply).unwrap().answers.is_empty());
    let rows = records(&rx);
    assert_eq!(
        rows.iter()
            .filter(|v| v.event == Event::DnsQueryStarted)
            .count(),
        2
    );
    assert_eq!(
        rows.iter()
            .filter(|v| v.event == Event::DnsQueryCompleted)
            .count(),
        2
    );
    assert!(rows.iter().any(|v| v.event == Event::DnsCacheHit));
    drop(tcp);
    service.stop().await.unwrap();
    resolver.close().await.unwrap();
}
#[tokio::test]
async fn connection_dns_events_share_outbound_connection_id() {
    use crate::{
        outbound::{Family, RoutedTarget},
        routing::{Config as RoutingConfig, Router, Target, Transport},
    };
    let peer = crate::test_fixture_https::dns::fixture().await;
    let resolver = resolver(peer.port);
    let (manager, rx) = consumer();
    manager
        .configure_network(
            Some(resolver.clone()),
            Some(Arc::new(
                Router::compile(RoutingConfig {
                    fallback: crate::routing::Policy::Direct,
                    ..Default::default()
                })
                .unwrap(),
            )),
            Default::default(),
        )
        .unwrap();
    // DNS夹具返回127.0.0.1，成功建连与查询事件使用同一真实连接ID。
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let opened = manager
        .connect_configured(RoutedTarget {
            target: Target {
                domain: Some("connect.example.test".into()),
                address: None,
                port: listener.local_addr().unwrap().port(),
                transport: Transport::Tcp,
            },
            timeout_ms: 200,
            family: Family::Ipv4,
            mapping_generation: None,
        })
        .await
        .unwrap();
    let (_peer, _) = listener.accept().await.unwrap();
    manager.close(opened.connection.id).await.unwrap();
    let all: Vec<_> = rx.try_iter().collect();
    let id = all
        .iter()
        .find(|v| v.event == Event::ConnectionCreated)
        .unwrap()
        .context
        .connection_id
        .unwrap();
    let dns: Vec<_> = all.iter().filter(|v| v.module == "dns").collect();
    assert!(!dns.is_empty());
    assert!(dns.iter().all(|v| v.context.connection_id == Some(id)));
    assert!(
        all.iter()
            .any(|v| v.event == Event::ConnectionEstablished && v.context.connection_id == Some(id))
    );
    resolver.close().await.unwrap();
}

#[tokio::test]
async fn backpressure_or_missing_consumer_never_changes_dns_results() {
    let peer = crate::test_fixture_https::dns::fixture().await;
    let resolver = resolver(peer.port);
    let (manager, rx) = consumer();
    drop(rx);
    let (tx, rx) = sync_channel(1);
    manager.set_log_output(tx).unwrap();
    let answer = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        manager.resolve_dns_query(&resolver, query("example.test"), true),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(answer.addresses, ["192.0.2.44"]);
    assert_eq!(rx.try_iter().count(), 1);
    manager.detach_log_output().unwrap();
    let answer = manager
        .resolve_dns_query(&resolver, query("other.example.test"), true)
        .await
        .unwrap();
    assert_eq!(answer.addresses, ["192.0.2.44"]);
    let (tx, rx) = sync_channel(16);
    manager.set_log_output(tx).unwrap();
    assert!(rx.try_recv().is_err(), "不回放失联期间的DNS日志");
    resolver.close().await.unwrap();
}

#[test]
fn panic_during_query_trace_is_failed_not_cancelled() {
    let (manager, rx) = consumer();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _trace = Trace::begin(
            Some(Scope::new(manager.logs.clone(), None, None)),
            &query("panic.example.test"),
        );
        panic!("仅注入DNS日志作用域展开异常");
    }));
    assert!(result.is_err());
    let rows = records(&rx);
    assert_eq!(
        rows.iter()
            .filter(|v| v.event == Event::DnsQueryFailed)
            .count(),
        1
    );
    assert_eq!(
        event(&rows, Event::DnsQueryFailed).error_info.unwrap().code,
        Error::Internal.code()
    );
    assert!(!rows.iter().any(|v| v.event == Event::DnsQueryCancelled));
}
