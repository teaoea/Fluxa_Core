//! 测试文件：tests/test_platform_providers.rs
//! 测试类型：单元测试、真实系统边界
//! 测试对象：src/platform/providers.rs
//! 测试内容：
//! - shared_container_requires_real_entitlement：无共享组签名权限时拒绝，不回退到 HOME。
//! - exchange_swaps_whole_directories_without_missing_target：真实同级目录交换。
//! - local_calendar_uses_valid_civil_dates_and_bounded_time：设备日历与无效时间边界。
//!
//! 测试依据：单次原子发布与设备本地时区；不把 UTC 日秒数当成本地日历。
//! 前置条件：macOS/Linux 文件系统交换能力；设备系统日历。
use super::*;
#[test]
fn exchange_swaps_whole_directories_without_missing_target() {
    let parent = std::env::temp_dir().join(format!(
        "fluxa-swap-{}",
        crate::lifecycle::identifier().unwrap()
    ));
    std::fs::create_dir(&parent).unwrap();
    let a = parent.join("a");
    let b = parent.join("b");
    std::fs::create_dir(&a).unwrap();
    std::fs::create_dir(&b).unwrap();
    std::fs::write(a.join("a"), b"old").unwrap();
    std::fs::write(b.join("b"), b"new").unwrap();
    exchange(&a, &b).unwrap();
    assert_eq!(std::fs::read(a.join("b")).unwrap(), b"new");
    assert_eq!(std::fs::read(b.join("a")).unwrap(), b"old");
    std::fs::remove_dir_all(parent).unwrap();
}
#[test]
fn local_calendar_uses_valid_civil_dates_and_bounded_time() {
    let day = local_day(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    )
    .unwrap();
    assert!((19700101..=99991231).contains(&day.date));
    assert!(day.previous < day.date);
    assert!(day.seconds < 86400);
    assert!(local_day(u64::MAX).is_err());
}

#[test]
fn shared_container_requires_real_entitlement() {
    // Cargo 单元测试可执行文件不携带 App Group entitlement；不访问或创建真实用户目录。
    assert!(crate::platform::storage::context().is_err());
}
