# Fluxa Core

作者：可莉唯一的狗

自有代码采用 MIT，许可记录见 [LICENSE.md](LICENSE.md)。

技术说明统一保存在 [Documentation](Documentation/Overview.md)，包括 [错误代码查询](Documentation/错误代码查询表.md)、[开发强制规范](AGENTS/错误代码体系设计.md)、[规则性能测试](Documentation/规则性能测试.md)、[API 与版本强制规范](AGENTS/API全生命周期管理.md)、[版本示例](Documentation/Versioning.md) 和 [发布流程](Documentation/Publishing.md)。

当前未完成事项及暂停状态统一见 [TODO](Documentation/TODO.md)。

主配置规则提供者使用小写 `rules` 正文，Rust 负责设备本地更新时间、缓存和事务；接口、示例及尚未接入的范围见 [规则提供者生命周期](Documentation/Subscriptions.md)。

主配置 DNS 与 GeoIP 已接入内核异步事务：逐上游独立期限、专用 DNS 出站、地址族选择、Ready 数据库编译及同目录原子缓存。接口、示例和真实验证边界见 [DNS/GeoIP 编排](Documentation/DNSGeoIP编排.md)。系统代理、局域网共享及文件日志执行仍暂存 TODO。

## VPN 隔离候选准备（开发接口）

2.6.0-dev 提供有界、可取消的候选资产准备，复用配置、DNS/GeoIP 与提供者规则编译，不发布到当前管理器、不改写正式缓存。候选就绪不等于必要链路可用、前置检查通过、用户授权或系统 VPN 启动。参见 [候选准备说明](Documentation/VPN候选准备.md) 和 examples/vpn_candidate.rs。

共享容器与默认主配置由 Core 管理，固定 App Group、扩展只读及真实验证边界见 [共享存储](Documentation/共享存储.md)。
