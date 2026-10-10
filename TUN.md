# TUN 数据包基础

作者：可莉唯一的狗。版本：v0.3.0。

## 本阶段能力

Rust 的 `tun::parse` 接收一个完整 IP 数据包和 MTU；不接受 Ethernet、utun 地址族前缀或多个拼接包。返回 IP 版本、源/目标地址、TCP/UDP 端口、包长与负载长度。IPv4 校验首部，TCP/UDP 校验伪首部与正文；IPv4 UDP 可省略校验和，IPv6 UDP 不允许。输入长度必须与 IP 和 UDP 声明长度一致。

遵循 [RFC 791](https://www.rfc-editor.org/rfc/rfc791)、[RFC 8200](https://www.rfc-editor.org/rfc/rfc8200)、[RFC 9293](https://www.rfc-editor.org/rfc/rfc9293)、[RFC 768](https://www.rfc-editor.org/rfc/rfc768) 的基本包格式与校验和定义；这不代表完整实现这些协议的状态机。

`PacketSession::new` 限定 MTU 1280–9000；`inspect` 只更新成功包计数，错误不改变计数；计数溢出拒绝，close 可重复且关闭后拒绝使用。会话不保存正文，不创建队列、线程或套接字，不产生返回包。`forward` 明确返回 Unsupported。

IPv4 分片、IPv4 选项、IPv6 扩展头/分片/jumbogram、ICMP 和其他传输协议暂不支持。TCP 当前只检查头长度与校验和、提取端口和负载长度；不解释选项或建立 TCP 状态机。无重组、重传、路由、DNS 查询或 NAT。

## FFI 与平台分工

复用 ABI 1 的 `fluxa_engine_command`、结果句柄与释放入口，不增加裸指针业务对象。

- packet_open：创建一个检查会话，mtu 必填；重复创建拒绝。
- packet_inspect：传入 Base64 编码的一个 IP 包，返回 packet_info；编码输入上限 12000 字节，原始包受 MTU 限制。
- packet_close：释放会话，可重复；关闭后检查拒绝。
- tun_start：始终明确拒绝。本版本不会更改系统网络设置。

一个引擎同时只有一个检查会话；不同调用复用现有 busy 排他与句柄注册表（全局最多 64 个引擎/结果资源）。输入仅同步借用，Rust 解码临时正文在返回时清零释放，结果只包含元数据，现有返回缓冲区总预算 32 MiB 保持不变。直接 Rust API 会话通过所有权释放，不持有外部内存。

Swift 门面 NativeTunPacketClient 只做参数打包、FFI 生命周期与结果解码。能力位 5 表示数据包检查，不表示 TUN 转发。平台适配未来负责 NE 生命周期、配置虚拟接口及 packetFlow 读写；IP/TCP/UDP、路由、DNS 与返回包生成保留在 Rust。

## 验证

Rust 46 项测试全部通过，包含四种 IPv4/IPv6 TCP/UDP 组合、奇数字节负载、逐字节截断、校验错误、错误头长度、分片/扩展头、IPv4 选项、MTU、关闭和计数溢出。额外对 0–9000 字节有界伪随机输入验证不会 panic。

真实 Swift FFI 五组通过，TUN 组使用独立生成的 RFC 向量，不依赖 Rust 被测函数生成校验和；包含四种包、每个包逐字节截断、正文损坏、未实现转发拒绝和 100 次会话创建/释放。现有 HTTPS/MITM/JS 与句柄生命周期回归通过。Clippy 严格检查通过。

只验证当前 macOS ARM64。数据包向量测试不能代替实际 TUN 收发或 TCP/UDP/DNS 服务互通；最低 macOS 14 运行、Network Extension、签名权限、真实系统流量、Intel 与其他平台未测试。

## 后续顺序

下一阶段实现 UDP 转发与 DNS，再实现 TCP 状态机/返回包、共用路由和出站，最后接入 macOS Network Extension。数据包输出接口随真实转发实现提供；本阶段不放入空队列或回显包伪造能力。
