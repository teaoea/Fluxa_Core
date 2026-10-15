# TUN 数据包与 UDP 转发

当前 dev 自研 TCP/UDP 数据包会话及资源/关闭契约见 [TunTCP.md](TunTCP.md)。下文保留历史阶段与发布证据，不表示这些旧限制仍是当前 dev 能力边界；系统 VPN 仍未交付。

作者：可莉唯一的狗。新增 UDP 转发版本：v1.9.0。

## UDP 第一阶段

`tun::udp::Session` 接收一个完整 IPv4/IPv6 UDP 包，调用共用 `outbound::Manager::connect_configured` 执行路由与出站。返回数据由 Rust 生成反向 UDP/IP 包，包含正确长度、校验和、TTL/Hop Limit；不要求一次请求只有一个响应，保留空 UDP 报文。输入不是 Ethernet 帧或 utun 地址族前缀。

会话选项为 Rust/C ABI 控制接口参数，不新增主 YAML 配置字段：MTU 1280–9000，单会话流数 1–32（默认 16），输出队列 1–64（默认 32），发送/建连超时 1–60000 ms（默认 3000），等待下次响应的空闲超时 1–60000 ms（默认 30000）。默认最多 8 个会话、全局 64 个 TUN UDP 流；共用出站的 128 个全局连接、单管理器连接上限及缓冲区预算同时生效。输出正文全局预算 4 MiB，按所有权归还；每个流最多一个等待入队的返回包，队列满时停止读取、形成有界背压。UDP 操作系统接收缓存仍可能丢报，不能承诺可靠传输。

一个会话只允许一个提交在途，并发提交明确返回配额错误，无输入后台队列或每包任务；每个流只有一个有上限的接收任务。异步接收错误进入有界结果队列，不伪装为“暂无数据”。等待响应超时正常回收流；关闭取消建连/读写、解除队列背压、清空输出并关闭所属连接，不停止共享管理器的其他连接。关闭可重复，关闭后拒绝使用；析构执行同样的回收。

流按源/目标地址、端口和映射代次区分。已有流保留原网络/路由快照，新流使用新的有效快照。Fake-IP 必须提供所属解析器有效代次，由共用出站还原域名并查询真实地址；没有代次或映射失效时拒绝，绝不连接合成地址。本轮不截获 DNS 包来生成映射，不自动推断代次。

TCP、分片、IPv4 选项、IPv6 扩展头、ICMP、超 MTU 报文明确拒绝；不切片、不截断伪造成功。VLESS UDP/XUDP 尚未实现，代理 UDP 明确失败，不能降级直连。广播、组播、未指定地址和端口零拒绝。

## 二进制 FFI 与宿主

能力位 21 仅表示 UDP 数据包转发，不表示系统 VPN。`tun_udp_open` 严格校验选项；`tun_udp_close` 可以越过普通请求配额取消会话。数据面使用 `fluxa_engine_tun_submit` 和 `fluxa_engine_tun_receive`，不使用 JSON/Base64。输入指针只在同步调用期借用；接收返回统一资源句柄，复制后必须 destroy。错误和队列空状态分开；详见头文件状态码。Swift `NativeTunUDPClient` 仅封装配置、参数、二进制借用与资源释放，不实现协议逻辑。

App、CLI 与未来网关可使用同一个 Session；不创建系统接口或改变系统网络。`tun_start`、`fluxa_engine_start` 和 App PacketTunnelProvider 仍明确拒绝系统隧道启动。后续顺序为 TUN DNS 衔接、TCP 流处理、macOS Network Extension，之后网关与其他入口。

## v1.9.0 发布记录

确定源码提交 52c35f601b5c419705977c54d9f41ef156b4171f；最终标签 v1.9.0 为 a20cf85e6fdc376a563aa92a3520ddbd8a897052，仅下载校验值不同，Rust/C ABI/Swift 源码相同。crates.io 实际源码包的 .cargo_vcs_info.json 已核对，包含 UDP 实现，无 Swift 源码或原生二进制。框架 SHA-256：750883eee458d53c80a28ef1f7274648bf30d4259ff5f77cccd350fa1952d582；独立测试服务 SHA-256：1173e2efa1468ba63a4a37fd49b2de30cfb5af64adfe5427f1b1960c0646771e。

GitHub Actions 38196503205 第二次全部通过，包含远程二进制下载与 Swift 门面验证。第一次已有 DNS 服务 FFI 报“网络连接或读写失败”；本机独立复测及同源码远程重跑通过，原因尚未确认，保留首次失败记录。调试 FFI 的 CI 链接有既有部署版本警告；交付框架按 macOS 14 部署目标构建，不将其视作最低系统运行验收。

## v1.9.0 验证范围

本机 Rust 全量 155 项、严格 Clippy、真实 Swift FFI 全部 13 组与独立协议互通通过。新增 Swift 文件与修改门面通过 SwiftFormat/严格 SwiftLint。

Rust 新增七组测试：真实 IPv4/IPv6 UDP、空报文、多响应、返回校验、规则拒绝与缺少配置、无效包/截断、流配额、队列背压、空闲回收、更新新旧快照、Fake-IP 有效/缺失/过期代次、超 MTU 错误、并发接收与关闭以及 40 次回收。独立 Swift FFI 使用真实回环 UDP 服务、自行构造包并校验返回首部/伪首部校验和、空包、拒绝与 40 次生命周期。

Fake-IP 测试最初只监听 IPv4，但自动地址候选先使用 IPv6，导致测试等待；已修正独立对端为实际 IPv6 候选，并给接收加截止时间。完整 FFI 回归发现 HTTPS 在宿主后台小栈上溢出，已将该请求 future 移到现有有界 Rust 运行时执行；不增加请求配额或逐包任务。

验证以本机 macOS ARM64 为主；没有实际系统 TUN 收发、最低 macOS 14、Intel 或其他平台运行验收。不能将回环 UDP 与数据包向量测试称作系统 VPN 已通过。

---

## 历史 v0.3.0 数据包检查阶段

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
