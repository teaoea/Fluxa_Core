//! named_script_modifies_real_tls_request_and_conditional_response：验证同名业务契约。
//!
//! 迁移的 DNS/GeoIP 契约用例：
//! - admitted_family_fallback_survives_configuration_replacement：保留原独立业务断言，以私有根目录替代 HOME。
//! - named_numeric_dns_failure_never_uses_direct：保留原独立业务断言，以私有根目录替代 HOME。
//! - explicit_doh_ignores_formal_route_and_retires_old_resolver：保留原独立业务断言，以私有根目录替代 HOME。
//! - geoip_ready_failure_atomicity_and_disabled_rules：保留原独立业务断言，以私有根目录替代 HOME。
//! - preferred_family_order_ip_reject_and_nxdomain：保留原独立业务断言，以私有根目录替代 HOME。
//! - ordered_upstreams_get_independent_deadlines：保留原独立业务断言，以私有根目录替代 HOME。
//!
//! 测试文件：tests/test_outbound_configuration.rs
//! 测试类型：单元测试、原子事务、并发和预算生命周期
//! 测试对象：src/outbound/configuration.rs
//! 测试内容：
//! - concurrent_candidates_have_one_commit_winner：同基线并发候选只能提交一个，失败方不覆盖。
//! - failed_preparation_preserves_previous_configuration：非法和未接入能力均不得部分提交。
//! - stale_preparation_cannot_overwrite_newer_configuration：旧候选不得覆盖已提交新版本。
//! - stopped_manager_rejects_prepared_candidate：关闭优先于候选提交。
//! - replacement_releases_unreferenced_old_snapshot：不保留旧快照历史，外部旧引用仍保留真实预算。
//! - partial_updates_invalidate_summary_but_retain_budget：分模块变更撤销完整性声明且节点预算不提前释放。
//! - debug_overlay_survives_main_commit：配置提交不能撤销专用授权的调试规则。
//!
//! 测试依据：配置失败保留旧状态、新请求读取新快照、Debug 最高及内存实际占用预算约定。
//!
//! 前置条件：纯内存操作，无网络、VPN、文件或系统设置副作用。
use super::*;
use crate::{
    configuration::Document,
    routing::{Policy, Target, Transport},
};
fn target() -> Target {
    Target {
        domain: None,
        address: Some("192.0.2.1".into()),
        port: 443,
        transport: Transport::Tcp,
    }
}
fn policy(manager: &Manager) -> Policy {
    manager
        .network_snapshot()
        .unwrap()
        .router
        .as_ref()
        .unwrap()
        .evaluate(target())
        .unwrap()
        .policy
}
#[test]
fn failed_preparation_preserves_previous_configuration() {
    let manager = Manager::default();
    let revision = manager
        .apply_configuration("rules: ['final,reject']")
        .unwrap();
    for (yaml, expected) in [
        ("rules: ['final,unknown']", Error::InvalidInput),
        (
            "rules: ['final,direct']\ndns: {enable: true, server: ['system']}",
            Error::Unsupported,
        ),
        (
            "rules: ['final,direct']\ndns: {enable: true, server: ['1.1.1.1']}",
            Error::Unsupported,
        ),
        (
            "rules: ['final,direct']\nsystem-proxy: {port: 1054}",
            Error::Unsupported,
        ),
        (
            "rules: ['final,direct']\nlog-level: info",
            Error::Unsupported,
        ),
        (
            "rules: ['final,direct']\nip-strategy: prefer-ipv6",
            Error::Unsupported,
        ),
        (
            "rules: ['final,direct']\nrule-providers: [{name: remote, url: 'https://example.com/rules.yaml', policy: direct, enable: true}]",
            Error::Unsupported,
        ),
    ] {
        assert_eq!(manager.apply_configuration(yaml), Err(expected));
        assert_eq!(manager.configuration_status().unwrap().revision, revision);
        assert_eq!(policy(&manager), Policy::Reject);
    }
}
#[test]
fn stale_preparation_cannot_overwrite_newer_configuration() {
    let manager = Manager::default();
    let base = manager
        .apply_configuration("rules: ['final,reject']")
        .unwrap();
    let candidate = Document::from_yaml("rules: ['final,direct']")
        .unwrap()
        .prepare()
        .unwrap();
    let next = manager
        .apply_configuration("rules: ['final,reject']")
        .unwrap();
    assert_eq!(
        manager.commit_configuration(base, candidate),
        Err(Error::InvalidState)
    );
    assert_eq!(manager.configuration_status().unwrap().revision, next);
    assert_eq!(policy(&manager), Policy::Reject);
}
#[tokio::test]
async fn stopped_manager_rejects_prepared_candidate() {
    let manager = Manager::default();
    let base = manager
        .apply_configuration("rules: ['final,reject']")
        .unwrap();
    let candidate = Document::from_yaml("rules: ['final,direct']")
        .unwrap()
        .prepare()
        .unwrap();
    manager.stop().await.unwrap();
    assert_eq!(
        manager.commit_configuration(base, candidate),
        Err(Error::InvalidState)
    );
    assert_eq!(policy(&manager), Policy::Reject);
}
#[test]
fn replacement_releases_unreferenced_old_snapshot() {
    let manager = Manager::default();
    manager
        .apply_configuration("rules: ['final,reject']")
        .unwrap();
    let old = manager.network_snapshot().unwrap();
    let old_document = Arc::downgrade(old.configuration.as_ref().unwrap());
    let old_index = Arc::downgrade(old.router.as_ref().unwrap());
    manager
        .apply_configuration("rules: ['final,direct']")
        .unwrap();
    assert_eq!(policy(&manager), Policy::Direct);
    assert_eq!(
        old.router
            .as_ref()
            .unwrap()
            .evaluate(target())
            .unwrap()
            .policy,
        Policy::Reject
    );
    assert!(old_document.upgrade().is_some());
    drop(old);
    assert!(old_document.upgrade().is_none());
    assert!(old_index.upgrade().is_none());
}
#[test]
fn partial_updates_invalidate_summary_but_retain_budget() {
    let manager = Manager::default();
    manager
        .apply_configuration("rules: ['final,reject']")
        .unwrap();
    let document = Arc::downgrade(
        manager
            .network_snapshot()
            .unwrap()
            .configuration
            .as_ref()
            .unwrap(),
    );
    manager
        .configure_limits(Limits {
            maximum_connections: 1,
            ..Limits::default()
        })
        .unwrap();
    assert!(
        manager
            .configuration_status()
            .unwrap()
            .configuration
            .is_none()
    );
    assert!(
        document.upgrade().is_some(),
        "仍共享已准备对象的快照保留配置预算"
    );
    manager
        .configure_network(None, None, Limits::default())
        .unwrap();
    assert!(document.upgrade().is_none());
}
#[test]
fn debug_overlay_survives_main_commit() {
    let manager = Manager::default();
    manager
        .apply_configuration("rules: ['final,reject']")
        .unwrap();
    let session = Arc::new(crate::routing::debug::Session::new().unwrap());
    manager
        .replace_debug(
            session,
            vec![crate::routing::debug::DebugRule {
                // 调试对象的具体字段由现有公开规则模型构造，不能通过 YAML 注入。
                rule: crate::routing::Rule {
                    r#type: crate::routing::Match::Domain,
                    value: "example.com".into(),
                    policy: Policy::Direct,
                    outbound: None,
                    protocol: None,
                },
                protocol: None,
                lifetime_seconds: Some(60),
            }],
            None,
        )
        .unwrap();
    manager
        .apply_configuration("rules: ['final,reject']")
        .unwrap();
    let mut request = target();
    request.address = None;
    request.domain = Some("example.com".into());
    assert_eq!(
        manager
            .network_snapshot()
            .unwrap()
            .router
            .as_ref()
            .unwrap()
            .evaluate(request)
            .unwrap()
            .policy,
        Policy::Direct
    );
}

#[test]
fn concurrent_candidates_have_one_commit_winner() {
    let manager = Arc::new(Manager::default());
    let base = manager
        .apply_configuration("rules: ['final,reject']")
        .unwrap();
    let barrier = Arc::new(std::sync::Barrier::new(2));
    let workers: Vec<_> = ["rules: ['final,direct']", "rules: ['final,reject']"]
        .into_iter()
        .map(|yaml| {
            let candidate = Document::from_yaml(yaml).unwrap().prepare().unwrap();
            let manager = manager.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                manager.commit_configuration(base, candidate)
            })
        })
        .collect();
    let results: Vec<_> = workers
        .into_iter()
        .map(|worker| worker.join().unwrap())
        .collect();
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|result| **result == Err(Error::InvalidState))
            .count(),
        1
    );
    assert_eq!(
        manager.configuration_status().unwrap().revision,
        *results
            .iter()
            .find_map(|result| result.as_ref().ok())
            .unwrap()
    );
}

// 原跨模块测试迁至对应实现的单元测试，允许注入隔离目录而不扩大生产 API。
mod dns_geoip_contract {
    //! 测试文件：tests/test_outbound_configuration.rs（dns_geoip_contract 子模块）
    //! 测试类型：单元测试、跨模块独立配置与出站语义
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
    use crate::test_fixture_https as support;
    use fluxa_core::{
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

    /// 只在测试模块注入私有根目录；不增加生产 API，也不依赖 HOME 或真实共享容器。
    struct FixtureManager {
        inner: Arc<Manager>,
        root: std::path::PathBuf,
    }
    impl FixtureManager {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "fluxa-dns-storage-{}",
                crate::lifecycle::identifier().unwrap()
            ));
            std::fs::create_dir(&root).unwrap();
            Self {
                inner: Arc::new(Manager::default()),
                root,
            }
        }
        async fn apply_configuration_with_client(
            &self,
            yaml: &str,
            client: HttpsClient,
        ) -> Result<u64, Error> {
            let document = Arc::new(crate::configuration::Document::from_yaml(yaml)?);
            self.inner
                .apply_provider_document(document, client, Some(self.root.clone()))
                .await
        }
        async fn apply_configuration_async(&self, yaml: &str) -> Result<u64, Error> {
            self.apply_configuration_with_client(yaml, HttpsClient::system()?)
                .await
        }
    }
    impl std::ops::Deref for FixtureManager {
        type Target = Arc<Manager>;
        fn deref(&self) -> &Self::Target {
            &self.inner
        }
    }
    impl Drop for FixtureManager {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.root).unwrap();
        }
    }
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
        let manager = FixtureManager::new();
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
        let fixture = support::fixture().await;
        let manager = FixtureManager::new();
        let yaml = format!(
            "geoip: {{enabled: true,url: 'https://localhost:{}/geoip'}}\nip-strategy: only-ipv4\nrules: ['geoip,US,reject','final,direct']",
            fixture.port
        );
        let revision = manager
            .apply_configuration_with_client(&yaml, client(fixture.root.clone()))
            .await
            .unwrap();
        assert!(manager.root.join("rule/geoip.mmdb").is_file());
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
        let manager = FixtureManager::new();
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
        let manager = FixtureManager::new();
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
        let manager = FixtureManager::new();
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
        let manager = FixtureManager::new();
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
        // 新配置仍使用回环 DNS；省略 DNS 会按主配置契约启用系统解析，测试便依赖外部环境。
        // 保留 final/reject 及拒绝断言，真实候选解析完成后必须按新快照拒绝。
        manager
            .apply_configuration_async(&format!(
                "dns: {{enable: true,server: ['127.0.0.1:{dns}']}}\nrules: ['final,reject']"
            ))
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
}

#[tokio::test]
async fn named_script_modifies_real_tls_request_and_conditional_response() {
    // 私有候选夹具只注入绑定，不调用/冒充系统共享容器；执行走真实发布和 TLS 数据管线。
    let fixture = crate::test_fixture_https::fixture().await;
    let path = std::env::temp_dir().join(format!(
        "fluxa-js-wire-{}",
        crate::lifecycle::identifier().unwrap()
    ));
    std::fs::create_dir(&path).unwrap();
    let directory = crate::platform::storage::Directory::open(&path, "js", true).unwrap();
    directory.write("response",br#"input => input.status ? ({status:201,body: 'response:' + input.body}) : ({method:'POST',body:'request-body'})"#,false).unwrap();
    let url = format!("https://localhost:{}/echo", fixture.port);
    let doc=Arc::new(Document::from_yaml(&serde_json::json!({"rules":["final,direct"],"scripts":{"name":"response"},"rewrites":[
        {"url":url,"phase":"request","action":"script","js":"response"},
        {"url":url,"phase":"response","when":{"status":200,"content-type":"text/plain"},"action":"script","js":"response"}
    ]}).to_string()).unwrap());
    let bindings = crate::script::worker::Bindings::in_root(
        &path,
        true,
        Some("response"),
        vec![("response".into(), 3600)],
    )
    .unwrap();
    let mut prepared = Arc::new(Document::from_yaml("rules: ['final,direct']").unwrap())
        .prepare_shared(Vec::new())
        .unwrap();
    prepared.document = doc;
    prepared.scripts = bindings;
    let manager = Arc::new(Manager::default());
    let baseline = manager.network_snapshot().unwrap().revision;
    manager.commit_configuration(baseline, prepared).unwrap();
    let mut roots = rustls::RootCertStore::empty();
    roots.add(fixture.root.clone()).unwrap();
    let client = crate::https::HttpsClient::with_roots(roots)
        .unwrap()
        .with_network(manager.clone());
    let response = client
        .execute_request("GET", &url, &[], b"original", None)
        .await
        .unwrap();
    assert_eq!(response.status, 201);
    assert_eq!(response.body, b"response:request-body");
    assert!(
        !response
            .headers
            .iter()
            .any(|h| h.name.eq_ignore_ascii_case("content-length"))
    );
    manager.clear_script_cache().await.unwrap();
    directory
        .write(
            "response",
            b"() => {throw new Error('condition must skip JS');}",
            true,
        )
        .unwrap();
    let doc=Arc::new(Document::from_yaml(&serde_json::json!({"rules":["final,direct"],"scripts":{"name":"response"},"rewrites":[{"url":url,"phase":"response","when":{"status":403},"action":"script","js":"response"}]}).to_string()).unwrap());
    let mut prepared = Arc::new(Document::from_yaml("rules: ['final,direct']").unwrap())
        .prepare_shared(Vec::new())
        .unwrap();
    prepared.document = doc;
    prepared.scripts = crate::script::worker::Bindings::in_root(
        &path,
        true,
        Some("response"),
        vec![("response".into(), 3600)],
    )
    .unwrap();
    manager
        .commit_configuration(manager.network_snapshot().unwrap().revision, prepared)
        .unwrap();
    let skipped = client
        .execute_request("GET", &url, &[], b"untouched", None)
        .await
        .unwrap();
    assert_eq!(skipped.status, 200);
    assert_eq!(skipped.body, b"untouched");
    manager.stop().await.unwrap();
    assert!(matches!(
        client.execute_request("GET", &url, &[], &[], None).await,
        Err(Error::InvalidState)
    ));
    std::fs::remove_dir_all(path).unwrap();
}
