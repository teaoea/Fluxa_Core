# Fluxa Core

跨平台 Rust 网络代理内核，作者：可莉唯一的狗。自有代码采用 MIT，许可记录见 [LICESE.md](LICESE.md)。SwiftUI App 位于独立仓库。

技术说明统一保存在 [Documentation](Documentation/Overview.md)，包括 [公开整数错误协议](Documentation/错误代码查询表.md)、[开发强制规范](Documentation/错误代码体系设计.md)、[验证与审查记录](Documentation/ErrorReworkReview.md)、[版本约定](Documentation/Versioning.md) 和 [发布流程](Documentation/Publishing.md)。

App 使用固定远程 Swift 包和经校验的静态二进制；未完成能力明确拒绝。文档及测试不代表系统 VPN 或全部目标平台已验收。

GeoIP 修复包 v2.4.1：可选 `geoip_url` 只接受用户明确的 MMDB HTTPS 来源；未配置返回 WARN + Disabled，已配置失败返回 2401–2409，合法未匹配是非错误。使用与接口迁移见 [GeoIP.md](Documentation/GeoIP.md)，可运行示例为 `cargo run --example geoip_configuration`。本包不修改 App。

H1 修复包 v2.4.2：退役 Fake-IP 池不再阻止重叠区间变更或永久占用存活池额度；继续保护历史地址不被新 TUN 映射复用。可运行示例：`cargo run --example fake_ip_reconfigure`，审查见 [H1 生命周期修复](Documentation/H1-FakeIP生命周期修复.md)。本次不修改 App。
