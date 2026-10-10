# 共用出站基础

作者：可莉唯一的狗。v0.8.0。

本阶段提供数字地址 TCP 字节流及连接式 UDP，Rust 公共 API 为 outbound::Manager，复用稳定 C ABI 与 NativeOutboundClient。只接受明确的数字 IP 和非零端口，拒绝未指定地址，不解析域名，不自动执行路由。Policy::Direct 创建连接，Reject 返回 Rejected，Proxy 返回 Unsupported，绝不隐式直连。

TCP 独立读写锁，支持并发双向传输、写端半关闭和读 EOF。UDP 保留完整报文边界，支持空报文，连接式 socket 仅接收目标对端的报文；收到的报文超过调用者上限时整包丢弃并明确报错，不以截断数据伪造成功。TCP 读取返回当前可用的一段数据，不承诺完整应用响应；UDP 没有 EOF 或半关闭。

每次打开、读、写、半关闭的截止时间为 1–60000 ms，包括锁等待。TCP 读超时可继续读取；写超时或网络错误会使连接失效，不能重新发送导致重复写入。close 可从另一调用取消等待中的读写；close_all 可重复，后续允许创建新会话；stop 为永久停止，取消所有会话及正在打开的连接。单会话重复关闭明确拒绝过期标识。FFI 引擎销毁也执行 stop，不保存调用者内存地址。

每个管理器最多 64 个已登记会话，跨管理器最多 128 个正在打开或存活的连接；通过所有权预算归还。标识单调递增、不复用。单次 TCP 数据最多 65536 字节，UDP 发送最多 65507 字节；接收缓冲区全局预算 16 MiB，释放时清零。FFI 读写最多八个普通调用，小型关闭命令可绕过等待配额，避免阻塞读占满后无法取消。没有后台队列或无限任务；尚无自动空闲淘汰，显式关闭或析构释放。

Rust 测试使用独立 TCP/UDP 对端，覆盖二进制数据、双向读写、半关闭/EOF、IPv4/IPv6、UDP 空包和过大报文、无触网拒绝、读超时恢复、真实背压写超时失效、取消、64 会话限额和 100 次回收。真实 Swift FFI 覆盖实际对端收发及另一线程取消，不通过 Swift 网络实现或假结果验证。全部 77 项 Rust 测试、严格 Clippy、九组真实 Swift FFI、SwiftFormat/严格 SwiftLint 通过。

当前只交付共用出站接口和短会话诊断。DNS、Fake-IP、路由执行、出站注册表与活动配置规则将在后续按顺序接入，再替换现有 HTTP/HTTPS/MITM 的连接路径。尚未实现代理协议、系统 TUN/网关；本轮不能作为其验收。最低 macOS 14 和其他架构/平台尚未运行。

## 发布及客户端验证（2026-10-10）

内核 v0.8.0 构建及实际 crate 来源 a3bb95dd4ef0d84023d01ca4b93a8e7b7cb554ac，发布标签 ea002a30cf7eb9c9a403478f9724db8c9016db88 仅更新二进制校验值；下载包核对 56 个发布文件。GitHub Actions 38040660839 成功，77 项 Rust 测试、严格 Clippy、九组真实 Swift FFI、静态框架及远程包验证通过；发布前 SwiftFormat/SwiftLint strict 通过。

框架 SHA-256 98e63cc69761987b75310c8adb7bf10c1b3518859d6ef697e698670832c7080a；测试服务 SHA-256 ba11733888b031d1524066c79dcf3c68f219f6def617bc86f52cb363a32d78cb。Fluxa 固定远程 0.8.0，下载并校验后运行 45 项客户端测试，零失败、无跳过；包括真实 TCP/UDP 二进制数据、空报文、拒绝策略和失败恢复。App 0.6.0 / build 6，SwiftFormat、SwiftLint strict、资源一致性及 Xcode 27 包解析通过，无本地 Rust 构建或目录依赖。

数字地址出站支持 TCP 独立读写、半关闭/EOF、连接式 UDP 完整报文、截止时间及取消；真实对端另验证 IPv4/IPv6、超时恢复、TCP 背压写超时使连接失效、会话上限与回收。Fluxa 工具提供手动短会话测试，原始数据只在本次操作中使用。此版本不解析域名、不自动执行路由；HTTP/MITM 仍待后续共用出站接入。

测试环境 macOS 27.2 ARM64 / Xcode 27.0。尝试读取原生界面仍报告 Mac 锁定，本轮 GUI 操作未验收；最低 macOS 14、其他架构/平台、TUN/网关和代理协议均未运行验收。不合并 main，保留用户两张首页标注图。

## 配置 DNS 到域名出站（v0.9.0）

新增 Manager::connect、outbound_connect C ABI 与 NativeOutboundClient 域名接口。必须先配置 DNS，使用既有 UDP/TCP/DoT/DoH 解析、缓存及 DNSSEC 验证；不调用系统解析或 hosts，不自动改变系统配置。输入域名、非零端口、TCP/UDP 和 auto/ipv4/ipv6 地址族，结果包含会话标识及实际连接地址。拒绝及未实现代理策略在 DNS 查询前返回错误。

打开截止时间覆盖 DNS 与全部候选连接。自动地址族交错 IPv6/IPv4 候选，每 250 ms 启动下一项，上一项全部失败可立即尝试；最多 16 个地址，成功只登记胜出会话，取消或失败释放所有候选。TCP 成功表示连接已建立；UDP 建立 socket 不验证对端可达，选定地址后不会自动重发或切换，避免重复业务报文。close_all/stop 可取消正在解析或连接的操作。

NXDOMAIN、空记录、DNSSEC 失败、合成 Fake-IP 明确拒绝；DNSSEC/资源/输入错误不能被另一地址族掩盖。Fake-IP 还原与路由执行属于后续独立交付。现有 HTTP/HTTPS/MITM 尚未迁移到本流程。当前验证只覆盖本机真实协议夹具，不代表公网服务或最低系统运行验收。

本轮本机验证：新增六项域名出站真实套接字测试，全量 Rust 共 83 项通过；严格 Clippy、SwiftFormat、严格 SwiftLint（零违规）及九组真实 Swift FFI 通过。环境 macOS 27.2 ARM64；最低系统及其他平台未运行，不代表 TUN/网关或公网互通验收。

## DNS 接入出站（2026-10-10）

内核 0.9.0 来源 8c58e3673ddfc4bc6802b660a25b6d8cc38e551e，最终标签 df56bd7b1236059ea539088d7ec58c783fb52de5。GitHub Actions 38044725276 成功；83 项 Rust 测试、严格 Clippy、九组真实 Swift FFI 和远程 Swift 包 11 项测试通过。实际 crates.io 包 58 个文件，VCS 提交一致，未包含 Swift 或二进制。发布前 SwiftFormat、严格 SwiftLint 零违规。

框架 SHA-256 5a797598ef2a51babbf287f6e614bba2e0432a46ffa32a6020918fdceee861ef；测试服务 SHA-256 67233506b4010e602a5f20df3cad7df84ab14b66bcfe8479435ac3d6decc3966。Fluxa 0.7.0 / build 7 固定远程内核 0.9.0，下载并验证二进制，46 项客户端测试通过、零失败及跳过；新增 YAML → UDP/TCP/DoT/DoH → 实际 TCP/UDP 收发，并验证 Fake-IP 连接拒绝。Xcode 27 解析固定版本通过，SwiftUI 页面经 Swift 包编译，无本地 Rust 构建或目录引用。

域名连接覆盖 CNAME、正缓存、自动/强制地址族、整体截止时间、解析取消、DNSSEC 失败、无效输入及拒绝策略不触发 DNS。UDP socket 创建不证明目标可达，不重发/切换地址。Fake-IP 连接、路由执行和现有 HTTP/MITM 迁移仍待独立交付。

本机 macOS 27.2 ARM64。最低 macOS 14 实际运行、其他架构/平台切片、GUI 交互、真机 VPN、系统 TUN/网关及远端代理协议未验收；本轮不宣称已具备 VPN。未合并 main，用户首页标注图不加入提交。
