//! 测试文件：tests/test_dns_main.rs
//! 测试类型：单元测试、DNS 编排契约
//! 测试对象：src/dns/main.rs
//! 测试内容：close_stops_idle_dns_transport_before_waiting：关闭主动释放持有权限令牌的空闲传输。
//! 测试内容：initialization_failure_never_publishes_partial_endpoints：重复自举失败不暴露部分端点，不发生索引越界。
//! 测试内容：bootstrap_ttl_rebuild_and_system_dnssec_fail_closed：引导到期重建、无 DNSSEC 证明的系统结果不得放行。
//! 测试内容：hosts_precedes_fake_and_disabled_isolated：关闭隔离，hosts 优先，未知名上游失败不降级。
//! 测试依据：用户已确认 hosts 优先、DNS 关闭后 hosts/Fake-IP 停用及无隐式系统回退。
//! 前置条件：只用回环和私有根，不使用外部网络。
use super::*;
#[tokio::test]
async fn hosts_precedes_fake_and_disabled_isolated() {
    let client = crate::https::HttpsClient::system().unwrap();
    let raw:crate::configuration::models::Dns=crate::routing::runtime::yaml::decode(r#"{enable: true, server: ['127.0.0.1:9'], hosts: {example.com: 192.0.2.1}, fake-ip: {enable: true, ipv4-range: 198.18.0.0/24, ipv6-range: 'fd00::/120'}}"#).unwrap();
    let resolver = Resolver::from_main(&raw, Default::default(), client.tls_config())
        .await
        .unwrap()
        .unwrap();
    let answer = resolver
        .resolve(Query {
            name: "example.com".into(),
            query_type: "A".into(),
        })
        .await
        .unwrap();
    assert_eq!(answer.addresses, ["192.0.2.1"]);
    assert_eq!(answer.validation, "hosts");
    resolver.close().await.unwrap();
    assert!(matches!(
        resolver
            .resolve(Query {
                name: "example.com".into(),
                query_type: "A".into()
            })
            .await,
        Err(Error::InvalidState)
    ));
    let replacement = Arc::new(resolver.replacement().unwrap());
    assert_eq!(
        replacement
            .resolve(Query {
                name: "example.com".into(),
                query_type: "A".into()
            })
            .await
            .unwrap()
            .addresses,
        ["192.0.2.1"]
    );
    replacement.close().await.unwrap();
    let disabled: crate::configuration::models::Dns = crate::routing::runtime::yaml::decode(
        "{enable: false, server: ['invalid'], hosts: {example.com: bad}}",
    )
    .unwrap();
    assert!(
        Resolver::from_main(&disabled, Default::default(), client.tls_config())
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn bootstrap_ttl_rebuild_and_system_dnssec_fail_closed() {
    let fixture = crate::test_fixture_https::dns::fixture().await;
    let mut roots = rustls::RootCertStore::empty();
    roots.add(fixture.root.clone()).unwrap();
    let client = crate::https::HttpsClient::with_roots(roots).unwrap();
    let yaml = format!(
        "{{enable: true,bootstrap: ['127.0.0.1:{}'],server: ['https://connect.example.test:{}/dns-query,direct']}}",
        fixture.port, fixture.doh_port
    );
    let config: Dns = crate::routing::runtime::yaml::decode(&yaml).unwrap();
    let resolver = Resolver::from_main(&config, Default::default(), client.tls_config())
        .await
        .unwrap()
        .unwrap();
    let plan = resolver.main.as_ref().unwrap();
    let before = fixture.count.load(std::sync::atomic::Ordering::Relaxed);
    let bootstrap = plan.bootstrap.as_ref().unwrap();
    bootstrap.clear().unwrap();
    // 测试明确清空两层缓存，独立验证 A、AAAA 引导和 DoH 三条实际查询。
    for inner in &bootstrap.resources.read().await.as_ref().unwrap().inner {
        inner.clear_cache();
    }
    plan.endpoints.write().await[0].expires = Instant::now() - Duration::from_secs(1);
    let answer = resolver
        .resolve_real(Query {
            name: "connect.example.test".into(),
            query_type: "A".into(),
        })
        .await
        .unwrap();
    assert_eq!(answer.addresses, ["127.0.0.1"]);
    assert!(fixture.count.load(std::sync::atomic::Ordering::Relaxed) >= before + 3);
    assert!(plan.endpoints.read().await[0].expires > Instant::now());
    assert_eq!(plan.bootstrap("zero.example.test").await.unwrap().1, 0);
    assert!(
        crate::outbound::resolve_explicit_system(
            "127.0.0.1",
            crate::outbound::Family::Ipv6,
            resolver.authority.clone()
        )
        .await
        .unwrap()
        .is_empty()
    );
    resolver.close().await.unwrap();
    let config: Dns =
        crate::routing::runtime::yaml::decode("{enable: true,server: [system],dnssec: true}")
            .unwrap();
    let resolver = Resolver::from_main(&config, Default::default(), client.tls_config())
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(
        resolver
            .resolve_real(Query {
                name: "localhost".into(),
                query_type: "A".into()
            })
            .await,
        Err(Error::DnsValidation)
    ));
    resolver.close().await.unwrap();
}

#[tokio::test]
async fn initialization_failure_never_publishes_partial_endpoints() {
    let fixture = crate::test_fixture_https::dns::fixture().await;
    let client = crate::https::HttpsClient::system().unwrap();
    let bootstrap = Arc::new(
        Resolver::new(Config {
            servers: vec![Server::Udp {
                address: format!("127.0.0.1:{}", fixture.port),
            }],
            timeout_ms: 1000,
            dnssec: false,
            fake_ip: None,
            service: None,
        })
        .unwrap(),
    );
    let authority = Authority::new().unwrap();
    let runtime = ManagedRuntime::new(authority.clone());
    let plan = Plan {
        settings: vec![
            format!("127.0.0.1:{}", fixture.port),
            "https://nxdomain.example.test/dns-query".into(),
        ],
        bootstrap: Some(bootstrap),
        hosts: Default::default(),
        endpoints: tokio::sync::RwLock::new(Vec::new()),
        nodes: Default::default(),
        tls: client.tls_config(),
        timeout: Duration::from_secs(1),
        dnssec: false,
        authority,
        _memory: METADATA_BUDGET.reserve(65536).unwrap(),
    };
    // 夹具 nxdomain.* 是 NXDOMAIN；失败发生在第一项已准备之后。
    for _ in 0..2 {
        assert!(plan.initialize(&runtime).await.is_err());
        assert!(plan.endpoints.read().await.is_empty());
    }
    plan.close().await.unwrap();
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn close_stops_idle_dns_transport_before_waiting() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let client = crate::https::HttpsClient::system().unwrap();
    let config: Dns =
        crate::routing::runtime::yaml::decode("{enable: true,server: ['127.0.0.1:9']}").unwrap();
    let resolver = Resolver::from_main(&config, Default::default(), client.tls_config())
        .await
        .unwrap()
        .unwrap();
    let network = Arc::new(crate::outbound::Manager::default());
    network
        .configure_dns_transport(None, None, resolver.authority.clone())
        .unwrap();
    let opened = network
        .connect_configured(crate::outbound::RoutedTarget {
            target: crate::routing::Target {
                domain: None,
                address: Some(address.ip().to_string()),
                port: address.port(),
                transport: crate::routing::Transport::Tcp,
            },
            family: crate::outbound::Family::Auto,
            timeout_ms: 1000,
            mapping_generation: None,
        })
        .await
        .unwrap();
    let (_peer, _) = listener.accept().await.unwrap();
    resolver.main.as_ref().unwrap().endpoints.write().await[0].network = Some(network.clone());
    // 保留旧解析器和连接管理器引用，关闭仍必须主动释放连接，不能依赖 drop。
    tokio::time::timeout(Duration::from_secs(2), resolver.close())
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(
        network.close(opened.connection.id).await,
        Err(Error::InvalidState)
    ));
    assert!(matches!(
        resolver
            .resolve(Query {
                name: "example.test".into(),
                query_type: "A".into()
            })
            .await,
        Err(Error::InvalidState)
    ));
}
