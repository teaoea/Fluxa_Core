//! 测试文件：tests/test_platform_debug_identity.rs
//! 测试类型：单元测试、迁移回归
//! 测试对象：src/platform/debug_identity.rs
//! 测试内容：
//! - private_seed_rejects_wrong_permissions_symlinks_and_wrong_size：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//!
//! 测试依据：保留原测试的业务/格式/授权/资源约束；《测试文件与报告统一管理原则》，独立语义审查不以实现输出反推预期。
//!
//! 前置条件：本地测试需文件/回环套接字权限；平台限定及忽略项以实际登记为准。
//!
//! 执行状态：由同名 Markdown 报告记录，不在源码中固化通过结论。

#[cfg(all(test, unix))]
mod file_tests {
    use super::super::*;
    #[test]
    fn private_seed_rejects_wrong_permissions_symlinks_and_wrong_size() {
        use std::os::unix::fs::{PermissionsExt, symlink};
        let directory = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "fluxa-debug-file-{}-{}",
            std::process::id(),
            crate::lifecycle::identifier().unwrap()
        ));
        std::fs::create_dir(&directory).unwrap();
        std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700)).unwrap();
        let file = directory.join("seed");
        std::fs::write(&file, [3; 32]).unwrap();
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600)).unwrap();
        assert_eq!(read_credential(&file).unwrap().len(), 32);
        let alias = directory.join("alias");
        symlink(&file, &alias).unwrap();
        assert!(read_credential(&alias).is_err());
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert!(matches!(
            read_credential(&file),
            Err(Error::DebugIdentityRejected)
        ));
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600)).unwrap();
        std::fs::write(&file, [3; 31]).unwrap();
        assert!(matches!(
            read_credential(&file),
            Err(Error::DebugAuthenticationFailed)
        ));
        std::fs::write(&file, [3; 33]).unwrap();
        assert!(matches!(read_credential(&file), Err(Error::ResourceLimit)));
        // 临时目录可被普通用户写入，不能充当 root 管理策略的信任链。
        assert!(matches!(
            read_policy(&file, 32768),
            Err(Error::DebugIdentityRejected)
        ));
        std::fs::remove_dir_all(directory).unwrap();
    }
}
