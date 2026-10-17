# Fluxa Core

作者：可莉唯一的狗

自有代码采用 MIT，许可记录见 [LICENSE.md](LICENSE.md)。

技术说明统一保存在 [Documentation](Documentation/Overview.md)，包括 [错误代码查询](Documentation/错误代码查询表.md)、[开发强制规范](AGENTS/错误代码体系设计.md)、[规则性能测试](Documentation/规则性能测试.md)、[API 与版本强制规范](AGENTS/API全生命周期管理.md)、[版本示例](Documentation/Versioning.md) 和 [发布流程](Documentation/Publishing.md)。

当前未完成事项及暂停状态统一见 [TODO](Documentation/TODO.md)。

主配置规则提供者使用小写 `rules` 正文，Rust 负责设备本地更新时间、缓存和事务；接口、示例及尚未接入的范围见 [规则提供者生命周期](Documentation/Subscriptions.md)。
