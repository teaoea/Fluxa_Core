# libfluxa_core 发布说明

Rust 包名与 Rust 库名均为 libfluxa_core，版本 0.2.0。对外 C ABI 的 fluxa_engine_ 前缀保留兼容；Swift 产品 FluxaEngine、FluxaCore 与 FluxaEngine.xcframework 名称不变，内部静态二进制为 libfluxa_core，无扩展名。

crates.io 发布内容由 Cargo.toml 的 include 白名单限定：Rust 源码、C ABI 头、Rust 测试、测试例程和指定文档；不包含 Swift/App 源码、钥匙串、真实证书私钥、缓存或二进制。先执行 cargo package --list 与 cargo publish --dry-run --registry crates-io，再上传。凭据只通过 cargo login 或受保护环境变量配置，不写入仓库和聊天。

GitHub Release 用于 Apple 二进制下载；crates.io 用于 Rust 源码库，不承载 XCFramework。发布工作流在 dev 的 Release.version 变更时执行 Rust/真实 FFI 测试、构建 macOS ARM64 静态框架、计算压缩包 SHA-256、更新 Package.swift、提交 dev 并创建版本标签和下载附件；不合并 main。Swift 包不提供本地目录回退。App 固定远程包 0.2.0，通过 URL 与 SHA-256 下载二进制。

当前只提供 macOS ARM64 切片；其他 Apple 平台明确不可用。发布失败必须记录，不能把本地构建或 GitHub 上传称为 crates.io 上传成功。

## 版本与作者

作者署名为可莉唯一的狗。版本使用 v主版本.次版本.修订号：初始 v0.0.0，新增功能递增次版本，bug 修复递增修订号；主版本发布使用 v1.0.0。Cargo/Swift 元数据不带 v，Git 标签和发布标题带 v。0.1.0 已经 crates.io 实际上传并确认索引；后续修改需要新版本，不能覆盖已发布版本。

0.1.1 为 README 文档修订，移除客户端与开发流程说明，只保留网络内核介绍；网络实现与 0.1.0 相同。

每项功能的开发、验证、源码提交、同版本发布、客户端更新与验收顺序遵循 AGENTS.md。发布记录同时记录构建源码提交和最终版本标签；生成校验值提交只更新下载元数据，不改动已打包的 Rust 源码。


## 0.1.1 发布核验

构建和 crate 源码提交为 5f47b9afee5dd82b3322b8406db5b704b7e0f4af；最终 v0.1.1 标签为 e7aa631d818c3dc2ab4a9ab3b064b6f0b9b182a9，二者仅 Package.swift 校验值不同，Rust 源码完全相同。crate 的 .cargo_vcs_info.json 已核对。

Rust 36 项测试、Clippy、真实 Swift FFI 四组与远程 Swift 包 10 项测试全部通过；GitHub Actions 运行 38018885504 成功，crates.io 已实际发布 0.1.1。框架压缩包 SHA-256 为 3cd9a441ca55e404914cb5358c42fc6d4267ada8f84bbc381121aead04a3f29e。仅验证 macOS ARM64，最低系统、其他平台、真机、系统浏览器信任与远端代理互通仍未验收。


## 0.2.0 发布核验

新增受限流量响应脚本，版本按功能号递增。构建及 crate 源码提交 e7b7d75e2b2994b98bde14b11d289c743e87a9a5，最终 v0.2.0 标签 e92801bd9af33e1f22c9c2591e9094c8f59165ba 仅更新 Package.swift 校验值，Rust 源码相同。发布从该确定提交的干净副本进行，避免 Finder 元数据被 src 通配符纳入 crate；实际包列表与 .cargo_vcs_info.json 已核对，不使用 allow-dirty 绕过来源校验。

Rust 38 项、Clippy、真实 FFI 四组、远程 Swift 包 10 项全部通过；GitHub Actions 38021591747 成功，crates.io 已确认 0.2.0 发布。框架 SHA-256 为 5df5e67abbe4048954852921149157722422859df99f07ee71b6bfb8b2900b4d。功能与边界见 TrafficProcessing.md；未验证最低系统、其他平台、真实浏览器信任、TUN 或网关。
