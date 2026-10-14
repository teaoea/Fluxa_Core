//! Fluxa 跨平台网络引擎；业务模块拒绝不安全代码。
//! HTTPS、MITM 与脚本运行时复用资源管理；代理协议分阶段实现。

mod budget;
pub mod dns;
pub mod error;
pub mod geoip;
pub mod http;
pub mod https;
mod lifecycle;
pub mod mitm;
mod network;
// 仅系统身份适配边界允许必要 FFI，业务模块仍禁止 unsafe。
pub mod outbound;
#[allow(unsafe_code)]
mod platform;
pub mod protocol;
pub mod rewrite;
pub mod routing;
pub mod script;
pub mod subscription;
pub mod traffic_script;
pub mod tun;

// 仅此模块允许 Rust 2024 的符号导出属性；不允许隐式不安全操作。
// 导出对象使用整数句柄；字节指针仅在同步调用期间借用。
#[allow(unsafe_code)]
pub mod ffi;
