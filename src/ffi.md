# ffi.rs 代码说明

## 职责（干嘛的）

对应源码：`src/ffi.rs`，公开头文件为 `include/nodus_core.h`。模块将 Rust 能力暴露为固定 C ABI，目标是调用方无需持有 Rust 对象地址，也不能绕过内核资源、授权和生命周期边界。

## 目标（想达成什么）

以稳定值类型句柄和有界缓冲向外提供内核能力，维护输出无损重试、统一错误及确定关闭，不转移内部活动资源管理权。

## 类型与调用流程

`Engine` 汇集受管理网络能力；注册表分配不可复用整数句柄，区分引擎、响应缓冲区和专用资源。create 创建有效入口；command／command_v2 先检查引擎、指针长度及输出／并发预算，解码严格请求，再调用实际业务模块；返回值或输出参数传递受管理响应句柄。JSON 是 ABI 编码，不改变用户配置使用 YAML 的约定。

调用方先查询响应长度，再提供有效可写缓冲区复制，最后用唯一销毁入口释放句柄。输入借用仅在调用期间有效，Core 不保存外部临时指针。Rust 分配的缓冲区由 Rust 释放，输出容量不足不进行部分写入。响应预算在可能消费 TCP／TUN 数据或产生副作用的操作之前预留，准入失败允许释放旧响应后重试，不丢弃本应交付的数据。

## 错误、panic 与关闭

当前 ABI 为 2，错误由 error.rs 统一产生固定整数和结构化上下文；线程本地最后错误查询不分配响应句柄。last_error_copy 在 unwind 边界内序列化；编码错误或 panic 使用由同一权威定义生成并编译期核验的固定 SerializationFailed JSON。查询不清除原线程错误，空目标或容量不足返回所需长度且不写入。

destroy 先撤销注册表身份和新操作准入，再逐项尝试独立关闭步骤；一个阶段的错误或 unwind 不跳过后续步骤。最终返回首个真实错误，其他清理故障进入有界诊断。不恢复中毒锁，不把 destroy 失败宣称成全部资源已关闭；旧 Arc 不得恢复出站或监听权限，重复销毁仍报无效句柄。正常异步运行时和同步 ABI 的嵌套限制以 run_network 的实际准入为准。

## TUN、权限与可用性

二进制 submit／receive 处理受预算约束的包，队列为空与错误状态区分。普通命令不能注入 Debug；其独立认证 IPC 在 ffi/debug.rs 中维护。候选就绪、诊断查询或句柄存在不等于系统 VPN Running；本 ABI 不接受配置中的“已授权／检查通过”标志替代真实系统验证。

## VPN 前置检查事务命令（2.14.0-dev）

`Engine.vpn_transactions` 是该句柄的事务控制入口，事务本身保存在 `src/vpn/transaction.rs` 的进程唯一槽中。`vpn_preflight_begin` 在句柄未关闭时调用 `Controller::begin_preflight`，YAML 以 `Zeroizing` 持有，计划由前置检查在受理前严格校验；成功后回复刚受理事务的摘要。`vpn_preflight_status`、`vpn_transaction_status` 只读返回报告和槽内事务摘要；`VpnTransactionReply` 把 `Status` 中的错误转为公开 `ErrorInfo`，不含配置、凭据或系统原状态。

`begin_vpn_preflight` 持有 `Engine.vpn_transaction_admission` 锁完成关闭检查与登记，destroy 在同一把锁内置位关闭，避免“关闭检查之后、登记之前被销毁”的交错；锁内不等待网络任务。`vpn_transaction_stop`、`vpn_transaction_retry_cleanup` 由 `finish_vpn_transaction` 经 `run_network` 调用 `stop_status`／`retry_cleanup_status`，直接回复同一次操作取得的该事务终态，不再重新查询全局槽；两者也登记在不受忙碌状态阻塞的小型取消通道。destroy 在撤销准备准入（`vpn_closed`）之后调用 `Controller::stop_owned` 等待本句柄受理的事务结束；槽中已是其他事务时不做操作，清理失败作为首个真实错误返回且保留恢复入口。能力位 32 声明这些命令存在，不声明系统 VPN 可用。

测试专用暂停点 `PausePoint`／`test_pause` 只在测试编译存在，用于受控并发回归。

禁止：接受调用方提交的检查、授权或接管成功字段；用一个事务 ID 控制槽内的其他事务；在句柄关闭后受理新事务；把受理、网络检查通过或停止成功报告为 Running。对外契约见 [ffi_api.md](../Documentation/API/External/ffi_api.md) “VPN 前置检查事务命令”。

## 对应 API、示例与测试

- [ffi_api.md](../Documentation/API/External/ffi_api.md)：已编写的配置检查与 VPN 前置检查事务 C ABI 请求／响应。
- [error_api.md](../Documentation/API/External/error_api.md)：固定错误协议与参数 Schema。
- [ffi_error_query.rs](../examples/ffi_error_query.rs)：长度查询、容量不足与释放。
- [ffi_response_retry.rs](../examples/ffi_response_retry.rs)：输出预算与无损重试。
- [ffi_vpn_preflight.rs](../examples/ffi_vpn_preflight.rs)：受理前置检查、查询报告、停止与销毁，不接管系统 VPN。
- [test_ffi.rs](../tests/test_ffi.rs) 及 [同名文档](../tests/test_ffi.md)：实际边界和关闭回归；执行证据不等同于最低系统、真实权限或系统 VPN 验收。

## 禁止行为（不允许做什么）

不得保存外部临时指针、跨语言释放不属于本方分配的对象、复用撤销句柄或让 panic 穿越 ABI；输出预算不足不得先消费网络数据，关闭失败不能跳过其余独立清理。

违反输入、授权或资源边界时，使用实际模块的既有错误／拒绝路径，不伪造成功或擅自扩大权限；具体失败后果以上文流程和公开契约为准。

## 实时日志（2.16.0-dev）

当前结构化事件、connection_id、非阻塞消费者接口及可选加密协议见`Documentation/API/External/logging_api.md`。消费者拥有持久化与访问；Core不提供历史查询。日志事件不提升配置候选或VPN阶段的实际成功等级。
