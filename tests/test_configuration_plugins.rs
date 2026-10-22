//! 测试文件：tests/test_configuration_plugins.rs
//! 测试类型：单元测试、只读文件及真实 JS 执行测试。
//! 测试对象：src/configuration/plugins.rs、src/script/worker.rs。
//! 测试内容：only_strict_plugin_fields_and_single_script_mapping_are_allowed、main_mitm_bypass_and_master_switch_override_plugin_decryption、host_files_preserve_priority_and_script_namespaces、invalid_plugin_preserves_host_files、missing_script_excludes_only_script_rule。
//! 测试依据：主配置优先；宿主授权资源；Core 不下载和落盘；缺失 JS 排除对应复写。
//! 前置条件：测试宿主准备临时文件，不访问网络或真实共享容器。
use super::*;
use crate::configuration::rewrites::execution::{Message, Outcome};
use crate::storage::{NamedFile, PluginFiles, Resources};
use serde_json::json;
use std::{borrow::Cow, path::PathBuf};
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "nodus-plugin-{}",
            crate::lifecycle::identifier().unwrap()
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn write(&self, name: &str, text: &str) -> PathBuf {
        let path = self.0.join(name);
        std::fs::write(&path, text).unwrap();
        path
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
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
fn configured(main: bool) -> Arc<Document> {
    Arc::new(Document::from_yaml(&json!({"rules":["final,direct"],"plugin-providers":[{"name":"a","category":"test","url":"https://example.com/a","enabled":true},{"name":"b","category":"test","url":"https://example.com/b","enabled":true}],"rewrites":if main {json!([{"url":"https://ads.example/**","action":"reject","status":403}])} else {json!([])}}).to_string()).unwrap())
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
async fn host_files_preserve_priority_and_script_namespaces() {
    let root = Fixture::new();
    let mut files = Resources::default();
    for (id, name, status) in [(1, "a", 401), (2, "b", 402)] {
        let path=root.write(&format!("{name}.yaml"),&format!("rewrites: [{{url: 'https://ads.example/**', action: reject, status: {status}}}, {{url: 'https://data.example/**', phase: response, action: script, js: response}}]\nscripts: {{name: response}}"));
        let script = root.write(&format!("{name}.js"), &format!("() => '{name}'"));
        files.plugins.push(PluginFiles {
            name: name.into(),
            id,
            path,
            script: Some(NamedFile {
                name: "response".into(),
                path: script,
            }),
        });
    }
    let before: Vec<_> = files
        .plugins
        .iter()
        .map(|p| std::fs::read(&p.path).unwrap())
        .collect();
    let main = configured(true);
    let assets = load_local(&main, &files).unwrap();
    let req = request();
    let mut output = req.clone();
    assert!(
        matches!(main.rewrite_request_bound(&req,&mut output,Some(&assets.scripts),&assets.bundle).unwrap(),Outcome::Response(v) if v.status==403)
    );
    let plain = configured(false);
    let assets = load_local(&plain, &files).unwrap();
    assert!(
        matches!(plain.rewrite_request_bound(&req,&mut output,Some(&assets.scripts),&assets.bundle).unwrap(),Outcome::Response(v) if v.status==401)
    );
    assert_eq!(
        assets
            .bundle
            .plugins
            .iter()
            .map(|p| p.id)
            .collect::<Vec<_>>(),
        vec![1, 2]
    );
    for (index, name) in ["a", "b"].iter().enumerate() {
        let cache = Arc::new(crate::script::worker::Cache::default());
        let binding = assets.bundle.plugins[index]
            .bindings
            .get("response", 3600)
            .unwrap();
        assert_eq!(
            cache.execute(binding, json!({})).await.unwrap(),
            json!(name)
        );
        cache.close().unwrap();
        assert_eq!(
            std::fs::read(&files.plugins[index].path).unwrap(),
            before[index]
        );
    }
    assert_eq!(std::fs::read_dir(&root.0).unwrap().count(), 4);
    // 顺序/编号由宿主显式提供；Core 不重命名文件。
    files.plugins.swap(0, 1);
    files.plugins[0].id = 1;
    files.plugins[1].id = 2;
    let reordered = load_local(&plain, &files).unwrap();
    assert_eq!(
        reordered
            .bundle
            .plugins
            .iter()
            .map(|p| p.id)
            .collect::<Vec<_>>(),
        vec![1, 2]
    );
    assert!(
        matches!(plain.rewrite_request_bound(&req,&mut output,Some(&reordered.scripts),&reordered.bundle).unwrap(),Outcome::Response(v) if v.status==402)
    );
}
#[test]
fn invalid_plugin_preserves_host_files() {
    let root = Fixture::new();
    let path = root.write("a.yaml", "rules: ['final,direct']");
    let b = root.write("b.yaml", "rewrites: []");
    let resources = Resources {
        plugins: vec![
            PluginFiles {
                name: "a".into(),
                id: 1,
                path: path.clone(),
                script: None,
            },
            PluginFiles {
                name: "b".into(),
                id: 2,
                path: b,
                script: None,
            },
        ],
        ..Default::default()
    };
    assert!(load_local(&configured(false), &resources).is_err());
    assert_eq!(
        std::fs::read_to_string(path).unwrap(),
        "rules: ['final,direct']"
    );
    assert_eq!(std::fs::read_dir(&root.0).unwrap().count(), 2);
}
#[test]
fn missing_script_excludes_only_script_rule() {
    let root = Fixture::new();
    let a=root.write("a.yaml","rewrites: [{url: 'https://ads.example/**', action: reject, status: 404}, {url: 'https://data.example/**', phase: response, action: script, js: response}]\nscripts: {name: response}");
    let b = root.write("b.yaml", "rewrites: []");
    let resources = Resources {
        plugins: vec![
            PluginFiles {
                name: "a".into(),
                id: 1,
                path: a,
                script: Some(NamedFile {
                    name: "response".into(),
                    path: root.0.join("missing.js"),
                }),
            },
            PluginFiles {
                name: "b".into(),
                id: 2,
                path: b,
                script: None,
            },
        ],
        ..Default::default()
    };
    let doc = configured(false);
    let assets = load_local(&doc, &resources).unwrap();
    assert!(!assets.bundle.plugins[0].bindings.contains("response", 3600));
    let req = request();
    let mut output = req.clone();
    assert!(
        matches!(doc.rewrite_request_bound(&req,&mut output,Some(&assets.scripts),&assets.bundle).unwrap(),Outcome::Response(v) if v.status==404)
    );
    assert!(!root.0.join("missing.js").exists());
}
