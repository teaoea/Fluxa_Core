# Fluxa Core

跨平台 Rust 网络代理内核，作者：可莉唯一的狗。自有代码采用 MIT，许可记录见 [LICESE.md](LICESE.md)。SwiftUI App 位于独立仓库。

技术说明统一保存在 [Documentation](Documentation/Overview.md)，包括 [公开整数错误协议](Documentation/错误代码查询表.md)、[开发强制规范](Documentation/错误代码体系设计.md)、[验证与审查记录](Documentation/ErrorReworkReview.md)、[API 与版本强制规范](Documentation/API全生命周期管理.md)、[版本示例](Documentation/Versioning.md) 和 [发布流程](Documentation/Publishing.md)。

App 使用固定远程 Swift 包和经校验的静态二进制；未完成能力明确拒绝。文档及测试不代表系统 VPN 或全部目标平台已验收。

GeoIP 修复包 v2.4.1：可选 `geoip_url` 只接受用户明确的 MMDB HTTPS 来源；未配置返回 WARN + Disabled，已配置失败返回 2401–2409，合法未匹配是非错误。使用与接口迁移见 [GeoIP.md](Documentation/GeoIP.md)，可运行示例为 `cargo run --example geoip_configuration`。本包不修改 App。

H1 dev 源码修复：会话异步关闭主动撤销权限并等待活动资源释放；外部保留旧解析器/快照不阻止重叠区间重绑，旧对象操作返回错误，历史地址继续禁止复用。可运行示例：`cargo run --example fake_ip_reconfigure`，审查见 [H1 生命周期修复](Documentation/H1-FakeIP生命周期修复.md)。本次不修改 App、不发布到 crates.io；远程 Swift 包仍固定最后已发布的 v2.4.1 二进制，不能把它当成 H1 新实现。
