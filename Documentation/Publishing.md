# libfluxa_core 发布说明

## 当前强制发布依据

API、版本与正式发布以 [《API全生命周期管理》](API全生命周期管理.md) 为唯一权威规范。dev 禁止持有 crates.io 发布凭据、执行生产发布或上传正式 crate；crates.io 仅允许已审查的受保护 main 来源，经授权的受控流程发布。内核版本采用 SemVer；示例见 [版本说明](Versioning.md)。以下既往发布说明及核验记录保留为历史证据，不构成继续从 dev 正式发布或沿用旧版本递增规则的授权，亦不表示新发布守卫已经实现或验证。

## 既往发布流程及记录

Rust 包名与 Rust 库名均为 libfluxa_core，版本 0.3.0。对外 C ABI 的 fluxa_engine_ 前缀保留兼容；Swift 产品 FluxaEngine、FluxaCore 与 FluxaEngine.xcframework 名称不变，内部静态二进制为 libfluxa_core，无扩展名。

crates.io 发布内容由 Cargo.toml 的 include 白名单限定：Rust 源码、C ABI 头、Rust 测试、测试例程和指定文档；不包含 Swift/App 源码、钥匙串、真实证书私钥、缓存或二进制。先执行 cargo package --list 与 cargo publish --dry-run --registry crates-io，再上传。凭据只通过 cargo login 或受保护环境变量配置，不写入仓库和聊天。

GitHub Release 用于 Apple 二进制下载；crates.io 用于 Rust 源码库，不承载 XCFramework。发布工作流在 dev 的 Release.version 变更时执行 Rust/真实 FFI 测试、构建 macOS ARM64 静态框架、计算压缩包 SHA-256、更新 Package.swift、提交 dev 并创建版本标签和下载附件；不合并 main。Swift 包不提供本地目录回退。App 固定远程包 0.3.0，通过 URL 与 SHA-256 下载二进制。

当前只提供 macOS ARM64 切片；其他 Apple 平台明确不可用。发布失败必须记录，不能把本地构建或 GitHub 上传称为 crates.io 上传成功。

## 版本与作者

作者署名为可莉唯一的狗。后续版本递增遵循《API全生命周期管理》第四章，不再适用旧版本约定。Cargo/Swift 元数据不带 v，Git 标签和发布标题带 v。0.1.0 已经 crates.io 实际上传并确认索引；后续修改需要新版本，不能覆盖已发布版本。

0.1.1 为 README 文档修订，移除客户端与开发流程说明，只保留网络内核介绍；网络实现与 0.1.0 相同。

每项功能的开发、验证、源码提交、同版本发布、客户端更新与验收顺序遵循 AGENTS.md。发布记录同时记录构建源码提交和最终版本标签；生成校验值提交只更新下载元数据，不改动已打包的 Rust 源码。


## 0.1.1 发布核验

构建和 crate 源码提交为 5f47b9afee5dd82b3322b8406db5b704b7e0f4af；最终 v0.1.1 标签为 e7aa631d818c3dc2ab4a9ab3b064b6f0b9b182a9，二者仅 Package.swift 校验值不同，Rust 源码完全相同。crate 的 .cargo_vcs_info.json 已核对。

Rust 36 项测试、Clippy、真实 Swift FFI 四组与远程 Swift 包 10 项测试全部通过；GitHub Actions 运行 38018885504 成功，crates.io 已实际发布 0.1.1。框架压缩包 SHA-256 为 3cd9a441ca55e404914cb5358c42fc6d4267ada8f84bbc381121aead04a3f29e。仅验证 macOS ARM64，最低系统、其他平台、真机、系统浏览器信任与远端代理互通仍未验收。


## 0.2.0 发布核验

新增受限流量响应脚本，版本按功能号递增。构建及 crate 源码提交 e7b7d75e2b2994b98bde14b11d289c743e87a9a5，最终 v0.2.0 标签 e92801bd9af33e1f22c9c2591e9094c8f59165ba 仅更新 Package.swift 校验值，Rust 源码相同。发布从该确定提交的干净副本进行，避免 Finder 元数据被 src 通配符纳入 crate；实际包列表与 .cargo_vcs_info.json 已核对，不使用 allow-dirty 绕过来源校验。

Rust 38 项、Clippy、真实 FFI 四组、远程 Swift 包 10 项全部通过；GitHub Actions 38021591747 成功，crates.io 已确认 0.2.0 发布。框架 SHA-256 为 5df5e67abbe4048954852921149157722422859df99f07ee71b6bfb8b2900b4d。功能与边界见 TrafficProcessing.md；未验证最低系统、其他平台、真实浏览器信任、TUN 或网关。

## 0.3.0 发布准备

TUN 基础检查层新增功能，Rust 46 项、严格 Clippy、真实 Swift FFI 五组、内核 SwiftFormat 与 SwiftLint 均通过。包内容白名单收窄到 src/**/*.rs，排除 Finder 元数据。当前只提供数据包结构与校验和检查；真实 TUN 转发、返回包和系统隧道未实现。具体边界与验证见 TUN.md。源码提交后从干净副本预检并发布，二进制与版本标签由工作流生成，再记录实际发布结果。

## 0.3.0 发布核验

构建与 crate 源码提交 6562dadd169238fc867def9ebfb81ece105d1c8c，最终标签 e40f55af5ffd919484ade300dcb4828f0ce52ee0 仅更新 Package.swift 校验值；Rust 与发布包源码相同。干净副本的 .cargo_vcs_info.json 已核对，包列表不含 Swift、二进制或 Finder 元数据。crates.io 实际上传且索引可用；GitHub Actions 38023539148 全部成功，包含 Rust 46 项、真实 FFI 五组与远程 Swift 包 11 项测试。

框架 SHA-256：8800e08f820d79ec9db6b09386bb1e92b66c2499ec5bc0455a54de0b9019dc33；测试服务 SHA-256：3336f61db0c2820ba8060d0de5eac02afe3bbabb4eb33911bcb5c7489d4c30b5。仅 macOS ARM64；无系统 TUN、TCP/UDP 转发或 DNS 互通验收。

优先顺序纠正：检查基础交付后暂停后续 TUN 转发，先完成 DNS、路由、共用出站基础，再推进系统 TUN、网关和 SOCKS/HTTP；当前连接中的系统域名解析不能视为独立 DNS 模块，规则配置模型也不能视为执行路由。

## v2.3.0 统一整数错误协议发布准备

新增固定 u16 错误代码、显式结构化 ABI 扩展、受限诊断及查询表自动检查，按功能版本递增为 2.3.0。原 ABI 数值、旧 JSON 和正式规则算法保留兼容；Rust ProtocolViolation 变体与 Job/Capture 可选字段为已授权源码兼容变更。源码、crate、框架及远程 Swift 包必须对应同一来源；生成校验值提交仅修改下载元数据。当前错误协议见 [错误代码查询表](错误代码查询表.md)，未完成事项见 [待办清单](TODO.md)，不把发布视为全平台或系统 VPN 验收。

错误契约的 JSON 基线必须列入 Cargo 白名单；error_contract 使用显式 Tests 路径，避免 macOS 不区分大小写掩盖其他平台漏跑测试。CI 核对发布包内的源码、测试、基线和查询表文件集合，发布前解包运行错误契约测试。旧发布候选已停止，不复用不完整清单构建的产物。

## v2.4.0 类别分段错误体系修复发布

用户已要求现有错误接口直接迁移。四位整数前两位为类别、后两位为具体错误；现有命令、查询、TUN、后台任务统一新方案，ABI 版本 2，Rust/Swift 使用受限上下文。旧错误编号不兼容，必须整体更新二进制和客户端，回滚也整体还原。新错误契约与用户全文约束、独立回归一起发布，不将已有 H1、Debug 执行顺序或宿主认证未实现计为修复完成。
