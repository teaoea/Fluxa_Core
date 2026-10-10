# libfluxa_core 发布说明

Rust 包名与 Rust 库名均为 libfluxa_core，版本 0.1.0。对外 C ABI 的 fluxa_engine_ 前缀保留兼容；Swift 产品 FluxaEngine、FluxaCore 与 FluxaEngine.xcframework 名称不变，内部静态二进制为 libfluxa_core，无扩展名。

crates.io 发布内容由 Cargo.toml 的 include 白名单限定：Rust 源码、C ABI 头、Rust 测试、测试例程和指定文档；不包含 Swift/App 源码、钥匙串、真实证书私钥、缓存或二进制。先执行 cargo package --list 与 cargo publish --dry-run --registry crates-io，再上传。凭据只通过 cargo login 或受保护环境变量配置，不写入仓库和聊天。

GitHub Release 用于 Apple 二进制下载；crates.io 用于 Rust 源码库，不承载 XCFramework。发布工作流在 dev 的 Release.version 变更时执行 Rust/真实 FFI 测试、构建 macOS ARM64 静态框架、计算压缩包 SHA-256、更新 Package.swift、提交 dev 并创建版本标签和下载附件；不合并 main。Swift 包不提供本地目录回退。App 固定远程包 0.1.0，通过 URL 与 SHA-256 下载二进制。

当前只提供 macOS ARM64 切片；其他 Apple 平台明确不可用。发布失败必须记录，不能把本地构建或 GitHub 上传称为 crates.io 上传成功。
