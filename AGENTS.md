# Fluxa 内核开发约定

- 版本号统一采用 `v主版本.功能版本.修复版本`：初始为 `v0.0.0`；新增功能递增功能版本并将修复版本归零，功能版本从 0 到 9，`v0.9.x` 的下一功能版本为 `v1.0.0`；仅修复 bug 时递增修复版本。标签带 `v` 前缀，Cargo 与 App 数值版本不带 `v`。详见 `Versioning.md`。

- Rust crate 命名 libfluxa_core，网络引擎、C ABI、Swift FFI 门面与测试直接位于本仓库根目录；SwiftUI App 位于相邻 Fluxa 仓库，不复制到本仓库。
- 自研 Rust 实现网络处理，禁止引入 libXray、Xray-core 或 sing-box。交付 FluxaEngine.xcframework，内部静态二进制名 libfluxa_core，无扩展名。
- 复用公共协议、资源和生命周期逻辑；业务模块禁止 unsafe。仅必要 FFI 边界可使用带中文安全说明的不安全操作，不持有跨语言业务裸指针；资源、任务、连接和缓冲区必须有上限。
- 配置采用 YAML。说明性注释、文档与提交信息使用中文；Swift 遵循 Apple API 设计、Google Swift Style Guide、SwiftLint 与 SwiftFormat。
- 每项功能分别实现、验证真实客户端 FFI、提交和推送 dev；不再创建功能分支。未经用户明确说明对应功能真机通过并允许合并，禁止合入 main；测试通过不能代替授权。
- 本仓库自有代码采用 MIT，声明保留在 ./LICESE.md；第三方依赖遵循上游协议，未完成审计不得宣称没有许可风险。
- 不提交 CA 私钥、节点凭据、流量导出、缓存或大型原生二进制。未完成能力明确拒绝，不伪造成功或回退直连。
- 功能顺序：HTTPS、MITM、JS、规则流量处理、DNS、网关、代理协议。macOS 优先，其他平台后续；编译、集成测试、最低系统、真机与审核分别记录。

- App 禁止本地目录依赖内核，只能引用远程包和固定 URL/校验值的二进制库。crates.io 发布只包含 Rust 源码、C ABI 声明、Rust 测试与指定文档，不包含 App、Swift 源码、缓存或二进制。发布必须验证内容和认证，不伪造上传成功。

- 版本约定：初始 v0.0.0；新增功能递增次版本（如 v0.1.0、v0.9.0），主版本发布使用 v1.0.0；仅修复 bug 递增修订号（如 v0.0.1）。Git 标签带 v，Cargo/Swift 包版本不带 v。发布后同版本不可重用。
