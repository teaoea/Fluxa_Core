# Fluxa Core

Cross-platform Rust network core by 可莉唯一的狗. Core code is MIT licensed; see [LICESE.md](LICESE.md). The SwiftUI application is maintained separately.

Technical documentation lives in [Documentation](Documentation/Overview_ENGLISH.md). See the [integer error protocol](Documentation/错误代码查询表.md), [mandatory development contract](Documentation/错误代码体系设计.md), [validation evidence](Documentation/ErrorReworkReview.md), and [release workflow](Documentation/Publishing.md). Unsupported capabilities are rejected explicitly; tests do not establish system VPN or all-platform acceptance.

GeoIP 修复包 v2.4.1：可选 `geoip_url` 只接受用户明确的 MMDB HTTPS 来源；未配置返回 WARN + Disabled，已配置失败返回 2401–2409，合法未匹配是非错误。使用与接口迁移见 [GeoIP.md](Documentation/GeoIP.md)，可运行示例为 `cargo run --example geoip_configuration`。本包不修改 App。

H1 dev 源码修复：会话异步关闭主动撤销权限并等待活动资源释放；外部保留旧解析器/快照不阻止重叠区间重绑，旧对象操作返回错误，历史地址继续禁止复用。可运行示例：`cargo run --example fake_ip_reconfigure`，审查见 [H1 生命周期修复](Documentation/H1-FakeIP生命周期修复.md)。本次不修改 App、不发布到 crates.io；远程 Swift 包仍固定最后已发布的 v2.4.1 二进制，不能把它当成 H1 新实现。
