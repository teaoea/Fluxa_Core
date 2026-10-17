//! 测试文件：tests/test_providers.rs
//! 测试类型：单元测试、跨模块集成、真实 HTTPS 与缓存生命周期
//! 测试对象：src/providers.rs、configuration、outbound、https、规则快照
//! 测试内容：
//! - geoip_daily_slot_is_kernel_owned_and_idempotent：设备本地到期由内核更新，单个日期不重复。
//! - scheduler_outlives_request_runtime：外部请求运行时销毁后，内核计划和刷新继续有效。
//! - import_reuse_refresh_and_restart_keep_policy_and_atomic_cache：首次导入、复用、更新、重启与策略隔离。
//! - invalid_update_keeps_files_snapshot_and_live_connection：坏 YAML/下载错误保留旧目录、快照及已建立连接。
//! - close_cancels_download_and_releases_lease_with_old_arc_alive：并发关闭取消下载，旧引用不阻止新会话绑定。
//! - schedule_requires_local_slot_and_retry_without_caller_clock：本地日期补跑及失败退避由内核判断。
//!
//! - disabled_manual_and_policy_edits_reuse_body_without_rewriting：停用不下载、手动不启用、策略切换不重写文件。
//! - multi_provider_failure_never_partially_publishes：第二个源失败不替换任何旧正文。
//! - automatic_due_is_once_per_slot_and_errors_are_rate_limited：本地到期一次与失败退避。
//!
//! 测试依据：用户确认的提供者格式、策略、缓存路径、内核生命周期和失败原子性。
//! 前置条件：macOS/Linux 原子目录交换，本机 HTTPS/UDP；不修改真实 HOME 或系统代理。
use super::*;
static TEST_PROVIDER_GATE: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
use crate::routing::{Policy, Target, Transport};
fn home() -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "fluxa-provider-home-{}-{}",
        std::process::id(),
        crate::lifecycle::identifier().unwrap()
    ));
    std::fs::create_dir(&path).unwrap();
    path
}
fn yaml(port: u16, extra: &str) -> String {
    format!(
        "rule-providers:\n  - {{name: one, url: 'https://localhost:{port}/provider-one', policy: reject, enable: true}}\nrules: ['domain,localhost,direct', {extra}'final,direct']"
    )
}
fn client(f: &crate::test_fixture_https::Fixture) -> HttpsClient {
    let mut roots = rustls::RootCertStore::empty();
    roots.add(f.root.clone()).unwrap();
    HttpsClient::with_roots(roots).unwrap()
}
fn target(name: &str) -> Target {
    Target {
        domain: Some(name.into()),
        address: None,
        port: 443,
        transport: Transport::Tcp,
    }
}
fn policy(manager: &Manager, name: &str) -> Policy {
    manager
        .network_snapshot()
        .unwrap()
        .router
        .as_ref()
        .unwrap()
        .evaluate(target(name))
        .unwrap()
        .policy
}
#[tokio::test]
async fn import_reuse_refresh_and_restart_keep_policy_and_atomic_cache() {
    let _gate = TEST_PROVIDER_GATE.lock().await;
    use std::sync::atomic::Ordering;
    let fixture = crate::test_fixture_https::fixture().await;
    let home = home();
    let manager = Arc::new(Manager::default());
    fixture.provider.bodies.lock().unwrap().insert("/provider-one".into(),"rules: ['domain,ads.example', 'domain-suffix,too-wide.example', 'domain-keyword,track', 'domain,own.example,direct', 'protocol,quic', 'port,8080']".into());
    let config = yaml(fixture.port, "");
    manager
        .apply_provider_document(
            Arc::new(Document::from_yaml(&config).unwrap()),
            client(&fixture),
            Some(home.clone()),
        )
        .await
        .unwrap();
    assert_eq!(fixture.provider.requests.load(Ordering::SeqCst), 1);
    assert_eq!(policy(&manager, "ads.example"), Policy::Reject);
    assert_eq!(policy(&manager, "a.too-wide.example"), Policy::Direct);
    assert_eq!(policy(&manager, "own.example"), Policy::Direct);
    let status = manager.provider_status().unwrap();
    assert_eq!(status.providers[0].ignored, 3);
    assert_eq!(status.providers[0].rules, 3);
    let old = manager.network_snapshot().unwrap();
    let edited = yaml(fixture.port, "'domain,new.local,reject', ");
    manager
        .apply_provider_document(
            Arc::new(Document::from_yaml(&edited).unwrap()),
            client(&fixture),
            Some(home.clone()),
        )
        .await
        .unwrap();
    assert_eq!(
        fixture.provider.requests.load(Ordering::SeqCst),
        1,
        "本地修改不得重下未到期规则集"
    );
    assert_eq!(policy(&manager, "new.local"), Policy::Reject);
    assert_eq!(
        old.router
            .as_ref()
            .unwrap()
            .evaluate(target("new.local"))
            .unwrap()
            .policy,
        Policy::Direct
    );
    fixture.provider.bodies.lock().unwrap().insert(
        "/provider-one".into(),
        "rules: ['domain,new.remote']".into(),
    );
    // 只读状态与提交并发时不得反向持有 State/Authority 两把锁。
    let querying = manager.clone();
    let finished = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let signal = finished.clone();
    let reader = std::thread::spawn(move || {
        while !signal.load(Ordering::Acquire) {
            querying.provider_status().unwrap();
            std::thread::yield_now();
        }
    });
    tokio::time::timeout(
        Duration::from_secs(5),
        manager.refresh_providers(Some("one".into())),
    )
    .await
    .unwrap()
    .unwrap();
    finished.store(true, Ordering::Release);
    reader.join().unwrap();
    assert_eq!(policy(&manager, "new.remote"), Policy::Reject);
    assert_eq!(policy(&manager, "ads.example"), Policy::Direct);
    assert!(
        std::fs::read_to_string(home.join(".fluxa/rules/one.yaml"))
            .unwrap()
            .contains("new.remote")
    );
    let retained = manager.provider_runtime().unwrap().unwrap();
    manager.stop().await.unwrap();
    assert!(!retained.status().unwrap().active);
    let fresh = Arc::new(Manager::default());
    fresh
        .apply_provider_document(
            Arc::new(Document::from_yaml(&edited).unwrap()),
            client(&fixture),
            Some(home.clone()),
        )
        .await
        .unwrap();
    assert_eq!(
        fixture.provider.requests.load(Ordering::SeqCst),
        2,
        "无 update-time 重启复用缓存，不自动重下"
    );
    assert_eq!(policy(&fresh, "new.remote"), Policy::Reject);
    assert!(matches!(
        retained.refresh(None).await,
        Err(Error::InvalidState)
    ));
    fresh.stop().await.unwrap();
    drop(old);
    std::fs::remove_dir_all(home).unwrap();
}
#[tokio::test]
async fn invalid_update_keeps_files_snapshot_and_live_connection() {
    let _gate = TEST_PROVIDER_GATE.lock().await;
    let fixture = crate::test_fixture_https::fixture().await;
    let home = home();
    let manager = Arc::new(Manager::default());
    fixture.provider.bodies.lock().unwrap().insert(
        "/provider-one".into(),
        "rules: ['domain,old.example']".into(),
    );
    let config = yaml(fixture.port, "");
    manager
        .apply_provider_document(
            Arc::new(Document::from_yaml(&config).unwrap()),
            client(&fixture),
            Some(home.clone()),
        )
        .await
        .unwrap();
    let file = home.join(".fluxa/rules/one.yaml");
    let content = std::fs::read(&file).unwrap();
    let revision = manager.network_snapshot().unwrap().revision;
    let peer = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let opened = manager
        .connect_configured(crate::outbound::RoutedTarget {
            target: Target {
                domain: None,
                address: Some("127.0.0.1".into()),
                port: peer.local_addr().unwrap().port(),
                transport: Transport::Udp,
            },
            timeout_ms: 1000,
            family: crate::outbound::Family::Ipv4,
            mapping_generation: None,
        })
        .await
        .unwrap();
    fixture
        .provider
        .bodies
        .lock()
        .unwrap()
        .insert("/provider-one".into(), "rules: [broken\n".into());
    assert!(manager.refresh_providers(None).await.is_err());
    assert_eq!(manager.network_snapshot().unwrap().revision, revision);
    assert_eq!(std::fs::read(&file).unwrap(), content);
    assert_eq!(policy(&manager, "old.example"), Policy::Reject);
    assert!(manager.provider_status().unwrap().error_info.is_some());
    manager
        .write(opened.connection.id, b"unchanged", 1000)
        .await
        .unwrap();
    let mut buf = [0; 32];
    let (n, _) = tokio::time::timeout(Duration::from_secs(1), peer.recv_from(&mut buf))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(&buf[..n], b"unchanged");
    // 文件被外部改写时拒绝复用，不能把内存旧正文假装是当前有效磁盘缓存。
    std::fs::write(&file, b"rules: []").unwrap();
    assert!(
        manager
            .apply_provider_document(
                Arc::new(Document::from_yaml(&config).unwrap()),
                client(&fixture),
                Some(home.clone())
            )
            .await
            .is_err()
    );
    assert_eq!(manager.network_snapshot().unwrap().revision, revision);
    fixture.provider.bodies.lock().unwrap().insert(
        "/provider-one".into(),
        "rules: ['domain,old.example']".into(),
    );
    manager.refresh_providers(None).await.unwrap();
    assert_eq!(
        std::fs::read(&file).unwrap(),
        content,
        "明确手动刷新须修复磁盘，即使远端正文未变化"
    );
    assert_eq!(policy(&manager, "old.example"), Policy::Reject);
    manager.stop().await.unwrap();
    std::fs::remove_dir_all(home).unwrap();
}
#[tokio::test]
async fn close_cancels_download_and_releases_lease_with_old_arc_alive() {
    let _gate = TEST_PROVIDER_GATE.lock().await;
    use std::sync::atomic::Ordering;
    let fixture = crate::test_fixture_https::fixture().await;
    let home = home();
    let manager = Arc::new(Manager::default());
    fixture
        .provider
        .bodies
        .lock()
        .unwrap()
        .insert("/provider-one".into(), "rules: []".into());
    let config = yaml(fixture.port, "");
    manager
        .apply_provider_document(
            Arc::new(Document::from_yaml(&config).unwrap()),
            client(&fixture),
            Some(home.clone()),
        )
        .await
        .unwrap();
    fixture.provider.delay.store(true, Ordering::SeqCst);
    let old = manager.provider_runtime().unwrap().unwrap();
    let worker = manager.clone();
    let request = tokio::spawn(async move { worker.refresh_providers(None).await });
    tokio::time::timeout(Duration::from_secs(2), async {
        while fixture.provider.requests.load(Ordering::SeqCst) < 2 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    let (a, b) = tokio::join!(manager.stop(), manager.stop());
    a.unwrap();
    b.unwrap();
    assert!(request.await.unwrap().is_err());
    assert!(!old.status().unwrap().active);
    let (mut cache, _) = cache::Cache::open(&home).unwrap();
    cache.close().unwrap();
    assert!(matches!(old.refresh(None).await, Err(Error::InvalidState)));
    std::fs::remove_dir_all(home).unwrap();
}
#[test]
fn schedule_requires_local_slot_and_retry_without_caller_clock() {
    let now = now().unwrap();
    let day = crate::platform::providers::local_day(now).unwrap();
    assert_eq!(slot(None, now).unwrap(), None);
    assert_eq!(slot(Some("00:00"), now).unwrap(), Some(day.date));
    assert!(slot(Some("24:00"), now).is_err());
    assert_eq!(
        slot(Some("23:59"), now).unwrap(),
        Some(if day.seconds >= 86340 {
            day.date
        } else {
            day.previous
        })
    );
}

#[tokio::test]
async fn disabled_manual_and_policy_edits_reuse_body_without_rewriting() {
    let _gate = TEST_PROVIDER_GATE.lock().await;
    use std::os::unix::fs::MetadataExt;
    use std::sync::atomic::Ordering;
    let fixture = crate::test_fixture_https::fixture().await;
    let home = home();
    let manager = Arc::new(Manager::default());
    fixture.provider.bodies.lock().unwrap().insert(
        "/provider-one".into(),
        "rules: ['domain-suffix,wide.example', 'domain,ads.example']".into(),
    );
    let disabled = yaml(fixture.port, "").replace("enable: true", "enable: false");
    manager
        .apply_provider_document(
            Arc::new(Document::from_yaml(&disabled).unwrap()),
            client(&fixture),
            Some(home.clone()),
        )
        .await
        .unwrap();
    assert_eq!(fixture.provider.requests.load(Ordering::SeqCst), 0);
    manager.refresh_providers(None).await.unwrap();
    assert_eq!(fixture.provider.requests.load(Ordering::SeqCst), 1);
    assert_eq!(policy(&manager, "ads.example"), Policy::Direct);
    assert!(!manager.provider_status().unwrap().providers[0].enabled);
    let file = home.join(".fluxa/rules/one.yaml");
    let inode = std::fs::metadata(&file).unwrap().ino();
    let enabled = yaml(fixture.port, "");
    manager
        .apply_provider_document(
            Arc::new(Document::from_yaml(&enabled).unwrap()),
            client(&fixture),
            Some(home.clone()),
        )
        .await
        .unwrap();
    assert_eq!(manager.provider_status().unwrap().providers[0].ignored, 1);
    assert_eq!(policy(&manager, "ads.example"), Policy::Reject);
    let direct = enabled.replace("policy: reject", "policy: direct");
    manager
        .apply_provider_document(
            Arc::new(Document::from_yaml(&direct).unwrap()),
            client(&fixture),
            Some(home.clone()),
        )
        .await
        .unwrap();
    let status = manager.provider_status().unwrap();
    assert_eq!(status.providers[0].ignored, 0);
    assert_eq!(status.providers[0].rules, 2);
    assert_eq!(
        std::fs::metadata(&file).unwrap().ino(),
        inode,
        "只改元数据策略不能替换正文文件"
    );
    assert_eq!(fixture.provider.requests.load(Ordering::SeqCst), 1);
    manager.stop().await.unwrap();
    std::fs::remove_dir_all(home).unwrap();
}
#[tokio::test]
async fn multi_provider_failure_never_partially_publishes() {
    let _gate = TEST_PROVIDER_GATE.lock().await;
    let fixture = crate::test_fixture_https::fixture().await;
    let home = home();
    let manager = Arc::new(Manager::default());
    for name in ["one", "two"] {
        fixture.provider.bodies.lock().unwrap().insert(
            format!("/provider-{name}"),
            format!("rules: ['domain,{name}.old.example']"),
        );
    }
    let config = format!(
        "rule-providers:\n  - {{name: one, url: 'https://localhost:{0}/provider-one', policy: reject, enable: true}}\n  - {{name: two, url: 'https://localhost:{0}/provider-two', policy: reject, enable: true}}\nrules: ['domain,localhost,direct','final,direct']",
        fixture.port
    );
    manager
        .apply_provider_document(
            Arc::new(Document::from_yaml(&config).unwrap()),
            client(&fixture),
            Some(home.clone()),
        )
        .await
        .unwrap();
    let revision = manager.network_snapshot().unwrap().revision;
    let before: [Vec<u8>; 2] =
        ["one", "two"].map(|n| std::fs::read(home.join(format!(".fluxa/rules/{n}.yaml"))).unwrap());
    fixture.provider.bodies.lock().unwrap().insert(
        "/provider-one".into(),
        "rules: ['domain,one.new.example']".into(),
    );
    fixture
        .provider
        .bodies
        .lock()
        .unwrap()
        .insert("/provider-two".into(), "rules: [".into());
    assert!(manager.refresh_providers(None).await.is_err());
    assert_eq!(manager.network_snapshot().unwrap().revision, revision);
    for (i, name) in ["one", "two"].iter().enumerate() {
        assert_eq!(
            std::fs::read(home.join(format!(".fluxa/rules/{name}.yaml"))).unwrap(),
            before[i]
        );
        assert_eq!(
            policy(&manager, &format!("{name}.old.example")),
            Policy::Reject
        );
    }
    assert_eq!(policy(&manager, "one.new.example"), Policy::Direct);
    manager.stop().await.unwrap();
    std::fs::remove_dir_all(home).unwrap();
}
#[tokio::test]
async fn automatic_due_is_once_per_slot_and_errors_are_rate_limited() {
    let _gate = TEST_PROVIDER_GATE.lock().await;
    use std::sync::atomic::Ordering;
    let fixture = crate::test_fixture_https::fixture().await;
    let home = home();
    let manager = Arc::new(Manager::default());
    fixture.provider.bodies.lock().unwrap().insert(
        "/provider-one".into(),
        "rules: ['domain,old.example']".into(),
    );
    let config =
        yaml(fixture.port, "").replace("enable: true", "enable: true, update-time: '00:00'");
    manager
        .apply_provider_document(
            Arc::new(Document::from_yaml(&config).unwrap()),
            client(&fixture),
            Some(home.clone()),
        )
        .await
        .unwrap();
    let runtime = manager.provider_runtime().unwrap().unwrap();
    // 测试主动驱动 tick，停止真实轮询以免两个执行者争抢同一个到期任务。
    let worker = runtime.worker.lock().unwrap().take().unwrap();
    worker.abort();
    assert!(worker.await.unwrap_err().is_cancelled());
    // 使用私有控制门构造昨天已成功、今天未成功；公开 API 不接受宿主提供时间或计划状态。
    {
        let _gate = runtime.control.lock().await;
        let mut state = runtime.state.lock().unwrap();
        state.data.entries.get_mut("one").unwrap().0.last_slot = None;
        state.data.attempts.clear();
        state.last_cycle = None;
        let mut batch = state
            .cache
            .as_ref()
            .unwrap()
            .stage(state.data.clone())
            .unwrap();
        batch.commit().unwrap();
        batch.sync_parent().unwrap();
    }
    fixture
        .provider
        .bodies
        .lock()
        .unwrap()
        .insert("/provider-one".into(), "rules: [".into());
    assert!(runtime.automatic().await.is_err());
    let failed = fixture.provider.requests.load(Ordering::SeqCst);
    assert_eq!(failed, 2);
    runtime.automatic().await.unwrap();
    assert_eq!(
        fixture.provider.requests.load(Ordering::SeqCst),
        failed,
        "失败立即重试必须被内核退避阻止"
    );
    assert_eq!(policy(&manager, "old.example"), Policy::Reject);
    {
        let _gate = runtime.control.lock().await;
        let mut state = runtime.state.lock().unwrap();
        state.data.attempts.clear();
        state.last_cycle = None;
    }
    fixture.provider.bodies.lock().unwrap().insert(
        "/provider-one".into(),
        "rules: ['domain,new.example']".into(),
    );
    runtime.automatic().await.unwrap();
    assert_eq!(policy(&manager, "new.example"), Policy::Reject);
    assert!(manager.provider_status().unwrap().error_info.is_none());
    let count = fixture.provider.requests.load(Ordering::SeqCst);
    {
        runtime.state.lock().unwrap().last_cycle = None;
    }
    runtime.automatic().await.unwrap();
    assert_eq!(
        fixture.provider.requests.load(Ordering::SeqCst),
        count,
        "同一本地日期时刻只执行一次"
    );
    manager.stop().await.unwrap();
    std::fs::remove_dir_all(home).unwrap();
}

#[tokio::test]
async fn scheduler_outlives_request_runtime() {
    let _gate = TEST_PROVIDER_GATE.lock().await;
    let fixture = crate::test_fixture_https::fixture().await;
    let home = home();
    let manager = Arc::new(Manager::default());
    let worker = manager.clone();
    let directory = home.clone();
    let tls = client(&fixture);
    let document = Arc::new(
        Document::from_yaml(&yaml(fixture.port, "").replace("enable: true", "enable: false"))
            .unwrap(),
    );
    // 请求运行时只存在于该独立线程；它退出不代表内核控制器收到关闭意图。
    tokio::task::spawn_blocking(move || {
        std::thread::spawn(move || {
            let caller = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            caller
                .block_on(worker.apply_provider_document(document, tls, Some(directory)))
                .unwrap();
        })
        .join()
        .unwrap()
    })
    .await
    .unwrap();
    let runtime = manager.provider_runtime().unwrap().unwrap();
    assert!(
        !runtime
            .worker
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .is_finished(),
        "外部运行时销毁不得终止内核计划"
    );
    fixture.provider.bodies.lock().unwrap().insert(
        "/provider-one".into(),
        "rules: ['domain,after.example']".into(),
    );
    manager.refresh_providers(None).await.unwrap();
    assert_eq!(
        manager.provider_status().unwrap().providers[0].rules,
        0,
        "手动更新不能启用停用来源"
    );
    manager.stop().await.unwrap();
    std::fs::remove_dir_all(home).unwrap();
}

#[tokio::test]
async fn geoip_daily_slot_is_kernel_owned_and_idempotent() {
    let _gate = TEST_PROVIDER_GATE.lock().await;
    let fixture = crate::test_fixture_https::fixture().await;
    let manager = Arc::new(Manager::default());
    let home = home();
    let configuration=Arc::new(Document::from_yaml(&format!("geoip: {{enabled: true,url: 'https://localhost:{}/geoip',update-time: '00:00'}}\nrules: ['geoip,US,reject','final,direct']",fixture.port)).unwrap());
    manager
        .apply_provider_document(configuration, client(&fixture), Some(home.clone()))
        .await
        .unwrap();
    let runtime = manager.provider_runtime().unwrap().unwrap();
    // 停止真实时钟执行器后用内核当前本地日历驱动一次到期；不伪造业务时钟或来源。
    let worker = runtime.worker.lock().unwrap().take().unwrap();
    worker.abort();
    let _ = worker.await;
    let before = manager.network_snapshot().unwrap().revision;
    runtime.state.lock().unwrap().geoip_slot = None;
    let current = runtime.state.lock().unwrap().configuration.clone().unwrap();
    assert!(runtime.due(&current, now().unwrap()).unwrap());
    runtime.automatic().await.unwrap();
    let after = manager.network_snapshot().unwrap().revision;
    assert_ne!(before, after);
    runtime.automatic().await.unwrap();
    assert_eq!(manager.network_snapshot().unwrap().revision, after);
    manager.stop().await.unwrap();
    std::fs::remove_dir_all(home).unwrap();
}
