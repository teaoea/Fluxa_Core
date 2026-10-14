# Fluxa Core

Cross-platform Rust network core by 可莉唯一的狗. Core code is MIT licensed; see [LICESE.md](LICESE.md). The SwiftUI application is maintained separately.

Technical documentation lives in [Documentation](Documentation/Overview_ENGLISH.md). See the [integer error protocol](Documentation/错误代码查询表.md), [mandatory development contract](Documentation/错误代码体系设计.md), [validation evidence](Documentation/ErrorReworkReview.md), and [release workflow](Documentation/Publishing.md). Unsupported capabilities are rejected explicitly; tests do not establish system VPN or all-platform acceptance.

GeoIP 修复包 v2.4.1：可选 `geoip_url` 只接受用户明确的 MMDB HTTPS 来源；未配置返回 WARN + Disabled，已配置失败返回 2401–2409，合法未匹配是非错误。使用与接口迁移见 [GeoIP.md](Documentation/GeoIP.md)，可运行示例为 `cargo run --example geoip_configuration`。本包不修改 App。

H1 修复包 v2.4.2：退役 Fake-IP 池不再阻止重叠区间变更或永久占用存活池额度；继续保护历史地址不被新 TUN 映射复用。可运行示例：`cargo run --example fake_ip_reconfigure`，审查见 [H1 生命周期修复](Documentation/H1-FakeIP生命周期修复.md)。本次不修改 App。
