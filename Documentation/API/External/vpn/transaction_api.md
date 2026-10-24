# VPN 四阶段事务驱动 Rust API

- API 分类：外部调用 API；仅承诺本文已说明的公开入口。

- 来源源码：`src/vpn/transaction.rs`。
- 入口：`Controller::{begin,current_status,status,stop,retry_cleanup}`及只读`Status`。
- 契约版本：Nodus-Core 2.9.0-dev；开发接口，当前没有生产Steps实现，无C ABI入口或接管能力位，不可用于启动系统VPN。

## 可调用的只读查询

```rust
use nodus_core::vpn::transaction::Controller;
let controller = Controller::default();
let status = controller.current_status()?;
# Ok::<(), nodus_core::error::Error>(())
```

完整可运行只读示例为`examples/vpn_transaction_status.rs`。None表示没有保留的事务，不代表操作系统VPN状态已验证。Some展示内核ID、阶段、系统变更状态、原失败阶段/错误和清理错误，不含配置或系统原状态。

## 启动、停止与恢复契约

begin仅接受密封Steps，外部Rust调用者不能自行实现Sealed或提供“全部检查成功”的替身；当前生产没有可构造的驱动。期限必须在(0,120秒]，返回ID仅表示受理。stop按ID请求取消并等待确定清理；重复调用共享结果，原阶段失败不被清理成功覆盖。retry_cleanup只重试保留事务的清理，不重新启动；Unknown状态清理成功后才能恢复为Restored。旧ID、已有活动/清理失败事务、非恢复状态均拒绝InvalidState。

未来FFI及可信平台桥必须由Core验证同次配置、授权、检查及实际系统结果；此API不接受JSON成功标志，也不承诺当前已经接通任何平台权限或数据路径。详细算法与生命周期见[src/vpn/transaction.md](../../../../src/vpn/transaction.md)。

## 完成后的资源边界

stop／retry_cleanup 只有在回滚成功且内核持有的步骤资源释放后才报告清理完成；只读状态仍可查询。回滚失败保留恢复对象和 Unknown，不能开启下一事务。若资源析构发生 panic，同样返回清理错误，不将其包装成停止成功；已消失的步骤对象不能复活，重试缺失对象返回 InvalidState。外部保留的 Arc 不授予活动资源使用权，真实平台资源撤销仍由可信 rollback 实现负责。

## 真实检查入口（2.10.0-dev 增量）

`begin_preflight`与`preflight_status`见[preflight_api.md](preflight_api.md)。它们复用本事务槽；网络通过也不绕过未实现的权限、系统能力、预算及防回环条件，不开始系统VPN。

## 2.14.0-dev C ABI 增量

`current_status`、`stop`、`retry_cleanup` 已通过 `nodus_core_command_v2` 的 `vpn_transaction_status`、`vpn_transaction_stop`、`vpn_transaction_retry_cleanup` 命令暴露，能力位 32，契约见 [ffi_api.md](../ffi_api.md)。`begin`（密封 Steps 驱动）仍无 C ABI 入口，也没有接管能力位；FFI 命令不接受成功标志，不能启动系统 VPN。
