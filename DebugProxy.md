# Rust 调试服务与静态库接入

## 职责与产物

Rust 负责普通 HTTP 代理、CONNECT、精确域名 HTTPS MITM、上游证书验证、请求执行、声明式改写、连接及资源管理。SwiftUI 负责页面与展示；Swift 平台层只负责 Keychain、证书文件导出、启停调用和 Apple 必需接口。

macOS RequestRunner 与 RustDebugProxy 已通过实际 C ABI 调用内核。旧 Swift HTTPProxyServer、ProxySession、CertificateAuthority、RewriteProcessor 及 HTTPCodec 已移除。订阅下载和 IP 查询目前仍使用旧 Swift 服务，后续单独迁移；本次没有实现 SOCKS、TUN、DNS、网关或代理出站协议，0.2.0 已将受限内置 JS 接入 HTTP/HTTPS 响应管线，完整规则路由仍待开发。

Swift 产品名为 FluxaEngine，客户端使用 `import FluxaEngine`。Rust crate 保留内部 rlib 供测试；对外交付 `FluxaEngine.xcframework`，内部是 `libfluxa_core.framework`，真正的二进制文件名是 `libfluxa_core`，没有扩展名且内容为静态 archive。Cargo 自动生成的中间 archive 文件在打包时改名，不作为最终交付名称；不编译 cdylib。

```sh
bash ./build-macos.sh
bash ../Fluxa/Scripts/check.sh
```

当前构建器仅打包本机 macOS 架构，最低部署版本 14.0。Swift package 自动链接现有切片；App/Network Extension 后续通过同一包接入，但移动平台、其他架构切片与真实隧道尚未交付。缺少对应原生库时门面报错，不能宣称跨平台运行已验收。生成文件位于本仓库 ./Vendor/，不提交 GitHub；完整检查重新生成原生库，避免使用旧缓存。

## 请求与代理

C ABI v1 保持值类型句柄，能力位 3 表示通用 HTTP 调试。内部命令 request 支持 HTTP/HTTPS，https 保留只允许 HTTPS 的兼容行为；内部 JSON 只是 ABI 编码，用户配置继续使用 YAML。两个请求入口共享有界 HTTP/TLS 执行流程。

mitm_start 接收端口、精确域名、正文捕获开关、可选本机上游代理端口及改写列表。普通 HTTP absolute URI 转发和解密后的 HTTPS 共用 forward 流程。CONNECT 未选中域名只透传加密字节；选中域名先验证上游证书，再完成客户端 TLS。Host 必须与 URI/CONNECT 目标一致，防止跨主机请求。上游失败明确返回错误，不回退直连。

监听仅绑定 127.0.0.1。allow_lan 为 true 明确拒绝，LAN 认证与网关属于后续功能。没有隐式修改 macOS 系统代理、系统证书信任或路由。

保留原有精确域名加路径前缀声明式改写：拒绝、请求头、响应头、UTF-8 文本响应替换。受保护协议头不可改写，头部总量受 32 KiB/128 项限制；文本替换只处理不压缩的 text/json 正文，原文至多 1 MiB，先检查展开后长度再分配，结果至多 8 MiB。内置 JSON 响应脚本见 TrafficProcessing.md；完整路由及自定义脚本处理属于后续阶段。

HTTP/1.1、正文/解码 8 MiB、头部 32 KiB/128 项，60 秒会话/请求超时。不支持流水线、Upgrade、Expect、WebSocket、HTTP/2/3；不自动跟随重定向或解压压缩正文。请求编辑器在后台执行阻塞 FFI，超时有上限；尚未提供请求任务的立即取消接口。

## 身份与日志

Rust 生成并解析 CA；Swift 将私有身份存入独立 Keychain 项 mitm.rust.ca.v1，旧 Swift RSA CA 不转换或覆盖，用户需要重新信任新导出的公开 DER 证书。损坏的已保存身份会报错，不自动换 CA。测试不修改系统信任，私钥不写入测试输出或仓库。当前自定义上游根入口每次替换附加根，客户端适配器明确拒绝多根数组，避免静默只保留最后一个。

Rust 管理有界捕获队列，Swift 每轮最多消费 64 条，通过 peek/ack 确认消费后转为页面 Exchange。正文捕获开关关闭时只保留长度；MITM 明文与未解密 CONNECT 记录明确区分。队列达到数量/内存上限时计数丢弃；当前客户端尚未显示该计数。页面 startedAt 由采样时间和持续时间估算，不宣称精确网络开始时间。导出 HAR/JSON 复用既有展示逻辑。

## 验证范围

Rust 新增真实 HTTP/TLS 转发及改写测试，与已有 HTTPS/MITM/JS 回归测试共用一个 Rust 回环服务。Swift 客户端集成测试直接调用 Rust 库，验证 HTTP/HTTPS、证书拒绝、解密改写、公开证书导出、完整日志和停止行为；没有 Swift 网络服务器或模拟 FFI 替代。统一 FFI 检查另行验证生命周期、HTTPS、MITM 和 JS。

这里只记录本机 macOS ARM64 验证，最低 macOS 14、其他架构、Network Extension、浏览器系统信任、移动真机及 App Store 审核尚未验收。

## 共用服务连接管线（v1.3.0）

HTTP 主动请求、HTTPS TLS、普通代理转发、CONNECT 透传和 MITM 上游均使用同一受管理 TCP 流。每次连接按原目标执行显式 DNS、路由及候选地址检查，再登记连接配额、配置修订、字节计数和空闲时限；没有路由或域名 DNS 配置时明确失败，不调用系统域名解析回退。未实现代理出站仍拒绝。

管理流以安全 Rust 及 RAII 保持连接所有权，不向 TLS 暴露裸指针或复制网络流程。关闭全部连接、引擎销毁及空闲回收会唤醒正在等待的 HTTP/TLS 操作；流释放时取消并移除资源。TLS 流量计数为 TCP 线上字节，包含握手与密文开销，不等同于解密正文长度。请求取消不会绕过证书验证。

显式本机 HTTP 中转先检查原目标规则，再连接本机端口；若中转由 Fluxa 管理，其实际上游另执行自身配置。外部中转自行决定的最终 DNS 和地址不可由此客户端证明，不把预检查宣称为第三方代理的最终出口约束。CONNECT 透传仍保留端到端 TLS，不捕获明文；MITM 只解密指定域名。

本机 macOS ARM64：104 项 Rust 测试及九组真实 Swift FFI 回归通过，覆盖真实 HTTP/TLS 请求取消、空闲回收、拒绝/未实现代理不回退、重配置恢复、证书拒绝、双向 TLS、日志与脚本处理。严格 Clippy、SwiftFormat、SwiftLint 已检查。最低 macOS 14、其他架构/平台、系统 TUN/网关、GUI 与真机 VPN 未运行验收；此阶段不增加 TUN 转发。
