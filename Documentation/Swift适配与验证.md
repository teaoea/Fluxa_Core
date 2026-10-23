# Swift 适配与验证

Swift 源码、模型、测试和 C ABI 封装归 Fluxa App 仓库 `Packages/FluxaKernelBridge/`，内核仓库不提供 Package.swift 或 Swift 测试目标。原生模块为 `nodus_core`，框架容器为 `NodusCore.xcframework`。

## 当前二进制接入

由宿主复制已核对来源和 SHA-256 的框架到 App 项目内，使用项目内路径链接 `nodus_core.framework`；C 头文件为 `nodus_core.h` 和 `nodus_debug_ipc.h`，函数采用 `nodus_core_*`。缺少匹配二进制时明确失败，生产构建不能读取外部 Rust 源码、触发 Cargo 或回退旧包。

## 旧检查入口的实际状态

旧 `check-ffi.sh` 已迁至 Fluxa 的 `Packages/FluxaKernelBridge/check-ffi.sh`，Swift 测试也在该仓库；在内核根目录运行 `bash ./check-ffi.sh` 或 `swift test --package-path .` 已无效。

现有 Fluxa 脚本仍校验旧头文件并引用旧符号，尚不能用它宣称当前 Nodus ABI 已通过整套 App 联调。迁移应在 App 任务完成，再显式指定受测内核目录及固定成品、实际运行对应套件。脚本的存在不等于能验证当前 ABI；本次文档修复不修改 App 源码。

历史 Swift 联调来源见 Fluxa 的 [Swift 适配层迁移说明](../../Fluxa/Documentation/Swift适配层迁移.md)。旧运行结果只适用于原受测源码和二进制，不能替代当前版本、最低系统、签名、Network Extension 或系统 VPN 验收。

## 当前验证边界

Rust 执行入口见 [测试迁移与执行说明](测试迁移与执行说明.md)。需要当前 ABI 的宿主验证时，应从本轮真实构建产物导入 `nodus_core` 并实际链接执行；只改导入名或成功编译不能替代运行结果。
