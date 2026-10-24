# 真实网络前置检查 Rust API

- API 分类：外部调用 API；仅承诺本文已说明的公开入口。

- 来源源码：`src/vpn/preflight.rs`；公开调度入口实现于 `src/vpn/transaction.rs`。
- 契约版本：Nodus-Core 2.10.0-dev，开发接口，无新增 C ABI。
- 入口：`Controller::begin_preflight`、`Controller::preflight_status`；停止仍使用 `Controller::stop`。

## 提交检查意图

```rust
let id = controller.begin_preflight(
    yaml,
    resources,
    &["https://your-probe.example/health".to_owned()],
    std::time::Duration::from_secs(30),
)?;
```

地址由调用方明确指定，不提供默认外部目标。1–32个不同的 HTTPS 地址，每个最多2048字节，不允许凭据、查询参数、片段或零端口。期限必须大于0且不超过120秒。资源必须是明确授权的只读文件，配置及引用仍严格校验；不隐式下载、搜索目录或回退旧文件。

返回 u64 只表示取得事务槽和受理。已有运行、检查、清理或回滚失败事务时返回 InvalidState。无效计划在准入前拒绝，不开始联网；配置语义和运行检查错误由异步事务状态返回。不能提交权限、授权或探测成功布尔值。

## 查询结果

`preflight_status(id)` 返回 `Result<Option<Report>>`。普通事务没有该报告时为 None；不存在／过期的事务ID返回 InvalidState。报告绑定同一事务ID，包含 findings、DNS上游数、必要／已验证代理数、成功探测数、GeoIP状态、network_checks_passed及activation_permitted，不含目标网址和节点名称。

Finding 的 check 为 configuration、rules-and-assets、geoip、dns、required-outbounds、resource-budget、permission、system-capability、loop-protection、local-authorization；outcome 为 not-run、passed、disabled、partial、unavailable、authorization-required、failed，error_info 沿用固定错误协议。GeoIP未完成为None；明确未启用才是WARN/Disabled，显式初始化失败不能当作禁用成功。

`network_checks_passed=true`只表示本次指定目标和必要代理覆盖通过网络检查；不代表完整Preflight通过，不授予接管。当前平台检查Unavailable、整体预算Partial、无节点授权未接入，因此整体Status仍为Preflight失败、Unsupported，activation_permitted恒为false。失败时检查Status.failure与cleanup_failure，不能只看成功探测数。

## 探测与错误

探测复用真实配置路由，校验证书，要求HTTP 2xx，正文上限4096字节；不执行复写、不跟随重定向、不把其他代理或DIRECT成功算给缺失代理。必要集合含活动原生代理复写引用；缺必要覆盖返回InvalidInput；策略REJECT保留Rejected；DNS、TLS、资源及网络错误保留真实错误。启用DNS必须至少提供一个可由上游解析的非IP主机名，localhost不能作为逐上游探测名。

stop请求取消并等待确定清理。用户取消为Cancelled，监督期限为NetworkTimeout；内部子检查可能因期限收到取消，Finding显示Cancelled而整体failure显示NetworkTimeout。已成功的子检查不是可复用授权或启动凭证。真实平台启动、授权和故障恢复仍需后续实现。

完整可运行示例见`examples/vpn_preflight.rs`；实现、目标及禁止行为见[preflight.md](../../../../src/vpn/preflight.md)。

## 2.11.0-dev 平台诊断增量

Report兼容新增platform_check可选字段，详见[platform_check_api.md](platform_check_api.md)。没有到达平台检查时为null；已执行网络检查但缺平台生产者时三项Unavailable。字段不是授权证据，activation_permitted仍为false；无新增C ABI接口。

## 2.14.0-dev C ABI 增量

`begin_preflight` 与 `preflight_status` 已经 `nodus_core_command_v2` 的 `vpn_preflight_begin`、`vpn_preflight_status` 命令暴露，能力位 32，契约见 [ffi_api.md](../ffi_api.md)。输入校验、报告字段与 `activation_permitted=false` 的语义不变；仍无平台证据提交入口。
