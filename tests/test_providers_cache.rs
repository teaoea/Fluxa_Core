//! 测试文件：tests/test_providers_cache.rs
//! 测试类型：单元测试、磁盘原子性与恢复
//! 测试对象：src/providers/cache.rs
//! 测试内容：
//! - cache_batch_is_atomic_and_old_references_do_not_own_lock：整批目录交换、哈希校验与显式释放租约。
//! - corruption_version_and_symlink_are_rejected：损坏、未知版本、符号链接与无来源缓存拒绝；恢复尝试记录合并有界。
//!
//! 测试依据：完整验证后发布、来源真实性、真实租约与引用计数隔离。
//! 前置条件：macOS/Linux 原子目录交换，私有临时目录。
use super::*;
fn home() -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "fluxa-cache-{}-{}",
        std::process::id(),
        crate::lifecycle::identifier().unwrap()
    ));
    fs::create_dir(&p).unwrap();
    p
}
#[test]
fn cache_batch_is_atomic_and_old_references_do_not_own_lock() {
    let home = self::home();
    let (mut cache, data) = Cache::open(&home).unwrap();
    assert!(data.entries.is_empty());
    assert!(matches!(Cache::open(&home), Err(Error::InvalidState)));
    let mut data = Data::default();
    let text = Text::new("rules: ['domain,one.example']".into()).unwrap();
    data.entries.insert(
        "one".into(),
        (
            Metadata {
                url_hash: hash(b"https://one"),
                digest: hash(text.text.as_bytes()),
                fetched_at: 1,
                last_slot: None,
            },
            text,
        ),
    );
    let mut batch = cache.stage(data.clone()).unwrap();
    assert!(!cache.directory.join("one.yaml").exists());
    batch.commit().unwrap();
    batch.sync_parent().unwrap();
    cache.verify(&data).unwrap();
    drop(batch);
    cache.close().unwrap();
    let (mut second, loaded) = Cache::open(&home).unwrap();
    assert_eq!(loaded.entries["one"].1.text, data.entries["one"].1.text);
    assert!(cache.stage(data).is_err());
    second.close().unwrap();
    fs::remove_dir_all(home).unwrap();
}
#[test]
fn corruption_version_and_symlink_are_rejected() {
    let home = self::home();
    let (mut cache, _) = Cache::open(&home).unwrap();
    let mut batch = cache.stage(Data::default()).unwrap();
    batch.commit().unwrap();
    drop(batch);
    cache.close().unwrap();
    fs::write(
        home.join(".fluxa/rules/.state.json"),
        b"{\"version\":99,\"entries\":{},\"attempts\":[]}",
    )
    .unwrap();
    assert!(matches!(Cache::open(&home), Err(Error::Unsupported)));
    fs::remove_dir_all(home).unwrap();
    // 多次失败会更新独立日志而不发布旧清单；两份合法有界记录的并集不能导致永久恢复失败。
    let home = self::home();
    let (mut cache, _) = Cache::open(&home).unwrap();
    let mut data = Data::default();
    for i in 0..MAX_ATTEMPTS {
        data.attempts
            .insert(hash(format!("old-{i}").as_bytes()), i as u64);
    }
    let mut batch = cache.stage(data).unwrap();
    batch.commit().unwrap();
    drop(batch);
    let attempts = (0..MAX_ATTEMPTS)
        .map(|i| (hash(format!("new-{i}").as_bytes()), 1000 + i as u64))
        .collect();
    cache.attempts(&attempts).unwrap();
    cache.close().unwrap();
    let (mut cache, restored) = Cache::open(&home).unwrap();
    assert_eq!(restored.attempts.len(), MAX_ATTEMPTS);
    assert!(restored.attempts.values().all(|time| *time >= 1000));
    cache.close().unwrap();
    let key = hash(b"duplicate");
    let bad = serde_json::json!([{"key":key,"time":1},{"key":key,"time":2}]);
    fs::write(
        home.join(".fluxa/.provider-attempts.json"),
        serde_json::to_vec(&bad).unwrap(),
    )
    .unwrap();
    assert!(
        matches!(Cache::open(&home), Err(Error::InvalidInput)),
        "重复尝试记录不能静默覆盖"
    );
    fs::remove_dir_all(home).unwrap();
    #[cfg(unix)]
    {
        let home = self::home();
        let (mut cache, _) = Cache::open(&home).unwrap();
        cache.close().unwrap();
        std::os::unix::fs::symlink("/tmp", home.join(".fluxa/rules/link.yaml")).unwrap();
        assert!(Cache::open(&home).is_err());
        fs::remove_dir_all(home).unwrap();
    }
}
