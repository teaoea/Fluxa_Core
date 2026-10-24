# 启动预算只读诊断 API

- API分类：外部调用API（只读诊断）；来源 `src/vpn/startup_budget.rs`，入口 `vpn::preflight::Report::startup_budget`，通过 `Controller::preflight_status` 或 C ABI `vpn_preflight_status` 查询。
- 契约版本：2.15.0-dev；相关内部校验见 [Internal](../../Internal/vpn/startup_budget_api.md)。不新增C符号，不更改命令名或错误码。

## 字段与语义

`startup_budget` 为可选Assessment，未到达预算检查时为null。Assessment包含transaction（同次事务ID）、peak/additional（可选Usage）、reason（可选Reason）、budget_satisfied（仅本次预算检查是否满足）。Usage包含memory_bytes和file_descriptors，均u64、单位分别为字节和个。诊断不包含系统空闲量、地址、路径或配置原文，不是授权、资源租约或可以提交回Core的证据。

reason稳定序列化值：incomplete-plan、missing-system-observation、invalid-observation、expired-observation、arithmetic-overflow、process-memory-limit、process-descriptor-limit、system-memory-headroom、system-descriptor-headroom、cancelled。它们是预算诊断原因，不是第二套错误码；无效事务及重复内部消费仍使用现有InvalidInput/InvalidState。

当前生产示例：`{"transaction":7,"peak":null,"additional":null,"reason":"incomplete-plan","budget_satisfied":false}`。完整计划未知时总额为null，不将未知呈现为0。既有resource-budget Finding仍为partial，activation_permitted仍为false。

## 调用前提、生命周期和失败

调用方必须使用本次有效事务ID，错误与终态仍查询事务Status.failure/cleanup_failure。普通事务没有前置检查报告时为None；过期或不存在ID返回既有InvalidState。该查询不创建资源或修改系统状态。报告生命周期由Controller管理，复制摘要不会恢复启动权限。

即使未来预算满足，仍须通过全部前置检查、真实资源预留和四阶段事务，不能据此显示Running。当前没有完整计划生产者和可信OS余量采集器，无新增外部输入，不宣称完整P0预算或真实VPN验收。可运行示例 `examples/vpn_preflight.rs`；验证见 `tests/test_vpn_startup_budget.rs`、`tests/test_vpn_preflight.rs`及同名报告。
