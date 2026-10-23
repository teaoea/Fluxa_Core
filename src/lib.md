# lib.rs 代码说明

## 职责（干嘛的）

对应源码：`src/lib.rs`。本页说明实现职责、数据流程和边界；公开调用的请求／响应契约单独维护于 `Documentation/`，开发约束维护于 `AGENTS/`。

## 目标（想达成什么）

将公共模块与外部依赖组织为可嵌入的 Rust 网络库，明确哪些协议和系统能力由本库提供。

## 主要流程与模块协作

lib.rs 声明配置、DNS、出站、规则、脚本、TUN、VPN 和 FFI 模块；业务实现位于各模块，依赖版本由 Cargo.lock 固定，第三方原文由 Notices 保留。

## 输入、输出与失败边界

输入来自受约束的配置、明确授权的资源或校验后的网络数据；各入口的实际类型、字段和值域以下文及对应源码为准。正常结果只能说明该模块完成其职责，不能推定网络接管、其他模块初始化或外部权限已经成功。非法输入、协议错误、系统故障、取消和资源不足沿用 `src/error.rs` 的真实分类；不静默改为 DIRECT，也不修复或覆盖外部配置。

活动资源由 Core 的会话身份、有效租约、取消及明确关闭操作管理；只读快照引用不构成活动权限。内存与任务的实际存活期计入对应预算。具体模块没有活动任务时，不因本说明增加后台线程或额外权限。

## 测试与示例入口

对应测试及实际结果：[test_error_contract.md](../tests/test_error_contract.md)、[test_error_contract.rs](../tests/test_error_contract.rs)。

性能结果及执行记录保存在对应测试文档；下文涉及旧版本的验证陈述仅是历史资料，不是本次重新执行的结果。已发布版本的历史能力不能替代当前代码、真实平台或系统 VPN 验收。

## 实现细节

以下从当前 Cargo 元数据生成，包含测试、可选功能和其他平台依赖，并非全部已链接到 macOS 库。版本由 Cargo.lock 固定；源码来自 crates.io 的配置镜像。组件版本与协议声明统一见本目录 [LICENSE.md](../LICENSE.md)，新增分发依赖的上游许可证原文保留在 Notices/。正式分发前仍需核对实际链接组件、条件功能及嵌套的第三方通知，不将此清单视为发布许可审计完成。

rcgen 用于 MITM CA、站点证书以及测试服务器证书生成；TLS 使用 rustls 与 ring，没有引入代理内核。JS 使用 rquickjs 0.14.0 的安全 Rust API，底层 QuickJS 是 C 解释器；关闭可选模块加载、动态库和异步桥接功能。QuickJS 内嵌组件采用 MIT，声明见 [LICENSE.md](../LICENSE.md)。

DNS 使用固定 Hickory resolver/proto/net 0.26.3，选择 MIT 许可；原始声明见 Notices/。传递依赖由 Cargo.lock 固定；清单不等于完整分发许可审计，没有引入代理内核。

运行时规则 YAML 使用 yaml-rust2 0.13.0 的事件解析器，关闭默认字符编码转换；关键词索引使用 aho-corasick 1.1.4 的连续 NFA。新增传递依赖 arraydeque 0.5.1、hashlink 0.12.2 由 Cargo.lock 固定，上游声明原文保存于 Notices/。不引入代理内核。

VLESS/REALITY/Vision 由自有 Rust 代码实现；ring 0.17.14、x25519-dalek 2.0.1、ml-kem 0.3.2（秘密清零）仅提供密码原语。指纹公开字段参考固定 uTLS 提交 ff1b50fbbe9a6dff1dcb1cfc0493bd5b0f073f67，保留 BSD 声明，不链接其 Go 实现。独立 Xray v26.3.27 只用于互通测试，不进入交付物。新增依赖及原文声明见 LICENSE.md 与 Notices/。ml-kem 上游未独立审计，此处不宣称完成密码学或许可分发审计。

运行时规则缓存采用自有小端二进制平铺索引；SHA-256 复用现有 ring。历史上游声明保留于 Notices/，不代表当前仍链接其中的组件。

离线 GeoIP 使用 maxminddb 0.32.0（ISC），关闭默认 mmap 与 unsafe-str-decode 功能；新增传递依赖 ipnetwork 0.21.1（MIT/Apache-2.0）。许可证原文保存于 Notices/maxminddb 与 Notices/ipnetwork。MMDB 数据库不属于 crate，宿主分发免费 DB-IP Lite 时须另行保留 CC BY 4.0 署名。


## 注册域名关键词匹配

直接依赖 `psl = 2.1.238`、传递依赖 `psl-types = 2.0.11`，协议均为 `MIT/Apache-2.0`（来自各自实际 Cargo 元数据）。固定静态 PSL 包含 ICANN 与 PRIVATE 后缀，用于借用输入中的可注册域名标签，不在查询时下载列表或建立动态堆索引。`foo.github.io` 的租户标签是 `foo`；`example.co.uk` 是 `example`。数据变化通过审查依赖升级处理，不绕过锁文件。声明见根目录 `LICENSE.md`。

## macOS 进程归属

固定 `libproc = 0.14.11`（MIT）仅用于 macOS 系统 Socket 结构与联合体判别类型；平台模块调用既有 libc，核对返回记录完整长度，不依赖上游字符串错误。新增构建依赖由 Cargo.lock 固定，实际协议登记于 LICENSE.md，声明保留于 Notices。用途不是引入代理内核。上游构建使用 bindgen/libclang 与 Apple SDK；本机 Command Line Tools SDK 已构建验证，其他 Xcode-only、交叉编译环境尚未验证。查询固定 PID/FD 数量、总检查次数、1 秒截止和最多两次并发 64 KiB 扫描，超限返回资源错误而不是假装未知。

Apple 移动端构建启用 rquickjs 的 bindgen 功能，按固定目标 SDK 生成 QuickJS C 绑定；新增构建依赖由 Cargo.lock 锁定，不改变同步解释器、关闭 JIT 及脚本运行权限。跨平台构建通过不代表传递依赖许可审计完成。

## 禁止行为（不允许做什么）

不得引入被禁止的第三方代理内核，不能把模块导出或依赖链接成功当作真实网络能力验收。

违反输入、授权或资源边界时，使用实际模块的既有错误／拒绝路径，不伪造成功或擅自扩大权限；具体失败后果以上文流程和公开契约为准。
