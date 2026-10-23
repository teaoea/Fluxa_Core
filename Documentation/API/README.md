# Nodus Core API

本目录仅维护 API 契约与必要导航，可作为 GitBook 内容源。每页标明来源源码、入口及契约状态；实现说明位于 `src/` 对应同名 Markdown。开发接口不等同于已发布稳定契约。

| 来源源码 | API 文档 | 已编写范围 |
| --- | --- | --- |
| `src/configuration/check.rs` | [check_api.md](configuration/check_api.md) | 只读配置检查、诊断及源位置 |
| `src/ffi.rs` | [ffi_api.md](ffi_api.md) | 配置检查 C ABI 命令；其他命令未在本页承诺文档化 |
| `src/error.rs` | [error_api.md](error_api.md) | 固定错误协议、Schema 及 [机器契约](error_api.json) |

配置检查不应用配置、不启动 VPN。其他 API 后续按真实实现补充。
