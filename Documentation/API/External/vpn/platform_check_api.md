# 平台检查诊断 Rust API

- API 分类：外部调用 API；仅承诺本文已说明的公开入口。

- 来源源码：`src/vpn/platform_check.rs`、`src/vpn/preflight.rs`。
- 入口：`Controller::preflight_status(id)`返回`Report.platform_check: Option<Assessment>`。
- 契约版本：2.11.0-dev；兼容新增Rust诊断，无新增C ABI或激活能力位。

Assessment的公开字段为transaction、platform、permission、system_capability、loop_protection、reason。三项状态为not-run、passed、unavailable、rejected；序列化使用稳定kebab-case。reason用于说明missing-adapter、cancelled、consumed、expired、wrong-request、wrong-platform、source-revoked、source-failure、invalid-observation、permission-denied、permission-undetermined、missing-capability、egress-unresolved、missing-loop-guard或invalid-route-coverage，不替代error.rs的四位错误协议。

Report尚未进入平台检查时字段为None；生产缺少平台接线时为Some，三项Unavailable，reason为missing-adapter。该结果不能换算为Running。网络检查成功时activation_permitted仍为false。

当前没有可接收成功证据的公开Rust构造、JSON或FFI入口；内部权限状态值本身不是证据。Request、VerifiedEvidence和校验流程均为内部实现，外部不能构造或重放。诊断Assessment可以复制但不能提交回内核授权。

可运行示例沿用`examples/vpn_preflight.rs`：显式提供现有配置及HTTPS探测目标，输出报告并以真实错误退出，不自动接管系统。实现、限额、真实平台生产者的前提和未实现边界见[src/vpn/platform_check.md](../../../../src/vpn/platform_check.md)。

内部请求及校验约束另见[内部自用 API](../../Internal/vpn/platform_check_api.md)；该链接不提供外部证据提交能力。
