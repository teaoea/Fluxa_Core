//! 测试文件：tests/test_providers.rs
//! 测试类型：单元测试、跨模块只读文件及生命周期测试。
//! 测试对象：src/providers.rs、configuration、outbound。
//! 测试内容：host_files_are_applied_without_download_or_write、invalid_replacement_keeps_live_snapshot、missing_resource_does_not_create_files、close_releases_controller_with_old_reference_alive。
//! 测试依据：宿主下载和持久化；Core 严格只读；失败保留有效运行配置；关闭不依赖 Arc。
//! 前置条件：临时文件由测试宿主准备，不访问外网或系统 VPN。
use super::*;
use crate::{
    routing::{Policy, Target, Transport},
    storage::NamedFile,
};
use std::path::PathBuf;
static TEST_PROVIDER_GATE: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "nodus-providers-{}",
            crate::lifecycle::identifier().unwrap()
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn files(&self) -> Resources {
        Resources {
            rules: vec![NamedFile {
                name: "one".into(),
                path: self.0.join("one.yaml"),
            }],
            ..Default::default()
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}
fn doc() -> Arc<Document> {
    Arc::new(Document::from_yaml("rule-providers: [{name: one, url: 'https://127.0.0.1:1/never-download', policy: reject, enable: true}]\nrules: ['final,direct']").unwrap())
}
fn policy(manager: &Manager, name: &str) -> Policy {
    manager
        .network_snapshot()
        .unwrap()
        .router
        .as_ref()
        .unwrap()
        .evaluate(Target {
            domain: Some(name.into()),
            address: None,
            port: 443,
            transport: Transport::Tcp,
        })
        .unwrap()
        .policy
}
#[tokio::test]
async fn host_files_are_applied_without_download_or_write() {
    let _gate = TEST_PROVIDER_GATE.lock().await;
    let fixture = Fixture::new();
    let path = fixture.0.join("one.yaml");
    let text = "rules: ['domain,ads.example', 'domain,own.example,direct']";
    std::fs::write(&path, text).unwrap();
    let manager = Arc::new(Manager::default());
    manager
        .apply_provider_document(
            doc(),
            HttpsClient::with_roots(rustls::RootCertStore::empty()).unwrap(),
            fixture.files(),
        )
        .await
        .unwrap();
    assert_eq!(policy(&manager, "ads.example"), Policy::Reject);
    assert_eq!(policy(&manager, "own.example"), Policy::Direct);
    let status = manager.provider_status().unwrap();
    assert!(status.read_only);
    assert_eq!(status.providers[0].rules, 1);
    assert_eq!(status.providers[0].ignored, 1);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), text);
    assert_eq!(std::fs::read_dir(&fixture.0).unwrap().count(), 1);
    assert!(matches!(
        manager.refresh_providers(None).await,
        Err(Error::Unsupported)
    ));
    manager.stop().await.unwrap();
}
#[tokio::test]
async fn invalid_replacement_keeps_live_snapshot() {
    let _gate = TEST_PROVIDER_GATE.lock().await;
    let fixture = Fixture::new();
    let path = fixture.0.join("one.yaml");
    std::fs::write(&path, "rules: ['domain,ads.example']").unwrap();
    let manager = Arc::new(Manager::default());
    manager
        .apply_provider_document(
            doc(),
            HttpsClient::with_roots(rustls::RootCertStore::empty()).unwrap(),
            fixture.files(),
        )
        .await
        .unwrap();
    let old = manager.network_snapshot().unwrap();
    let revision = old.revision;
    std::fs::write(&path, "unknown: true").unwrap();
    assert!(
        manager
            .apply_provider_document(
                doc(),
                HttpsClient::with_roots(rustls::RootCertStore::empty()).unwrap(),
                fixture.files()
            )
            .await
            .is_err()
    );
    assert_eq!(manager.network_snapshot().unwrap().revision, revision);
    assert_eq!(policy(&manager, "ads.example"), Policy::Reject);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "unknown: true");
    manager.stop().await.unwrap();
}
#[tokio::test]
async fn missing_resource_does_not_create_files() {
    let _gate = TEST_PROVIDER_GATE.lock().await;
    let fixture = Fixture::new();
    let manager = Arc::new(Manager::default());
    let revision = manager.network_snapshot().unwrap().revision;
    assert!(
        manager
            .apply_provider_document(
                doc(),
                HttpsClient::with_roots(rustls::RootCertStore::empty()).unwrap(),
                fixture.files()
            )
            .await
            .is_err()
    );
    assert_eq!(manager.network_snapshot().unwrap().revision, revision);
    assert_eq!(std::fs::read_dir(&fixture.0).unwrap().count(), 0);
    manager.stop().await.unwrap();
}
#[tokio::test]
async fn close_releases_controller_with_old_reference_alive() {
    let _gate = TEST_PROVIDER_GATE.lock().await;
    let manager = Arc::new(Manager::default());
    let runtime = Runtime::new(
        &manager,
        HttpsClient::with_roots(rustls::RootCertStore::empty()).unwrap(),
    )
    .unwrap();
    runtime.close().await.unwrap();
    assert!(matches!(
        runtime.apply_files(doc(), Resources::default()).await,
        Err(Error::InvalidState) | Err(Error::Cancelled)
    ));
    assert!(!runtime.status().unwrap().active);
    let replacement = Runtime::new(
        &manager,
        HttpsClient::with_roots(rustls::RootCertStore::empty()).unwrap(),
    )
    .unwrap();
    replacement.close().await.unwrap();
}
