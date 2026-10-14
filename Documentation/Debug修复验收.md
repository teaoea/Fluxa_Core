# Debug 三项修复验收

日期：2026-10-15。作者：可莉唯一的狗。范围：Debug 完整出站优先级、专用本地 IPC 身份认证/授权、C ABI/Swift 封装。其他 TODO 继续暂停，App 不改动，main 和 crates.io 不改动。

## 已确认缺陷与独立规范依据

| 问题 | 规范、原实际行为和最小复现 | 修复及源码证据 | 独立验收 |
| --- | --- | --- | --- |
| 正式传输保护抢先于 Debug | 合法 Debug 可覆盖正式 REJECT。正式全 UDP REJECT + 60 秒 127.0.0.1/32 Debug DIRECT：求值 DIRECT，但完整出站 Rejected | `src/outbound/execution.rs::connect_with_snapshot` 先核验目标 Debug，DNS/IP 候选后核验；`routing::TrafficProtection` 保存正式保护与限时例外 | 实际 TCP、UDP、QUIC 数据面，清空恢复，普通流量拒绝回归通过 |
| 普通 Rust 构造入口未经专用授权 | Debug 必须经过专用本地 IPC；旧公开 Session::new/replace 和 Router::with_debug 允许绕过 | `src/routing/debug.rs` 构造/状态控制为 crate 私有；`src/routing/debug/ipc.rs` 仅可信策略、OS 身份、签名证明后授予逐操作权限 | 普通 ABI 拒绝 debug_* 及伪造 authorized/uid 字段；签名状态权限拒绝 replace，代码 4004 |
| 清空后旧视图仍放行 | 会话撤销不能依赖外部 Arc，调试授权不能因旧快照存活而延长。旧测试曾预期清空后旧视图 DIRECT | Compiled 的显式撤销信号、连接期限监听及 `Manager::close_revoked_debug`；新旧视图同步撤销，等待必要 I/O 释放 | 原测试迁入 crate 并按用户的新撤销规范改为 REJECT；未删除其断言。到期主动取消挂起 I/O 通过 |
| IPC I/O 依赖调用方执行器 | 内核独立运行。原监听在调用方运行时注册；调用方等待 Swift 子进程时握手超时 4005 | `Service::start_reserved` 以标准 Unix listener 创建，注册到内核私有执行器；内核任务托管 accept/授权/清理 | 阻塞调用方等待真实 Swift 子进程，签名 IPC 仍查询、替换、清空、审计、关闭成功 |
| 多控制入口可能互相覆盖 | 资源占用权基于有效内核身份，不是指针或引用计数 | Session 不可复用身份，Manager 对共享 Debug 所有者核验；旧身份不能解绑新会话 | 多入口冲突 4004，旧解绑不移除新所有者通过 |
| 认证拒绝类型可能被误报 | 公开错误真实、错误不能转换成功或混同规则 REJECT | `Greeting` 支持受控拒绝；认证阶段明确白名单映射及 reply 成功/错误一致性检查 | 身份/预算/内部失败保留代码，伪造 success + error 被拒绝；重放 4005 撤销授权 |

## L1：静态正确性

实际执行：

```sh
cargo fmt -- --check
cargo check --all-targets --offline
cargo clippy --all-targets --offline -- -D warnings
git diff --check
```

最终相关检查通过。Swift 类型检查与真实链接分别执行；没有用类型检查代替联调。

## L2：实现一致性

- `cargo test --offline -- --test-threads=1`：**245 通过，0 失败，1 项原有外部互通测试忽略**；忽略项没有被删除或改成通过。全局预算测试串行执行，专项并发回归仍保留。
- 18 组实际 Swift FFI：debug_ipc、error_code、response_retry、tun_dns、protocol_block、lifecycle、https、mitm、script、tun、tun_udp、dns、dns_service、routing、outbound、runtime_rules、subscription、geoip 全部通过。
- `python3 Tools/check_debug_ipc.py`：当前源码实际静态库、独立加固签名 Rust 测试及独立签名 Swift 进程，验证真实 Unix IPC 和 OS 身份。状态权限拒绝修改、正常授权替换/清空/审计/关闭、实际 UDP 放行/撤销、重复序号拒绝通过。交付核对补齐 `DebugIPC` 子模块可用性：旧 ABI 模块布局下 Swift 编译及明确不可用返回通过，新模块下完整签名联调通过。固定远程 v2.4.1 二进制不替换，新能力不冒充旧二进制已经提供。
- `cargo run --offline --example error_contract -- --check`：**43 项公开错误全登记，重复/遗漏/未定义/空字段均 0**；旧正式契约基线未改号、改义。新增 4003、4004、4005 集中于 `src/error.rs`，有中文注释、Schema、真实触发和后果。
- 原 Debug 测试从外部 integration 移入私有模块，保持正常规则、失败保留、TTL、配置禁止、来源和 DNS 前后预期。唯一有意更改的旧预期是清空后的旧视图放行，依据用户新的显式授权撤销规范。

完整 FFI 回归沿用 `check-ffi.sh` 的编译/运行方式，验证时复用已构建目标目录；新增专用测试可运行 `check-ffi.sh debug_ipc`。签名跨进程完整检查由 `Tools/check_debug_ipc.py` 重现，测试签名不改用户 App、系统策略或正式发布物。

## L3：独立业务语义核验

预期来自用户明确规范，不从索引或参考实现生成：

| 规范 | 实际核验结果 |
| --- | --- |
| Debug 高于正式 REJECT，不能越过安全/预算/协议约束 | 实际 TCP/UDP/QUIC 可用例外与清除恢复通过；非法 TTL、全局 transport DIRECT、数量超限失败保留通过；网络数据解析、节点支持和预算检查保留 |
| API-only、独立本地 IPC | 生产 Debug 控制只绑定 Unix socket；无 TCP/UDP Debug 监听。配置/订阅不能声明 Debug；普通 command 不新增 Debug 操作 |
| 身份不是客户端声明 | 实际 OS peer credentials/audit token + 动态代码签名/运行时加固核验，伪造身份字段拒绝 |
| 逐操作授权及防重放 | status 权限不能 replace；签名绑定会话、挑战、序号、载荷及 grant 期限；重复签名请求拒绝并撤销 |
| 生命周期由内核负责 | 到期取消挂起读写；清空/关闭撤销旧视图并等待必要连接释放及名额归还；IPC 监听不依赖调用方执行器；重复关闭结果稳定 |
| 没有隐式 DIRECT 或错误转成功 | 权限失败保持正式路由，错误/成功矛盾的 reply 拒绝；认证拒绝不能假造客户端句柄；既有 DNS/规则/TUN 失败回归通过 |
| 调试内存和审计有界 | 编译预算保留；服务读取前预留 512 KiB、最多四服务；连接 512 KiB、全局八名额；帧 64 KiB，审计 256 项，读取最多 128 项 |
| 外部不能取得规则/资源控制权 | Swift 和 C 客户端仅 IPC 句柄，Session/with_debug 为私有；授权所属使用内核非复用身份，引用计数不决定调试占用权 |

策略权限从启动时管理员只读策略取得；管理员撤销整个服务由内核 close 执行并等待释放，或由连接关闭/期限到期撤销 grant。没有实现策略文件热重载，不应把修改文件解释为已立即撤销存量权限。

## L4：证据、兼容性与未测边界

- 代码：`src/routing/debug/ipc.rs`、`src/platform/debug_identity.rs`、`src/routing/debug/semantic_tests.rs`、`src/outbound/configuration.rs`、`src/outbound/execution.rs`、`src/routing.rs`、`src/ffi/debug.rs`、`Sources/FluxaEngine/NativeDebugClient.swift`。
- 运行结果：本轮本机完整 Rust 245/0/1、18 组 FFI、当前源码签名跨进程通过、43 项错误契约通过。受控失败发现并修复：无参数请求接受未知字段、动态签名查询符号错误、调用方运行时依赖、认证拒绝误分类。没有把失败测试删除或降低断言。
- dev API 明确不稳定：删除未经授权的公开 Session 构造入口；提供专用 IPC Rust/C/Swift 迁移与运行示例。原普通 ABI 不新增隐式授权，新增能力位 26 仅表示 macOS 安全平台可用。正式 main/已发布包、Cargo 版本与 App 未修改。
- 本轮只推送 dev 源码修复，不发布 crate、XCFramework、正式版本标签或远程 Swift 包，不同步 crates.io。
- **尚未验证**：Developer ID 正式安装/管理员策略配置、App 沙箱、真实系统 VPN、非 macOS 的安全 IPC 实现/目标构建、被合法签名调用方主动泄露凭证及操作系统漏洞。临时 ad-hoc 固定哈希测试证明实际执行链，不替代这些验收，不宣称防止全部进程注入。
- 初期验证曾遇到自动审批复核超时；按工具允许重试后实际检查完成。超时没有被记作测试失败或安全否决。
