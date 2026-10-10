# Fluxa 网络引擎

作者：可莉唯一的狗。

本仓库根目录同时包含跨平台 Rust crate、C ABI 声明与 Swift package。网络业务使用安全 Rust；Swift 只调用 FFI 和提供平台适配。

## 功能顺序与当前进度

1. HTTPS 解密：Rust 客户端已实现 TLS 1.2/1.3、系统根证书校验、HTTP/1.1 请求/响应、分块解码与错误处理；客户端 FFI 已验证。详见 [HTTPS.md](HTTPS.md)。
2. MITM：已实现 CONNECT、双向 TLS、精确域名匹配、CA/站点证书和有上限的请求/响应捕获；Rust 与客户端 FFI 均已验证。详见 [MITM.md](MITM.md)。
3. JS 引擎：已实现受限同步 JSON 转换、内存/栈/时间预算和执行隔离，Rust 与实际 Swift FFI 已验证。详见 [JS.md](JS.md)。
4. 按不同规则处理流量。
5. DNS。
6. 网关。
7. 代理协议，首个目标为 VLESS + TCP + REALITY + Vision。

每项功能单独验收、中文提交并上传开发分支，再推进下一项。没有明确授权不得合并 main。HTTPS 客户端不等于浏览器 MITM，不改变系统代理、路由或证书信任。当前不改 UI。

## 代码与内存

- `src/`：Rust 引擎与可复用 HTTP/TLS、错误、资源管理。
- `include/`：C ABI 和模块声明。
- `Sources/FluxaEngine/`：实际 Swift 客户端门面，平台未链接库时明确拒绝。
- `Sources/FluxaCore/`：暂时保留的共享 Swift 配置模型。
- `Tests/` 与 `examples/https_fixture.rs`：Rust 和真实 Swift FFI 的回环 HTTP/TLS 验证。

资源由 Rust 管理，外部只持有不复用句柄；释放对象在注册表锁之外执行。业务模块禁止 unsafe，FFI 仅有输入借用与输出复制两个受契约约束的内存边界，详见 [MemorySafety.md](MemorySafety.md)。自有内核 MIT，依赖版本见 [DEPENDENCIES.md](DEPENDENCIES.md)，内核开源协议声明见本目录 [LICESE.md](LICESE.md)。

```sh
cargo fmt --manifest-path ./Cargo.toml --check
cargo test --manifest-path ./Cargo.toml --offline
cargo clippy --manifest-path ./Cargo.toml --offline --all-targets -- -D warnings
bash ./build-macos.sh
swift test --package-path .
bash ./check-ffi.sh all
```

FFI 检查脚本当前使用 macOS 的 Swift 工具链；临时服务器仅绑定回环地址，测试证书不会安装到系统，所有临时输出退出时清理。测试不等于 UI、最低系统或真机验收。运行 `bash ./build-macos.sh` 可生成本机 macOS 架构的 `FluxaEngine.xcframework`（静态二进制 `libfluxa_core`，无扩展名），Swift package 会自动链接并运行实际 FFI 身份恢复和脚本测试；macOS App 请求编辑器与 HTTP/HTTPS/MITM 调试代理已切换到 Rust，迁移及打包方法见 [DebugProxy.md](DebugProxy.md)。构建缓存不上传 GitHub。其他平台或架构尚未打包验收。

## 独立库与 App 接入

本仓库目录为 FLuxa_Core，文件直接位于根目录，不保留 core 子目录。SwiftUI App 位于相邻 Fluxa 仓库，使用 Swift package 路径 ../FLuxa_Core 引入 FluxaEngine 产品；该产品静态链接 Vendor/FluxaEngine.xcframework。App 内不保留 Rust 内核源码副本。

```text
Documents/
├── Fluxa/                 # SwiftUI App
└── FLuxa_Core/
    ├── Cargo.toml         # Rust 库
    ├── Package.swift      # Swift 库门面与静态框架接入
    ├── src/
    ├── Sources/
    └── Vendor/            # 本机构建产物，忽略提交
```
