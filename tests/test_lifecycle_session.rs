//! 测试文件：tests/test_lifecycle_session.rs
//! 测试类型：单元测试、迁移回归
//! 测试对象：src/lifecycle/session.rs
//! 测试内容：
//! - failed_commit_preserves_admission：外部提交失败不撤销旧权限，成功后才撤销。
//! - retained_objects_do_not_restore_revoked_rights_and_completion_waits_for_operations：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - invalid_identity_and_failed_cleanup_are_not_success：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//!
//! 测试依据：保留原测试的业务/格式/授权/资源约束；《测试文件与报告统一管理原则》，独立语义审查不以实现输出反推预期。
//!
//! 前置条件：本地测试需文件/回环套接字权限；平台限定及忽略项以实际登记为准。
//!
//! 执行状态：由同名 Markdown 报告记录，不在源码中固化通过结论。

#[cfg(test)]
mod tests {
    use super::super::*;
    #[tokio::test]
    async fn retained_objects_do_not_restore_revoked_rights_and_completion_waits_for_operations() {
        let owner = Authority::new().unwrap();
        let retained = owner.clone();
        let operation = owner.enter(owner.identity()).unwrap();
        assert!(owner.revoke().unwrap());
        assert!(!retained.revoke().unwrap());
        assert!(matches!(
            retained.enter(retained.identity()),
            Err(Error::InvalidState)
        ));
        assert!(matches!(owner.complete(Ok(())), Err(Error::InvalidState)));
        assert_eq!(owner.phase().unwrap(), Phase::Closing);
        drop(operation);
        owner.wait_idle().await.unwrap();
        owner.complete(Ok(())).unwrap();
        retained.wait_closed().await.unwrap();
        assert_eq!(retained.phase().unwrap(), Phase::Closed);
        assert!(matches!(
            retained.check(retained.identity()),
            Err(Error::InvalidState)
        ));
    }
    #[tokio::test]
    async fn invalid_identity_and_failed_cleanup_are_not_success() {
        let owner = Authority::new().unwrap();
        let other = Authority::new().unwrap();
        assert!(matches!(
            owner.enter(other.identity()),
            Err(Error::InvalidState)
        ));
        owner.revoke().unwrap();
        owner.canceled().await;
        owner.complete(Err(Error::Internal)).unwrap();
        assert!(matches!(owner.wait_closed().await, Err(Error::Internal)));
        assert_eq!(owner.phase().unwrap(), Phase::Failed);
    }
}

#[test]
fn failed_commit_preserves_admission() {
    use super::*;
    let owner = Authority::new().unwrap();
    assert_eq!(
        owner.revoke_after(|| Err(Error::InvalidInput)),
        Err(Error::InvalidInput)
    );
    assert!(owner.check(owner.identity()).is_ok());
    assert_eq!(owner.revoke_after(|| Ok(())), Ok(true));
    assert_eq!(owner.check(owner.identity()), Err(Error::InvalidState));
    owner.complete(Ok(())).unwrap();
}
