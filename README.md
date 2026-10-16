# Fluxa Core

作者：可莉唯一的狗

自有代码采用 MIT，许可记录见 [LICENSE.md](LICENSE.md)。

技术说明统一保存在 [Documentation](Documentation/Overview.md)，包括 [错误代码查询](Documentation/错误代码查询表.md)、[开发强制规范](AGENTS/错误代码体系设计.md)、[规则性能测试](Documentation/规则性能测试.md)、[API 与版本强制规范](AGENTS/API全生命周期管理.md)、[版本示例](Documentation/Versioning.md) 和 [发布流程](Documentation/Publishing.md)。

当前未完成事项及暂停状态统一见 [TODO](Documentation/TODO.md)。

## Swift / macOS 适配边界

Swift 数据类型、C ABI 调用封装、系统适配和 Swift 测试已移至 Fluxa 仓库 `Packages/FluxaKernelBridge/`；本仓库不再提供 Swift Package。内核仅交付 Rust、C ABI 和固定校验值的原生二进制。历史发布标签不改写，main 不变。

自研 TCP/TUN dev 数据包会话见 [TunTCP.md](Documentation/TunTCP.md)；正式发布与系统 VPN 接管另行验收。
