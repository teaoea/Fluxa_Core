//! 测试文件：tests/test_dns_geoip_contract.rs
//! 测试类型：集成测试、独立配置与出站语义
//! 测试对象：DNS、GeoIP、主配置事务、共用出站及真实 C ABI
//! 测试内容：
//! - admitted_family_fallback_survives_configuration_replacement：两族尝试共享准入，替换后旧事务完成而新请求走新配置。
//! - named_numeric_dns_failure_never_uses_direct：数字 DNS 指定失败节点，不向 DNS 直接发送报文。
//! - explicit_doh_ignores_formal_route_and_retires_old_resolver：私有根 DoH 走 direct，正式域名拦截不改其路径；关闭后旧引用失效。
//! - geoip_ready_failure_atomicity_and_disabled_rules：MMDB 成功后才编译，损坏更新不替换旧方案，关闭移除国家规则。
//! - preferred_family_order_ip_reject_and_nxdomain：地址族按序，单 IP 拦截排除，NXDOMAIN/全部拦截不回退。
//! - ordered_upstreams_get_independent_deadlines：首个上游超时后第二个独享完整期限。
//!
//! 测试依据：用户已确认 DNS/GeoIP、地址族、单 IP 拦截及 DNS 出站忽略业务路由的契约。
//!
//! 前置条件：本地临时 UDP/TCP/TLS；不访问公网、不写真实用户缓存、不更改系统代理/VPN。
#[path = "test_fixture_https.rs"]
mod support;
use libfluxa_core::{
    dns::Query,
    error::Error,
    https::HttpsClient,
    outbound::{Family, Manager, RoutedTarget},
    routing::{Policy, Target, Transport},
};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{
    io::AsyncWriteExt,
    net::{TcpListener, UdpSocket},
};
fn client(root: rustls::pki_types::CertificateDer<'static>) -> HttpsClient {
    let mut roots = rustls::RootCertStore::empty();
    roots.add(root).unwrap();
    HttpsClient::with_roots(roots).unwrap()
}
fn query(name: &str) -> Query {
    Query {
        name: name.into(),
        query_type: "A".into(),
    }
}
fn request(name: &str, port: u16) -> RoutedTarget {
    RoutedTarget {
        target: Target {
            domain: Some(name.into()),
            address: None,
            port,
            transport: Transport::Tcp,
        },
        family: Family::Auto,
        timeout_ms: 3000,
        mapping_generation: None,
    }
}
#[tokio::test]
async fn explicit_doh_ignores_formal_route_and_retires_old_resolver() {
    let fixture = support::dns::fixture().await;
    let manager = Arc::new(Manager::default());
    let yaml = format!(
        "dns: {{enable: true, server: ['https://localhost:{}/dns-query,direct'],hosts: {{pinned.test: 192.0.2.1}}}}\nrules: ['domain,localhost,reject','final,direct']",
        fixture.doh_port
    );
    manager
        .apply_configuration_with_client(&yaml, client(fixture.root.clone()))
        .await
        .unwrap();
    let old = manager
        .network_snapshot()
        .unwrap()
        .resolver
        .clone()
        .unwrap();
    assert_eq!(
        old.resolve(query("connect.example.test"))
            .await
            .unwrap()
            .addresses,
        ["127.0.0.1"]
    );
    assert_eq!(
        old.resolve(query("pinned.test")).await.unwrap().addresses,
        ["192.0.2.1"]
    );
    manager.apply_configuration_with_client("dns: {enable: false, server: ['invalid'],hosts: {pinned.test: invalid}}\nip-strategy: only-ipv4\nrules: ['final,direct']",client(fixture.root.clone())).await.unwrap();
    assert!(manager.network_snapshot().unwrap().resolver.is_none());
    assert!(matches!(
        old.resolve(query("pinned.test")).await,
        Err(Error::InvalidState)
    ));
    manager.stop().await.unwrap();
}
#[tokio::test]
async fn geoip_ready_failure_atomicity_and_disabled_rules() {
    // 持久化测试在独立进程指定 HOME，不改变并行测试或真实用户目录。
    if std::env::var_os("FLUXA_GEOIP_ISOLATED_CHILD").is_none() {
        let path = std::env::temp_dir().join(format!(
            "fluxa-geoip-contract-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&path).unwrap();
        let result = std::process::Command::new(std::env::current_exe().unwrap())
            .arg("geoip_ready_failure_atomicity_and_disabled_rules")
            .arg("--exact")
            .arg("--test-threads=1")
            .env("HOME", &path)
            .env("FLUXA_GEOIP_ISOLATED_CHILD", "1")
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(path.join(".fluxa/rules/geoip.mmdb").is_file());
        std::fs::remove_dir_all(path).unwrap();
        return;
    }
    let fixture = support::fixture().await;
    let manager = Arc::new(Manager::default());
    let yaml = format!(
        "geoip: {{enabled: true,url: 'https://localhost:{}/geoip'}}\nip-strategy: only-ipv4\nrules: ['geoip,US,reject','final,direct']",
        fixture.port
    );
    let revision = manager
        .apply_configuration_with_client(&yaml, client(fixture.root.clone()))
        .await
        .unwrap();
    let refreshed = manager.refresh_geoip().await.unwrap();
    assert_ne!(refreshed, revision);
    let revision = refreshed;
    assert_eq!(
        manager
            .lookup_geoip("8.8.8.8", "en")
            .unwrap()
            .location
            .unwrap()
            .country_code,
        "US"
    );
    assert!(
        manager
            .lookup_geoip("192.0.2.1", "en")
            .unwrap()
            .location
            .is_none()
    );
    let target = Target {
        domain: None,
        address: Some("8.8.8.8".into()),
        port: 443,
        transport: Transport::Tcp,
    };
    assert_eq!(
        manager
            .network_snapshot()
            .unwrap()
            .router
            .as_ref()
            .unwrap()
            .evaluate(target.clone())
            .unwrap()
            .policy,
        Policy::Reject
    );
    assert!(matches!(
        manager
            .apply_configuration_with_client(
                &yaml.replace("/geoip'", "/geoip-corrupt'"),
                client(fixture.root.clone())
            )
            .await,
        Err(Error::GeoipInvalidDatabase)
    ));
    assert_eq!(manager.network_snapshot().unwrap().revision, revision);
    assert!(manager.provider_status().unwrap().error_info.is_some());
    // 真正重新下载当前有效来源并提交后，才可清除失败状态；不能靠本地编辑冒充恢复。
    manager.refresh_geoip().await.unwrap();
    assert!(manager.provider_status().unwrap().error_info.is_none());
    assert_eq!(
        manager
            .network_snapshot()
            .unwrap()
            .router
            .as_ref()
            .unwrap()
            .evaluate(target.clone())
            .unwrap()
            .policy,
        Policy::Reject
    );
    manager
        .apply_configuration_with_client(
            "geoip: {enabled: false,url: invalid}\nrules: ['geoip,US,reject','final,direct']",
            client(fixture.root.clone()),
        )
        .await
        .unwrap();
    assert_eq!(
        manager
            .network_snapshot()
            .unwrap()
            .router
            .as_ref()
            .unwrap()
            .evaluate(target)
            .unwrap()
            .policy,
        Policy::Direct
    );
    manager.stop().await.unwrap();
}
async fn dns_server() -> (u16, Arc<Mutex<Vec<String>>>, tokio::task::JoinHandle<()>) {
    use hickory_resolver::proto::{
        op::{Message, ResponseCode},
        rr::{
            RData, Record, RecordType,
            rdata::{A, AAAA},
        },
    };
    let socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let port = socket.local_addr().unwrap().port();
    let log = Arc::new(Mutex::new(Vec::new()));
    let own = log.clone();
    let task = tokio::spawn(async move {
        let mut bytes = [0; 4096];
        loop {
            let (len, peer) = socket.recv_from(&mut bytes).await.unwrap();
            let request = Message::from_vec(&bytes[..len]).unwrap();
            let q = request.queries[0].clone();
            let name = q.name.to_ascii();
            own.lock()
                .unwrap()
                .push(format!("{}:{}", q.query_type, name));
            if name.starts_with("slow.") {
                tokio::time::sleep(Duration::from_millis(650)).await;
            }
            let mut reply = Message::response(request.metadata.id, request.metadata.op_code);
            reply.metadata.recursion_available = true;
            reply.queries = request.queries;
            if name.starts_with("nx.") {
                reply.metadata.response_code = ResponseCode::NXDomain;
            } else if q.query_type == RecordType::A {
                for ip in [A::new(127, 0, 0, 2), A::new(127, 0, 0, 1)] {
                    reply.add_answer(Record::from_rdata(q.name.clone(), 0, RData::A(ip)));
                }
            } else if q.query_type == RecordType::AAAA && name.starts_with("blocked.") {
                reply.add_answer(Record::from_rdata(
                    q.name.clone(),
                    0,
                    RData::AAAA(AAAA("::1".parse().unwrap())),
                ));
            }
            socket
                .send_to(&reply.to_vec().unwrap(), peer)
                .await
                .unwrap();
        }
    });
    (port, log, task)
}
#[tokio::test]
async fn preferred_family_order_ip_reject_and_nxdomain() {
    let (dns, log, task) = dns_server().await;
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let accept = tokio::spawn(async move {
        loop {
            let (mut stream, _) = listener.accept().await.unwrap();
            tokio::spawn(async move {
                let _ = stream.write_all(b"ok").await;
                tokio::time::sleep(Duration::from_secs(10)).await;
            });
        }
    });
    let manager = Arc::new(Manager::default());
    let yaml = format!(
        "dns: {{enable: true,server: ['127.0.0.1:{dns}']}}\nip-strategy: prefer-ipv6\nrules: ['ip-cidr,127.0.0.2/32,reject','ip-cidr,::1/128,reject','final,direct']"
    );
    manager.apply_configuration_async(&yaml).await.unwrap();
    let opened = manager
        .connect_configured(request("available.test", port))
        .await
        .unwrap();
    assert!(opened.connection.address.starts_with("127.0.0.1:"));
    manager.close(opened.connection.id).await.unwrap();
    assert_eq!(
        *log.lock().unwrap(),
        ["AAAA:available.test.", "A:available.test."]
    );
    log.lock().unwrap().clear();
    assert!(matches!(
        manager.connect_configured(request("nx.test", port)).await,
        Err(Error::DnsResponse)
    ));
    assert_eq!(*log.lock().unwrap(), ["AAAA:nx.test."]);
    log.lock().unwrap().clear();
    assert!(matches!(
        manager
            .connect_configured(request("blocked.test", port))
            .await,
        Err(Error::Rejected)
    ));
    assert_eq!(*log.lock().unwrap(), ["AAAA:blocked.test."]);
    manager.stop().await.unwrap();
    task.abort();
    accept.abort();
}
#[tokio::test]
async fn ordered_upstreams_get_independent_deadlines() {
    let blackhole = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let first = blackhole.local_addr().unwrap().port();
    let (second, log, task) = dns_server().await;
    let manager = Arc::new(Manager::default());
    let yaml = format!(
        "dns: {{enable: true,timeout: 1,server: ['127.0.0.1:{first}','127.0.0.1:{second}']}}\nrules: ['final,direct']"
    );
    manager.apply_configuration_async(&yaml).await.unwrap();
    let resolver = manager
        .network_snapshot()
        .unwrap()
        .resolver
        .clone()
        .unwrap();
    let start = std::time::Instant::now();
    let answer = resolver.resolve(query("slow.test")).await.unwrap();
    assert_eq!(answer.addresses, ["127.0.0.2", "127.0.0.1"]);
    assert!(start.elapsed() >= Duration::from_millis(1500));
    assert!(!log.lock().unwrap().is_empty());
    assert!(log.lock().unwrap().iter().all(|q| q == "A:slow.test."));
    manager.stop().await.unwrap();
    task.abort();
}

#[tokio::test]
async fn named_numeric_dns_failure_never_uses_direct() {
    let fixture = support::dns::fixture().await;
    let closed = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = closed.local_addr().unwrap().port();
    drop(closed);
    let manager = Arc::new(Manager::default());
    let yaml = format!(
        r#"
proxies:
 - {{name: PROXY,type: vless,server: 127.0.0.1,port: {port},uuid: '00000000-0000-0000-0000-000000000000',flow: xtls-rprx-vision,security: reality,sni: example.com,pbk: AQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQE,sid: '01'}}
dns: {{enable: true,timeout: 1,server: ['127.0.0.1:{},PROXY']}}
rules: ['final,direct']
"#,
        fixture.port
    );
    manager.apply_configuration_async(&yaml).await.unwrap();
    let resolver = manager
        .network_snapshot()
        .unwrap()
        .resolver
        .clone()
        .unwrap();
    assert!(
        resolver
            .resolve(query("connect.example.test"))
            .await
            .is_err()
    );
    assert_eq!(
        fixture.count.load(std::sync::atomic::Ordering::Relaxed),
        0,
        "代理失败不得向数字 DNS 直接发送报文"
    );
    manager.stop().await.unwrap();
}

#[tokio::test]
async fn admitted_family_fallback_survives_configuration_replacement() {
    use hickory_resolver::proto::{
        op::Message,
        rr::{RData, Record, RecordType, rdata::A},
    };
    let socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let dns = socket.local_addr().unwrap().port();
    let (arrived, seen) = tokio::sync::oneshot::channel();
    let (release, wait) = tokio::sync::oneshot::channel();
    let task = tokio::spawn(async move {
        let mut bytes = [0; 4096];
        let mut gate = Some((arrived, wait));
        loop {
            let (len, peer) = socket.recv_from(&mut bytes).await.unwrap();
            let request = Message::from_vec(&bytes[..len]).unwrap();
            let q = request.queries[0].clone();
            if q.query_type == RecordType::AAAA
                && let Some((arrived, wait)) = gate.take()
            {
                arrived.send(()).unwrap();
                wait.await.unwrap();
            }
            let mut reply = Message::response(request.metadata.id, request.metadata.op_code);
            reply.metadata.recursion_available = true;
            reply.queries = request.queries;
            if q.query_type == RecordType::A {
                reply.add_answer(Record::from_rdata(
                    q.name,
                    0,
                    RData::A(A::new(127, 0, 0, 1)),
                ));
            }
            socket
                .send_to(&reply.to_vec().unwrap(), peer)
                .await
                .unwrap();
        }
    });
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        stream.write_all(b"old").await.unwrap();
        tokio::time::sleep(Duration::from_secs(5)).await;
    });
    let manager = Arc::new(Manager::default());
    manager.apply_configuration_async(&format!("dns: {{enable: true,server: ['127.0.0.1:{dns}']}}\nip-strategy: prefer-ipv6\nrules: ['final,direct']")).await.unwrap();
    let old = manager
        .network_snapshot()
        .unwrap()
        .resolver
        .clone()
        .unwrap();
    let own = manager.clone();
    let pending =
        tokio::spawn(async move { own.connect_configured(request("switch.test", port)).await });
    tokio::time::timeout(Duration::from_secs(2), seen)
        .await
        .unwrap()
        .unwrap();
    manager
        .apply_configuration_async("rules: ['final,reject']")
        .await
        .unwrap();
    assert!(matches!(
        old.resolve(query("switch.test")).await,
        Err(Error::InvalidState)
    ));
    release.send(()).unwrap();
    let opened = tokio::time::timeout(Duration::from_secs(4), pending)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(
        &*manager
            .read(opened.connection.id, 3, 1000)
            .await
            .unwrap()
            .bytes,
        b"old"
    );
    assert!(matches!(
        manager
            .connect_configured(request("switch.test", port))
            .await,
        Err(Error::Rejected)
    ));
    manager.stop().await.unwrap();
    task.abort();
    server.abort();
}
