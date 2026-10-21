# Rust 依赖来源与许可证

以下从当前 Cargo 元数据生成，包含测试、可选功能和其他平台依赖，并非全部已链接到 macOS 库。版本由 Cargo.lock 固定；源码来自 crates.io 的配置镜像。组件版本与协议声明统一见本目录 [LICESE.md](../LICESE.md)，新增分发依赖的上游许可证原文保留在 Notices/。正式分发前仍需核对实际链接组件、条件功能及嵌套的第三方通知，不将此清单视为发布许可审计完成。

rcgen 用于 MITM CA、站点证书以及测试服务器证书生成；TLS 使用 rustls 与 ring，没有引入代理内核。JS 使用 rquickjs 0.14.0 的安全 Rust API，底层 QuickJS 是 C 解释器；关闭可选模块加载、动态库和异步桥接功能。QuickJS 内嵌组件采用 MIT，声明见 [LICESE.md](../LICESE.md)。

DNS 使用固定 Hickory resolver/proto/net 0.26.3，选择 MIT 许可；原始声明见 Notices/。传递依赖由 Cargo.lock 固定；清单不等于完整分发许可审计，没有引入代理内核。

运行时规则 YAML 使用 yaml-rust2 0.13.0 的事件解析器，关闭默认字符编码转换；关键词索引使用 aho-corasick 1.1.4 的连续 NFA。新增传递依赖 arraydeque 0.5.1、hashlink 0.12.2 由 Cargo.lock 固定，上游声明原文保存于 Notices/。不引入代理内核。

VLESS/REALITY/Vision 由自有 Rust 代码实现；ring 0.17.14、x25519-dalek 2.0.1、ml-kem 0.3.2（秘密清零）仅提供密码原语。指纹公开字段参考固定 uTLS 提交 ff1b50fbbe9a6dff1dcb1cfc0493bd5b0f073f67，保留 BSD 声明，不链接其 Go 实现。独立 Xray v26.3.27 只用于互通测试，不进入交付物。新增依赖及原文声明见 LICESE.md 与 Notices/。ml-kem 上游未独立审计，此处不宣称完成密码学或许可分发审计。

运行时规则缓存采用自有小端二进制平铺索引；SHA-256 复用现有 ring。历史上游声明保留于 Notices/，不代表当前仍链接其中的组件。

离线 GeoIP 使用 maxminddb 0.32.0（ISC），关闭默认 mmap 与 unsafe-str-decode 功能；新增传递依赖 ipnetwork 0.21.1（MIT/Apache-2.0）。许可证原文保存于 Notices/maxminddb 与 Notices/ipnetwork。MMDB 数据库不属于 crate，宿主分发免费 DB-IP Lite 时须另行保留 CC BY 4.0 署名。


## 注册域名关键词匹配

直接依赖 `psl = 2.1.238`、传递依赖 `psl-types = 2.0.11`，协议均为 `MIT/Apache-2.0`（来自各自实际 Cargo 元数据）。固定静态 PSL 包含 ICANN 与 PRIVATE 后缀，用于借用输入中的可注册域名标签，不在查询时下载列表或建立动态堆索引。`foo.github.io` 的租户标签是 `foo`；`example.co.uk` 是 `example`。数据变化通过审查依赖升级处理，不绕过锁文件。声明见根目录 `LICENSE.md`。

## macOS 进程归属

固定 `libproc = 0.14.11`（MIT）仅用于 macOS 系统 Socket 结构与联合体判别类型；平台模块调用既有 libc，核对返回记录完整长度，不依赖上游字符串错误。新增构建依赖由 Cargo.lock 固定，实际协议登记于 LICENSE.md，声明保留于 Notices。用途不是引入代理内核。上游构建使用 bindgen/libclang 与 Apple SDK；本机 Command Line Tools SDK 已构建验证，其他 Xcode-only、交叉编译环境尚未验证。查询固定 PID/FD 数量、总检查次数、1 秒截止和最多两次并发 64 KiB 扫描，超限返回资源错误而不是假装未知。

Apple 移动端构建启用 rquickjs 的 bindgen 功能，按固定目标 SDK 生成 QuickJS C 绑定；新增构建依赖由 Cargo.lock 锁定，不改变同步解释器、关闭 JIT 及脚本运行权限。跨平台构建通过不代表传递依赖许可审计完成。
