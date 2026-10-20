//! 测试文件：tests/test_platform_storage.rs
//! 测试类型：单元测试、安全回归测试
//! 测试对象：src/platform/storage.rs
//! 测试内容：signed_team_namespace 验证团队绑定；reject_invalid_team 拒绝非团队输入。
//! 测试依据：Apple macOS 团队前缀共享组规范，不允许配置、路径或旧 group 前缀替代签名团队。
//! 前置条件：纯函数测试不需要授权；不能替代真实签名宿主测试。
use super::*;

#[test]
fn signed_team_namespace() {
    assert_eq!(
        macos_group("DVXRRLRJ78").unwrap(),
        "DVXRRLRJ78.app.fluxa.shared"
    );
    assert_eq!(
        macos_group("ABCDEFGHIJ").unwrap(),
        "ABCDEFGHIJ.app.fluxa.shared"
    );
}

#[test]
fn reject_invalid_team() {
    for value in [
        "",
        "group",
        "group.app.fluxa.ios",
        "DVXRRLRJ7",
        "DVXRRLRJ780",
        "dvxrrlrj78",
        "../../tmp/",
        "DVXRRLRJ7\0",
        "ＡBCDEFGHIJ",
    ] {
        assert!(
            matches!(macos_group(value), Err(Error::InvalidState)),
            "{value:?}"
        );
    }
}
