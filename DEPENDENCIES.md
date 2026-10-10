# Rust 依赖来源与许可证

以下从当前 Cargo 元数据生成，包含测试、可选功能和其他平台依赖，并非全部已链接到 macOS 库。版本由 Cargo.lock 固定；源码来自 crates.io 的配置镜像。组件版本与协议声明统一见本目录 [LICESE.md](LICESE.md)，新增分发依赖的上游许可证原文保留在 Notices/。正式分发前仍需核对实际链接组件、条件功能及嵌套的第三方通知，不将此清单视为发布许可审计完成。

rcgen 用于 MITM CA、站点证书以及测试服务器证书生成；TLS 使用 rustls 与 ring，没有引入代理内核。JS 使用 rquickjs 0.14.0 的安全 Rust API，底层 QuickJS 是 C 解释器；关闭可选模块加载、动态库和异步桥接功能。QuickJS 内嵌组件采用 MIT，声明见 [LICESE.md](LICESE.md)。

DNS 使用固定 Hickory resolver/proto/net 0.26.3，选择 MIT 许可；原始声明见 Notices/。传递依赖由 Cargo.lock 固定；清单不等于完整分发许可审计，没有引入代理内核。

运行时规则 YAML 使用 yaml-rust2 0.13.0 的事件解析器，关闭默认字符编码转换；关键词索引使用 aho-corasick 1.1.4 的连续 NFA。新增传递依赖 arraydeque 0.5.1、hashlink 0.12.2 由 Cargo.lock 固定，上游声明原文保存于 Notices/。不引入代理内核。
