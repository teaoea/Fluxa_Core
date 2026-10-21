//! 测试文件：tests/test_routing_store.rs
//! 测试类型：单元测试、只读缓存和内存规则回归。
//! 测试对象：src/routing/store.rs、src/routing/cache.rs。
//! 测试内容：host_export_roundtrip_is_read_only、memory_updates_never_persist、invalid_file_is_not_repaired、symlink_target_is_never_modified、cancelled_job_cannot_publish。
//! 测试依据：Core 不保存缓存、不建目录；宿主保存内存编码；损坏和链接文件必须拒绝。
//! 前置条件：独立临时目录由测试宿主创建、保存和清理。
use super::*;
use std::path::PathBuf;
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "fluxa-store-{}",
            crate::lifecycle::identifier().unwrap()
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}
#[test]
fn host_export_roundtrip_is_read_only() {
    let fixture = Fixture::new();
    let path = fixture.0.join("host.bin");
    let store = Store::memory();
    store.update(Config::default(), vec![]).unwrap();
    let expected = store.snapshot().unwrap();
    let encoded = expected.to_binary().unwrap();
    std::fs::write(&path, encoded.as_ref()).unwrap();
    let before = std::fs::metadata(&path).unwrap().modified().unwrap();
    let loaded = Store::open(&path).unwrap();
    assert_eq!(loaded.snapshot().unwrap().document(), expected.document());
    loaded.close().unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), encoded.as_ref());
    assert_eq!(
        std::fs::metadata(&path).unwrap().modified().unwrap(),
        before
    );
    assert_eq!(std::fs::read_dir(&fixture.0).unwrap().count(), 1);
    store.close().unwrap();
}
#[test]
fn memory_updates_never_persist() {
    let fixture = Fixture::new();
    let store = Store::memory();
    assert!(store.snapshot().is_err());
    let result = store.update(Config::default(), vec![]).unwrap();
    assert_eq!(result.version, 1);
    assert!(!result.directory_synced);
    assert_eq!(result.timings.write_sync, Duration::ZERO);
    let held = store.snapshot().unwrap();
    assert_eq!(store.update(Config::default(), vec![]).unwrap().version, 2);
    assert_eq!(held.document().version, 1);
    assert_eq!(std::fs::read_dir(&fixture.0).unwrap().count(), 0);
    store.close().unwrap();
    assert!(store.snapshot().is_err());
    assert_eq!(held.document().version, 1);
}
#[test]
fn invalid_file_is_not_repaired() {
    let fixture = Fixture::new();
    let path = fixture.0.join("broken.bin");
    std::fs::write(&path, b"must-remain-unchanged").unwrap();
    assert!(Store::open(&path).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), b"must-remain-unchanged");
    assert!(Store::open(&fixture.0).is_err());
    assert!(Store::open(fixture.0.join("missing.bin")).is_err());
    assert_eq!(std::fs::read_dir(&fixture.0).unwrap().count(), 1);
}
#[cfg(unix)]
#[test]
fn symlink_target_is_never_modified() {
    let fixture = Fixture::new();
    let target = fixture.0.join("target");
    let link = fixture.0.join("link");
    std::fs::write(&target, b"host-owned").unwrap();
    std::os::unix::fs::symlink(&target, &link).unwrap();
    assert!(Store::open(&link).is_err());
    assert_eq!(std::fs::read(&target).unwrap(), b"host-owned");
    assert!(
        std::fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink()
    );
}
#[test]
fn cancelled_job_cannot_publish() {
    let store = Store::memory();
    store.update(Config::default(), vec![]).unwrap();
    let before = store.snapshot().unwrap();
    let manager = crate::outbound::Manager::default();
    let job = store.begin_job().unwrap();
    store.cancel_job(job).unwrap();
    assert!(matches!(
        store.update_job(Config::default(), vec![], &manager, job),
        Err(Error::Cancelled)
    ));
    assert_eq!(store.snapshot().unwrap().document(), before.document());
    store.finish_job(job).unwrap();
    store.close().unwrap();
}
