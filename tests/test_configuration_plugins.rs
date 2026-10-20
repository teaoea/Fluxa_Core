//! recovery_journal_cannot_name_external_paths：验证受控资源事务恢复。
//!
//! interrupted_resource_transaction_recovers_single_swap_and_accepts_complete_pair：验证受控资源事务恢复。
//!
//! automatic_refresh_requires_declared_local_slot_and_does_not_repeat_same_slot：验证同名业务契约。
//!
//! disabled_new_entry_and_read_only_reuse_never_download_or_write：验证同名业务契约。
//!
//! - only_strict_plugin_fields_and_single_script_mapping_are_allowed：验证同名业务契约。
//! - main_mitm_bypass_and_master_switch_override_plugin_decryption：验证同名业务契约。
//! - imports_namespaced_scripts_and_main_rewrites_win：验证同名业务契约。
//! - reordering_moves_ids_files_and_priority_without_redownload：验证同名业务契约。
//! - invalid_plugin_or_abandoned_batch_preserves_old_resources：验证同名业务契约。
//! - missing_script_excludes_only_bad_rewrite_and_keeps_native_rules：验证同名业务契约。
//!
//! 测试文件：tests/test_configuration_plugins.rs
//! 测试类型：单元测试、真实 HTTPS、目录事务和业务优先级集成测试。
//! 测试对象：src/configuration/plugins.rs、src/configuration/rewrites/execution.rs、src/script/worker.rs。
//! 测试内容：见各同名测试；覆盖严格插件格式、单对象命名引用、优先级、换序、回滚及脚本隔离。
//! 测试依据：用户 2026-10-20 插件/主配置优先、脚本存储和执行契约。
//! 前置条件：内部私有临时目录及本机自签根 TLS 夹具；不冒充系统 App Group，不改变 VPN/系统信任。
use super::*;
use crate::configuration::rewrites::execution::{Message, Outcome};
use serde_json::json;
use std::borrow::Cow;
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "fluxa-plugin-{}-{}",
            std::process::id(),
            crate::lifecycle::identifier().unwrap()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn document() -> Arc<Document> {
    Arc::new(Document::from_yaml("rules: ['final,direct']").unwrap())
}
fn request() -> Message<'static> {
    Message {
        method: Cow::Borrowed("GET"),
        url: Cow::Borrowed("https://ads.example/a"),
        headers: Cow::Borrowed(&[]),
        body: Cow::Borrowed(&[]),
    }
}
fn client(f: &crate::test_fixture_https::Fixture) -> crate::https::HttpsClient {
    let mut roots = rustls::RootCertStore::empty();
    roots.add(f.root.clone()).unwrap();
    let manager = Arc::new(crate::outbound::Manager::default());
    manager
        .apply_configuration("rules: ['final,direct']")
        .unwrap();
    crate::https::HttpsClient::with_roots(roots)
        .unwrap()
        .with_network(manager)
}
fn configured(
    f: &crate::test_fixture_https::Fixture,
    order: &[&str],
    main_rules: serde_json::Value,
) -> Arc<Document> {
    let providers=order.iter().map(|name|json!({"name":name,"category":"test","url":format!("https://localhost:{}/provider-{name}",f.port),"enabled":true})).collect::<Vec<_>>();
    Arc::new(
        Document::from_yaml(
            &json!({"rules":["final,direct"],"plugin-providers":providers,"rewrites":main_rules})
                .to_string(),
        )
        .unwrap(),
    )
}
fn plugin_body(f: &crate::test_fixture_https::Fixture, name: &str, status: u16) -> String {
    format!(
        "mitm: {{enabled: true, hosts: ['ads.example']}}\nrewrites:\n  - {{url: 'https://ads.example/**', action: reject, status: {status}}}\n  - {{url: 'https://data.example/**', phase: response, action: script, js: response}}\nscripts:\n  name: response\n  url: https://localhost:{}/provider-script-{name}.js\n",
        f.port
    )
}
#[test]
fn only_strict_plugin_fields_and_single_script_mapping_are_allowed() {
    let doc = document();
    assert!(
        payload(
            "mitm: {}\nrewrites: []\nscripts: {name: response, url: 'https://example.com/file.js'}",
            &doc
        )
        .is_ok()
    );
    for text in [
        "scripts: {url: 'https://example.com/a.js'}",
        "scripts: [{name: response}]",
        "scripts: null",
        "scripts: {name: response, name: duplicate}",
        "rules: ['final,direct']",
        "scripts: {name: '../escape'}",
        "mitm: {skip-server-cert-verify: true}",
        "rewrites: [{url: 'https://a.example/', action: 'proxy(secret)'}]",
    ] {
        assert!(payload(text, &doc).is_err(), "accepted {text}");
    }
}
#[test]
fn main_mitm_bypass_and_master_switch_override_plugin_decryption() {
    let main=Document::from_yaml("rules: ['final,reject']\nmitm: {enabled: true, hosts: [main.example], bypass: ['*.blocked.example']}").unwrap();
    let (p, memory) = payload(
        "mitm: {enabled: true, hosts: ['*.blocked.example', '*.plugin.example']}",
        &main,
    )
    .unwrap();
    let bundle = Bundle {
        slots: Default::default(),
        plugins: vec![Plugin {
            id: 1,
            rules: p.rewrites,
            bindings: Arc::default(),
            mitm: p.mitm,
            _memory: memory,
        }],
    };
    assert!(bundle.decrypts(&main, "main.example"));
    assert!(bundle.decrypts(&main, "api.plugin.example"));
    assert!(!bundle.decrypts(&main, "blocked.example"));
    assert!(!bundle.decrypts(&main, "a.blocked.example"));
    assert!(!bundle.decrypts(&document(), "api.plugin.example"));
}
#[tokio::test]
async fn imports_namespaced_scripts_and_main_rewrites_win() {
    let f = crate::test_fixture_https::fixture().await;
    let root = Fixture::new();
    {
        let mut bodies = f.provider.bodies.lock().unwrap();
        for (name, status) in [("a", 401), ("b", 402)] {
            bodies.insert(format!("/provider-{name}"), plugin_body(&f, name, status));
            bodies.insert(
                format!("/provider-script-{name}.js"),
                format!("() => ({{body: '{name}'}})"),
            );
        }
    }
    let doc = configured(
        &f,
        &["a", "b"],
        json!([{ "url":"https://ads.example/**","action":"reject","status":403}]),
    );
    let mut batch = load_in(&doc, &client(&f), &root.0, false).await.unwrap();
    assert_eq!(
        batch
            .bundle
            .plugins
            .iter()
            .map(|p| p.id)
            .collect::<Vec<_>>(),
        vec![1, 2]
    );
    let main = Bindings::default();
    let req = request();
    let mut out = req.clone();
    assert!(
        matches!(doc.rewrite_request_bound(&req,&mut out,Some(&main),&batch.bundle).unwrap(),Outcome::Response(v) if v.status==403)
    );
    batch.commit().unwrap();
    batch.accept();
    for (index, name) in [(1, "a"), (2, "b")] {
        assert!(
            fs::read_to_string(root.0.join(format!("plugin/{index}/response")))
                .unwrap()
                .contains(name)
        );
        let binding = batch.bundle.plugins[index - 1]
            .bindings
            .get("response", 3600)
            .unwrap();
        assert_eq!(
            binding.path,
            root.0.join(format!("plugin/{index}/response"))
        );
        let cache = Arc::new(crate::script::worker::Cache::default());
        assert_eq!(
            cache.execute(binding, json!({})).await.unwrap(),
            json!({"body":name})
        );
        cache.close().unwrap();
    }
}
#[tokio::test]
async fn reordering_moves_ids_files_and_priority_without_redownload() {
    use std::sync::atomic::Ordering;
    let f = crate::test_fixture_https::fixture().await;
    let root = Fixture::new();
    {
        let mut bodies = f.provider.bodies.lock().unwrap();
        for (name, status) in [("a", 401), ("b", 402)] {
            bodies.insert(format!("/provider-{name}"), plugin_body(&f, name, status));
            bodies.insert(
                format!("/provider-script-{name}.js"),
                format!("() => '{name}'"),
            );
        }
    }
    let doc = configured(&f, &["a", "b"], json!([]));
    let mut first = load_in(&doc, &client(&f), &root.0, false).await.unwrap();
    first.commit().unwrap();
    first.accept();
    let old = first.bundle.clone();
    drop(first);
    assert_eq!(f.provider.requests.load(Ordering::SeqCst), 4);
    drop(Batch::new(&root.0).expect("重启时应能安全复制已发布资源"));
    catalog(&root.0, false).expect("已发布目录的插件目录表应能严格回读");
    let doc = configured(&f, &["b", "a"], json!([]));
    let mut second = load_in(&doc, &client(&f), &root.0, false).await.unwrap();
    let req = request();
    let mut out = req.clone();
    assert!(
        matches!(doc.rewrite_request_bound(&req,&mut out,Some(&Bindings::default()),&second.bundle).unwrap(),Outcome::Response(v) if v.status==402)
    );
    second.commit().unwrap();
    second.accept();
    drop(second);
    assert_eq!(f.provider.requests.load(Ordering::SeqCst), 4);
    assert!(
        fs::read_to_string(root.0.join("plugin/1/response"))
            .unwrap()
            .contains("'b'")
    );
    assert!(
        fs::read_to_string(root.0.join("plugin/2/response"))
            .unwrap()
            .contains("'a'")
    );
    // 目录交换清理后，已开始旧快照仍可懒加载原插件，不能被换序重新解释为 b。
    let cache = Arc::new(crate::script::worker::Cache::default());
    assert_eq!(
        cache
            .execute(
                old.plugins[0].bindings.get("response", 3600).unwrap(),
                json!(null)
            )
            .await
            .unwrap(),
        json!("a")
    );
    cache.close().unwrap();
}
#[tokio::test]
async fn invalid_plugin_or_abandoned_batch_preserves_old_resources() {
    let f = crate::test_fixture_https::fixture().await;
    let root = Fixture::new();
    f.provider.bodies.lock().unwrap().insert(
        "/provider-a".into(),
        "rewrites: [{url: 'https://ads.example/**', action: reject, status: 403}]".into(),
    );
    let doc = configured(&f, &["a"], json!([]));
    let mut first = load_in(&doc, &client(&f), &root.0, false).await.unwrap();
    first.commit().unwrap();
    first.accept();
    drop(first);
    let original = fs::read(root.0.join("plugin/1/.plugin.yaml")).unwrap();
    f.provider
        .bodies
        .lock()
        .unwrap()
        .insert("/provider-a".into(), "unknown: true".into());
    assert!(load_in(&doc, &client(&f), &root.0, true).await.is_err());
    assert_eq!(
        fs::read(root.0.join("plugin/1/.plugin.yaml")).unwrap(),
        original
    );
    f.provider
        .bodies
        .lock()
        .unwrap()
        .insert("/provider-a".into(), "rewrites: []".into());
    {
        let mut candidate = load_in(&doc, &client(&f), &root.0, true).await.unwrap();
        candidate.commit().unwrap();
        candidate.rollback().unwrap();
    }
    assert_eq!(
        fs::read(root.0.join("plugin/1/.plugin.yaml")).unwrap(),
        original
    );
}
#[tokio::test]
async fn missing_script_excludes_only_bad_rewrite_and_keeps_native_rules() {
    let f = crate::test_fixture_https::fixture().await;
    let root = Fixture::new();
    f.provider.bodies.lock().unwrap().insert("/provider-a".into(),"scripts: {name: response}\nrewrites:\n - {url: 'https://ads.example/**', action: script, phase: request, js: response}\n - {url: 'https://ads.example/**', action: reject, status: 418}".into());
    let doc = configured(&f, &["a"], json!([]));
    let batch = load_in(&doc, &client(&f), &root.0, false).await.unwrap();
    assert!(!batch.bundle.plugins[0].bindings.contains("response", 3600));
    let req = request();
    let mut out = req.clone();
    assert!(
        matches!(doc.rewrite_request_bound(&req,&mut out,Some(&Bindings::default()),&batch.bundle).unwrap(),Outcome::Response(v) if v.status==418)
    );
}

#[tokio::test]
async fn disabled_new_entry_and_read_only_reuse_never_download_or_write() {
    use std::sync::atomic::Ordering;
    let f = crate::test_fixture_https::fixture().await;
    let root = Fixture::new();
    let yaml = json!({"rules":["final,direct"],"plugin-providers":[{"name":"off","category":"test","url":format!("https://localhost:{}/provider-off",f.port),"enabled":false}]});
    let doc = Arc::new(Document::from_yaml(&yaml.to_string()).unwrap());
    let mut batch = load_in(&doc, &client(&f), &root.0, false).await.unwrap();
    batch.commit().unwrap();
    batch.accept();
    drop(batch);
    assert_eq!(f.provider.requests.load(Ordering::SeqCst), 0);
    assert!(catalog(&root.0, false).is_ok());
    let before = fs::read(root.0.join("plugin/.catalog.json")).unwrap();
    let assets = read_only(&doc, &root.0).unwrap();
    assert!(assets.bundle.plugins.is_empty());
    assert_eq!(
        fs::read(root.0.join("plugin/.catalog.json")).unwrap(),
        before
    );
    assert_eq!(f.provider.requests.load(Ordering::SeqCst), 0);
}

#[test]
fn automatic_refresh_requires_declared_local_slot_and_does_not_repeat_same_slot() {
    let configuration=Document::from_yaml("rules: ['final,direct']\nplugin-providers: [{name: daily, category: test, url: 'https://example.com/plugin.yaml', enabled: true, update-time: '00:00'}]").unwrap();
    let at = crate::providers::now().unwrap();
    let slot = crate::providers::slot(Some("00:00"), at).unwrap();
    let mut seen = std::collections::BTreeMap::new();
    assert!(configuration.assets_due(&seen, at).unwrap());
    seen.insert("daily".into(), slot);
    assert!(!configuration.assets_due(&seen, at).unwrap());
    let no_schedule=Document::from_yaml("rules: ['final,direct']\nplugin-providers: [{name: once, category: test, url: 'https://example.com/plugin.yaml', enabled: true}]").unwrap();
    assert!(!no_schedule.assets_due(&Default::default(), at).unwrap());
}

#[test]
fn interrupted_resource_transaction_recovers_single_swap_and_accepts_complete_pair() {
    for complete in [false, true] {
        let root = Fixture::new();
        let generation = ".assets-stage-0123456789abcdef0123456789abcdef";
        Directory::open(&root.0, ".assets-lock", true).unwrap();
        let _lock = Directory::open(&root.0, ".assets-lock", false)
            .unwrap()
            .lock()
            .unwrap();
        for child in ["plugin", "js"] {
            let target = Directory::open(&root.0, child, true).unwrap();
            let candidate =
                Directory::open(&root.0, &format!("{generation}.{child}"), true).unwrap();
            let target_new = child == "plugin" || complete;
            target
                .write(
                    ".generation",
                    if target_new {
                        generation.as_bytes()
                    } else {
                        b"old-generation"
                    },
                    false,
                )
                .unwrap();
            target
                .write("value", if target_new { b"new" } else { b"old" }, false)
                .unwrap();
            candidate
                .write("value", if target_new { b"old" } else { b"new" }, false)
                .unwrap();
            candidate
                .write(
                    ".generation",
                    if target_new {
                        b"old-generation"
                    } else {
                        generation.as_bytes()
                    },
                    false,
                )
                .unwrap();
        }
        Directory::open(&root.0, ".assets-lock", false)
            .unwrap()
            .write(
                "transaction.json",
                &serde_json::to_vec(&Transaction {
                    stage: generation.into(),
                })
                .unwrap(),
                false,
            )
            .unwrap();
        recover(&root.0).unwrap();
        for child in ["plugin", "js"] {
            assert_eq!(
                fs::read(root.0.join(child).join("value")).unwrap(),
                if complete { b"new" } else { b"old" }
            );
            assert!(!root.0.join(format!("{generation}.{child}")).exists());
        }
        assert!(!root.0.join(".assets-lock/transaction.json").exists());
    }
}
#[test]
fn recovery_journal_cannot_name_external_paths() {
    let root = Fixture::new();
    let directory = Directory::open(&root.0, ".assets-lock", true).unwrap();
    let _lock = directory.lock().unwrap();
    directory
        .write("transaction.json", br#"{"stage":"../escape"}"#, false)
        .unwrap();
    assert!(matches!(recover(&root.0), Err(Error::InvalidInput)));
}
