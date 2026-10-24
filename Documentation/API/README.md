# Nodus Core API

本目录仅维护 API 契约与必要导航，可作为 GitBook 内容源。每页标明分类、来源源码、入口、调用对象及契约状态；实现说明位于 `src/` 对应同名 Markdown。开发接口不等同于已发布稳定契约。

| 来源源码 | API 文档 | 已编写范围 |
| --- | --- | --- |
| `src/configuration/check.rs` | [check_api.md](External/configuration/check_api.md) | 只读配置检查、诊断及源位置 |
| `src/ffi.rs` | [ffi_api.md](External/ffi_api.md) | 配置检查 C ABI 命令；其他命令未在本页承诺文档化 |
| `src/error.rs` | [error_api.md](External/error_api.md) | 固定错误协议、Schema 及 [机器契约](External/error_api.json) |
| `src/vpn/preflight.rs` | [preflight_api.md](External/vpn/preflight_api.md) | 真实网络前置检查及未就绪项；不授予接管 |
| `src/vpn/transaction.rs` | [transaction_api.md](External/vpn/transaction_api.md) | 事务驱动 Rust 接口，尚无系统启动能力 |

配置检查不应用配置、不启动 VPN；事务文档中的状态也不代替系统数据路径验收。其他 API 后续按真实实现补充。

## 分类与逐项交付

- `External/`：外部调用方可访问的公开 Rust API、C ABI 与协议。以上历史已编写页面按实际公开用途迁入此目录，未扩展其已承诺范围。
- `Internal/`：Core 内部模块调用契约，不提供外部授权／控制入口。
- 每项 API 新增或修改时同步文档、导航、示例和适用测试；私有辅助实现仍见 `src/` 同名文档。文档迁移不改变可见性、错误协议或发布状态。

| 分类 | 来源源码 | API 文档 | 已编写范围 |
| --- | --- | --- | --- |
| 外部调用 | `src/vpn/platform_check.rs` | [platform_check_api.md](External/vpn/platform_check_api.md) | 只读平台诊断，无证据提交入口 |
| 内部自用 | `src/vpn/platform_check.rs` | [platform_check_api.md](Internal/vpn/platform_check_api.md) | 单次请求、缺证据拒绝及私有校验边界 |

本导航登记实际已编写范围，不代表全部历史内部 API 已补全文档。

- [混合策略与自动连接重试](External/configuration/rules_api.md)
- [策略解析内部API](Internal/routing/strategy_api.md)

- [规则集正文校验API](External/providers_api.md)
