//! Nodus 跨平台网络引擎；业务模块拒绝不安全代码。
//! HTTPS、MITM 与脚本运行时复用资源管理；代理协议分阶段实现。

// 支持范围由强制平台规范定义；不能借助手动 Cargo 调用生成被排除的 Apple 产物。
#[cfg(all(target_vendor = "apple", not(target_arch = "aarch64")))]
compile_error!("Nodus Core 的 Apple 平台仅支持 ARM64，不支持 Intel Mac 或 x86 模拟器");
#[cfg(any(
    target_os = "watchos",
    target_os = "visionos",
    target_env = "macabi",
    target_abi = "macabi"
))]
compile_error!("Nodus Core 不支持 Mac Catalyst、watchOS 或 visionOS");

mod budget;
pub mod configuration;
pub mod dns;
pub mod error;
pub mod geoip;
pub mod http;
pub mod https;
mod lifecycle;
pub mod logging;
pub mod mitm;
mod network;
// 仅系统身份适配边界允许必要 FFI，业务模块仍禁止 unsafe。
pub mod outbound;
#[allow(unsafe_code)]
mod platform;
pub mod protocol;
/// 内核规则提供者状态与正文预验证，不暴露活动生命周期控制权。
pub mod providers;
pub mod rewrite;
pub mod routing;
pub mod script;
/// 共享容器配置读写；扩展宿主只读。
pub mod storage;
pub mod subscription;
pub mod traffic_script;
pub mod tun;
/// VPN 前置检查的隔离资产准备；不授予系统接管权限。
pub mod vpn;

// 仅此模块允许 Rust 2024 的符号导出属性；不允许隐式不安全操作。
// 导出对象使用整数句柄；字节指针仅在同步调用期间借用。
#[allow(unsafe_code)]
pub mod ffi;

// 外置单元测试复用既有公开契约夹具，不扩大任何生产 API。
#[cfg(test)]
extern crate self as nodus_core;

// 共用夹具每个测试 crate 只加载一次，避免迁移后重复模块及重复类型。
#[cfg(test)]
#[path = "../tests/test_fixture_dnssec.rs"]
mod test_fixture_dnssec;
#[cfg(test)]
#[path = "../tests/test_fixture_https.rs"]
mod test_fixture_https;
