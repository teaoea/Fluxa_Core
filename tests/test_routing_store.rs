//! 测试文件：tests/test_routing_store.rs
//! 测试类型：单元测试、迁移回归
//! 测试对象：src/routing/store.rs
//! 测试内容：
//! - cache_owned_file_roundtrip_preserves_document：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - cache_write_rejects_nonempty_handle_without_truncation：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - cache_failed_exclusive_creation_does_not_remove_existing_file：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - cache_symlink_swap_never_truncates_or_overwrites_target：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - cache_regular_file_swap_is_rejected_by_identity：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - cache_failed_exclusive_creation_does_not_remove_symlink：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - cache_rejected_update_keeps_snapshot_old_file_and_symlink_target：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//!
//! 测试依据：保留原测试的业务/格式/授权/资源约束；《测试文件与报告统一管理原则》，独立语义审查不以实现输出反推预期。
//!
//! 前置条件：本地测试需文件/回环套接字权限；平台限定及忽略项以实际登记为准。
//!
//! 执行状态：由同名 Markdown 报告记录，不在源码中固化通过结论。

#[cfg(test)]
mod cache_path_tests {
    use super::super::*;
    use crate::routing::cache;

    struct Directory(PathBuf);
    impl Directory {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "fluxa-cache-path-{}-{}",
                std::process::id(),
                crate::lifecycle::identifier().unwrap()
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Directory {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }
    fn snapshot() -> Snapshot {
        Snapshot::generate(Config::default(), vec![], 1)
            .unwrap()
            .into_flat()
            .unwrap()
    }

    #[test]
    fn cache_owned_file_roundtrip_preserves_document() {
        let directory = Directory::new();
        let mut temporary = Temporary::create(directory.0.join("cache.tmp")).unwrap();
        let expected = snapshot();
        cache::write(&mut temporary.1, &expected).unwrap();
        temporary.validate().unwrap();
        assert_eq!(
            cache::load(&temporary.0).unwrap().document(),
            expected.document()
        );
    }

    #[test]
    fn cache_write_rejects_nonempty_handle_without_truncation() {
        let directory = Directory::new();
        let mut temporary = Temporary::create(directory.0.join("cache.tmp")).unwrap();
        use std::io::Write;
        temporary.1.write_all(b"original-content").unwrap();
        assert_eq!(
            cache::write(&mut temporary.1, &snapshot()).unwrap_err(),
            Error::InvalidInput
        );
        assert_eq!(fs::read(&temporary.0).unwrap(), b"original-content");
    }

    #[test]
    fn cache_failed_exclusive_creation_does_not_remove_existing_file() {
        let directory = Directory::new();
        let path = directory.0.join("existing.tmp");
        fs::write(&path, b"original-content").unwrap();
        assert!(Temporary::create(path.clone()).is_err());
        assert_eq!(fs::read(path).unwrap(), b"original-content");
    }

    #[cfg(unix)]
    #[test]
    fn cache_symlink_swap_never_truncates_or_overwrites_target() {
        use std::os::unix::fs::symlink;
        let directory = Directory::new();
        let victim = directory.0.join("victim.txt");
        fs::write(&victim, b"must-remain-unchanged").unwrap();
        let mut temporary = Temporary::create(directory.0.join("cache.tmp")).unwrap();
        // 确定性替换发生在安全创建之后、缓存写入之前，覆盖原漏洞的真实窗口。
        fs::remove_file(&temporary.0).unwrap();
        symlink(&victim, &temporary.0).unwrap();
        cache::write(&mut temporary.1, &snapshot()).unwrap();
        assert_eq!(fs::read(&victim).unwrap(), b"must-remain-unchanged");
        assert!(temporary.1.metadata().unwrap().len() > 0);
        assert_eq!(temporary.validate().unwrap_err(), Error::InvalidInput);
        assert!(cache::load(&temporary.0).is_err());
        drop(temporary);
        assert_eq!(fs::read(&victim).unwrap(), b"must-remain-unchanged");
    }

    #[cfg(unix)]
    #[test]
    fn cache_regular_file_swap_is_rejected_by_identity() {
        let directory = Directory::new();
        let mut temporary = Temporary::create(directory.0.join("cache.tmp")).unwrap();
        fs::remove_file(&temporary.0).unwrap();
        fs::write(&temporary.0, b"replacement").unwrap();
        cache::write(&mut temporary.1, &snapshot()).unwrap();
        assert_eq!(fs::read(&temporary.0).unwrap(), b"replacement");
        assert_eq!(temporary.validate().unwrap_err(), Error::InvalidInput);
    }

    #[cfg(unix)]
    #[test]
    fn cache_failed_exclusive_creation_does_not_remove_symlink() {
        use std::os::unix::fs::symlink;
        let directory = Directory::new();
        let victim = directory.0.join("victim.txt");
        let path = directory.0.join("existing.tmp");
        fs::write(&victim, b"original-content").unwrap();
        symlink(&victim, &path).unwrap();
        assert!(Temporary::create(path.clone()).is_err());
        assert!(fs::symlink_metadata(path).unwrap().file_type().is_symlink());
        assert_eq!(fs::read(victim).unwrap(), b"original-content");
    }

    #[cfg(unix)]
    #[test]
    fn cache_rejected_update_keeps_snapshot_old_file_and_symlink_target() {
        use std::os::unix::fs::symlink;
        let directory = Directory::new();
        let store = Store::open(&directory.0).unwrap();
        store.update(Config::default(), vec![]).unwrap();
        let before = store.snapshot().unwrap();
        let path = directory.0.join("runtime-rules.bin");
        let saved = directory.0.join("saved.bin");
        let bytes = fs::read(&path).unwrap();
        fs::rename(&path, &saved).unwrap();
        let victim = directory.0.join("victim.txt");
        fs::write(&victim, b"must-remain-unchanged").unwrap();
        symlink(&victim, &path).unwrap();
        assert_eq!(
            store.update(Config::default(), vec![]).unwrap_err(),
            Error::InvalidInput
        );
        assert_eq!(store.snapshot().unwrap().document(), before.document());
        assert_eq!(fs::read(&saved).unwrap(), bytes);
        assert_eq!(fs::read(&victim).unwrap(), b"must-remain-unchanged");
        assert!(!fs::read_dir(&directory.0).unwrap().any(|entry| {
            entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .ends_with(".tmp")
        }));
        fs::remove_file(&path).unwrap();
        fs::rename(saved, path).unwrap();
        store.close().unwrap();
        let reopened = Store::open(&directory.0).unwrap();
        assert_eq!(reopened.snapshot().unwrap().document(), before.document());
        reopened.close().unwrap();
    }
}
