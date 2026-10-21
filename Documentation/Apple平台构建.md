# Apple 平台本地构建

最低版本与架构以 [强制规范](../AGENTS/全平台系统支持的最低版本.md) 为准：macOS 13.0、iOS/iPadOS 16.0、tvOS 17.0。仅 Apple Silicon Mac；排除 Intel Mac、Mac Catalyst、watchOS、visionOS。

## 本地使用

```sh
# 查看五种 ARM64 切片，不构建、不联网。
python3 Tools/build-apple.py --list
# 当前默认只构建 macOS arm64。预先安装对应 SDK 与 Rust target；缺少时失败。
bash build-apple.sh --output /private/tmp/fluxa-apple-build/FluxaEngine.xcframework --package-dir /private/tmp/fluxa-apple-packages
# 仅 macOS，适用于既有 macOS 独立本地附件流程。
bash build-macos.sh --output /private/tmp/fluxa-macos-build/FluxaEngine.xcframework
# 其他平台保留支持配置；等用户开发对应 App 并明确要求后，才用 --platform 显式选择。
```

输出路径必须不存在，失败不覆盖已有成品；全部选中切片成功后才移动最终 XCFramework。中间 Cargo 缓存位于忽略目录 target/apple，各 Rust target 隔离。静态库随包保存公开 C ABI 头文件、许可与第三方声明，不包含实现源码。

每个切片显式选择 SDK、Rust target 和部署变量，并清除继承的部署目标、SDK 和通用编译 flags，避免主机环境将最低系统抬高。framework 记录完整版本、源码提交、dirty 状态及 Rust target。开发中 dirty 产物不等于干净提交发布物。

iOS/iPadOS 共用 iOS 切片；iOS 模拟器、tvOS 模拟器各自独立，禁止把同架构的不同平台静态库混用。Mac Catalyst 不在支持范围。

只声明支持目标，不能据此宣称最低系统实际运行、签名宿主、Network Extension 或 VPN 已验收。具体交叉编译和运行证据另行记录。平台专有能力仍按原授权边界处理，不增加网络直连回退。

所有产物默认只保存在本地。没有用户明确许可，不上传附件、来源清单、草稿或发布标签；源码 main 提交不构成上传许可。既有 macOS 独立附件来源清单只声明 macos-arm64、minimum_macos=13.0；全平台本地产物不能冒用 macOS 单平台清单。

发行产物强制采用 fluxa-core-{os}-{arch}[-{abi}]，包内库名为 fluxa_core。--package-dir 仅为本次选中的目标生成单切片 ZIP 与 manifest.json、SHA256SUMS；不生成 Catalyst 包。Core 当前统一 main；旧脚本文件名中的 dev 仅是历史入口名称，不表示仍有 dev 分支。

支持列表和构建列表分别见强制规范：开发保持全部约定平台兼容，日常只实际构建 macOS arm64。--list 仅查看完整支持矩阵，不触发其他平台构建。
