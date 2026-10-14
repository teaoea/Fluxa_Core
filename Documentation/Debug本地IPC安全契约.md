# Debug 本地 IPC 安全契约

作者：可莉唯一的狗。适用：dev 专用 Debug 控制；App 接入另行交付。

## 强制安全边界

本契约落实用户确认的十三项安全要求：独立本地 IPC、禁止 TCP/UDP Debug 监听、普通接口不能绕过授权、配置禁止 Debug、订阅不能注入控制命令、验证 OS 对端身份、逐操作权限、全部请求严格校验、仅内存存储、明确 TTL、优先级不越过安全边界、Rust 独立管理、受控审计。

`routing::debug::Session` 与 `Router::with_debug` 为内核私有；普通 `fluxa_engine_command` 不接受 Debug 操作。公开 Rust `routing::debug::ipc::Client`、专用 C ABI 和 `NativeDebugClient` 只能操作已认证 IPC。策略 REJECT 可以被合法限时 Debug 规则覆盖；授权、协议报文检查、预算、会话生命周期不能被覆盖。

## 身份、凭证和权限

当前安全平台实现是 macOS Unix domain socket。内核从实际连接读取 UID、PID 和 audit token，不能用请求内自称的身份代替；基于 audit token 查询运行中代码并验证管理员固定的代码签名 requirement、Hardened Runtime 和动态调试状态。拒绝解除 library validation、可注入动态库、允许未签名可执行内存及其他列出的危险 entitlement。每次操作重新验证系统身份及代码签名。

同时要求 Ed25519 独立凭证证明。挑战使用系统安全随机源，每个连接使用不可复用内核标识；签名绑定协议域、连接标识、挑战、序号和完整载荷。序号必须严格递增；重放、篡改、跨连接搬用不能恢复授权。公开密钥不包含权限；权限从管理员只读策略取得，按 replace、clear、status、close、audit 分开验证。不能仅因同 UID 或知道路径而授权。

管理员策略是独立安全控制文件，不是主配置或订阅，**不允许声明 Debug 规则**。策略文件必须 root 所有、禁止组和其他用户写入，祖先目录同样受保护；可用 0644 供内核读取。示意结构：

```json
{
  "version": 1,
  "principals": [{
    "uid": 501,
    "requirement": "anchor apple generic and identifier \"example.debug.client\" and certificate leaf[subject.OU] = \"管理员核实的TeamID\"",
    "public_key": [0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],
    "permissions": ["status"]
  }]
}
```

这是结构示例，零密钥和占位 TeamID 不能用于真实授权。管理员必须核实真实签名主体及公钥；禁止使用 `true`、只靠可伪造 identifier 或任意签名人的宽泛 requirement。凭证是 32 字节私有 seed 文件，当前用户所有、0600，禁止符号链接，不能提交到 Git。实际 server/client 可执行文件均须符合核实的签名与运行时加固策略。没有可信策略、正确签名或凭证时明确失败，没有开发绕过开关。当前其他平台明确拒绝安全入口，能力位 26 不声明可用。

## 生命周期、资源与审计

IPC grant 的期限为 1–3600 秒；每条规则 TTL 必填且 1–3600 秒，同时不得超过 grant 剩余授权期限，建议 grant 大于规则 TTL 至少数秒。规则保存绝对过期时间及单调截止；任何一个到期都停止授权。规则和授权只存内存，不进入持久化规则、订阅或缓存。

清空、替换、关闭、连接断开或验证失败会撤销旧视图；旧 Arc 不延长调试权限。以 Debug 例外建立的连接由内核监听撤销及过期；正式路由切换不能把已到期的直连例外永久保留。清空/关闭成功响应前等待必要连接关闭；IPC Service.close 等待接收任务及实际端点释放，可重复关闭。Drop 仅发送内核关闭意图，不能代替完成状态。

服务读取/解析前预留 512 KiB，最多四个服务；每帧最大 64 KiB、全局最多八个 IPC 连接、每连接固定 512 KiB 预算；策略最多 32 KiB、16 个主体，每条规则仍按原预算/数量限制验证。读取、请求和授权均有截止；输出预算先于 ABI 操作。审计最多 256 项，单次读取最多 128 项，仅记录 OS 身份、操作、时间和整数结果，不记录私钥、证明、规则内容或敏感错误链。

## 接口与迁移示例

- Rust：`Service::start(policy_file, private_directory, manager)` 由内核启动受控监听；`Service::close().await` 等待关闭。`Client::connect(..., grant_seconds).await` 完成双向身份验证和签名证明，`client.command(&Request::Status {}).await` 发出专用请求。
- C ABI：`fluxa_engine_debug_ipc_start`、`fluxa_engine_debug_ipc_connect`、`fluxa_engine_debug_ipc_command`；客户端句柄不是业务引擎句柄。响应和客户端句柄沿用内核 destroy 契约。
- Swift：`NativeDebugClient(endpoint:credential:serverUID:serverRequirement:lifetimeSeconds:)`；`replace`、`clear`、`status`、`close`、`audit` 只编码请求并显示内核权威结果。
- Rust 运行示例见 `examples/debug_ipc.rs`；原 `Session::new()/replace` 和 `.with_debug()` 直接构造方式不再公开。dev API 按不稳定契约调整，禁止从 dev 发布到 crates.io；main 未改变。

专用请求例子：

```json
{"operation":"replace","rules":[{"rule":{"type":"DOMAIN","value":"example.com","policy":"direct","outbound":null},"protocol":"tcp","lifetime_seconds":30}],"outbound":null}
```

这是 IPC 请求，禁止放入 YAML 主配置。空/零/超过一小时 TTL 会返回 4001；身份拒绝为 4003、逐操作未授权为 4004、证明/重放/授权期限失败为 4005。每个错误的权威源码和 Schema 在 `错误代码查询表.md`。

## 验证与边界

验证记录单独维护于 `Debug修复验收.md`。签名测试只在私有单元测试入口构造可信策略，不导出生产绕过方法或 feature。临时 ad-hoc 签名与固定测试 seed 用于独立测试，不代表已完成 Developer ID 发布、App 沙箱、跨进程安装权限、真机 VPN 或所有平台验收。代码签名及输入验证不能保证抵御被合法签名调用方主动泄露凭证、管理员误配策略或操作系统自身漏洞；不得宣称防止一切进程注入。
