# 自研 TCP/TUN 数据包会话

作者：可莉唯一的狗。

本阶段在 dev 实现有界 TCP 流与返回 IP 包，与 UDP 共用 DNS/Fake-IP、规则及出站；不引入外部 TCP 栈。本阶段不创建系统 VPN，不绕过《VPN启动技术规范》中的检查、确认和四阶段接管。`tun_start` 与 `fluxa_engine_start` 仍明确拒绝系统隧道启动。

## 接口与职责

Rust 的 `tun::session::Session` 提供 `new`、`submit`、`receive` 和异步 `close`；TCP 状态机、报文编解码与流工作任务保持内部可见，未为测试扩大公开 API。旧 `tun::udp::Session` 入口继续复用同一 UDP 实现。

C ABI 仍为版本 2，新增能力位 28 表示 TCP/UDP 数据包会话，位 25 的结构化错误协议含义不变。控制命令新增 `tun_open` / `tun_close`；包数据继续走既有 `fluxa_engine_tun_submit` / `fluxa_engine_tun_receive`，不使用 JSON/Base64 搬运包。成功提交只表示通过校验并进入有界队列，不代表远端连接或系统 VPN 成功。

Swift 封装及测试仅位于 Fluxa 的 `Packages/FluxaKernelBridge/`。`NativeTunClient` 只传配置、提交/读取字节及关闭意图，不实现 TCP、DNS、规则、状态机或资源管理。生产构建仍只使用固定远程二进制；没有能力位 28 的旧二进制明确拒绝新入口，不因 Swift 源码存在就声称功能已安装。

## TCP 与 DNS 行为

- IPv4/IPv6 端点、校验和、头部长度、MSS 与选项边界严格校验。
- 处理 SYN/SYN-ACK/ACK、32 位序号回绕、协商窗口扩大、乱序及重复数据、分段、部分确认、重传、拥塞窗口、零窗口探测、FIN/RST、半关闭与 TIME-WAIT。
- 重复 SYN 立即重发仍计入五次重试预算；半开流有包含出站准备的独立 30 秒握手上限，无效入包不能刷新空闲期限。
- RTT 估计与 RTO 使用 RFC 6298 的平滑/偏差及 Karn 原则；默认初始 1 秒、RTO 上限 60 秒，超时指数退避，最多五次重传后明确失败。SYN 超时后的数据 RTO 使用 3 秒起点，新的无歧义样本可更新它。
- TCP DNS 仅处理配置的虚拟 DNS 端点，必须收齐两字节长度和完整查询帧，再调用同一所属解析器；处理多次查询但每流最多保留一份待发送响应。虚拟 DNS 生成响应与普通 TCP 上游路径分离。
- Fake-IP 必须属于当前绑定解析器和有效代次，恢复域名并通过配置 DNS 获取真实候选，再执行既有规则语义；返回包保留原合成地址。不能连接合成地址，也不把新配置用于旧流的重拨。
- 流开始捕获一致网络快照；实际出站建立后释放这份完整快照，连接保留既有必要决策。新流使用新配置。
- 规则拒绝、DNS/映射失效、连接错误、资源不足和关闭状态按原统一错误体系报告，不降级 DIRECT。规则拒绝和技术失败保持区别。

入站有界 IP 分片重组及 IPv6 扩展头处理见 [IP 重组说明](IPReassembly.md)。

未实现：紧急数据、TCP Fast Open 数据型 SYN、时间戳、ECN 协商及原始 ICMP Echo 出站。出站 UDP IP 分片、SACK 与窗口扩大已有独立说明。未知选项校验其长度但不声明协商；不将此有界实现称为完整 RFC 合规 TCP 栈。TIME-WAIT 默认 30 秒，可在下述范围配置；不据此承诺跨进程旧报文隔离。

## 资源预算与关闭契约

| 项目 | 默认及硬上限 |
| --- | --- |
| TCP 流数 | 每会话默认 16，上限 32；全局 TCP 流预算最多 32 条 |
| TCP 输入队列 | 每流默认 8，上限 16 个完整包 |
| 返回队列 | TCP、UDP 各复用配置的输出容量，默认各 32，上限各 64 |
| TCP 收发环 | 每流各 32 KiB，接收位图 4 KiB，最多 64 项在途段记录 |
| TCP 流内存准入 | 每流 256 KiB，全局 8 MiB |
| 包所有权 | TCP 输入/输出与 UDP 输出共用 4 MiB 字节预算，每包最大 MTU 9000 |
| 超时 | TCP 空闲默认 30000 ms，范围 100–300000 ms；TIME-WAIT 默认 30000 ms，范围 1–120000 ms |

256 KiB 预留覆盖双环、位图、发送记录、最大 DNS 原响应及分帧副本同时存在、16 KiB 临时正文与控制结构；队列内包另持有共享包预算，不能重复把队列当成免费内存。DNS、出站 Socket/读写缓冲及规则快照继续持有各自既有预算；以上数值不是进程 RSS 或全部内核内存上限。返回队列背压有取消与空闲超时，不无限堆积包。

TCP 与 UDP 共用受控会话身份、Generation、撤销权限及任务登记。`close().await` 主动撤销、停止并等待真实任务和连接/解析器释放；保留旧 Arc 不能继续活动或阻止新会话使用已释放资源。Drop 仅提交关闭意图，不能当作关闭完成证据。关闭命令不受普通 FFI 请求配额阻塞，响应分配仍在取出包前完成，避免配额失败丢包。

## 可运行示例与验证入口

`examples/tun_tcp_kernel.rs` 通过标准输入/输出长度分帧传递 IP 包，复用生产会话；它自己不创建接口。真实系统 TCP 互通由 `tests/test_tun_tcp_kernel.py` 在独立 Linux 网络命名空间创建 TUN，仅连接本机测试服务，并拒绝在初始网络命名空间运行。

```sh
cargo build --locked --example tun_tcp_kernel
python3 Tools/test_reports.py --external test_tun_tcp_kernel -- sudo unshare --net python3 tests/test_tun_tcp_kernel.py "$PWD/target/debug/examples/tun_tcp_kernel"
```

状态机、编解码、缓冲和会话测试分别在 `tests/test_tun_tcp_state.rs`、`test_tun_tcp_wire.rs`、`test_tun_tcp_buffer.rs`、`test_tun_tcp_session.rs`；真实结果及未测边界以同名 `.md` 为准。报告生成器保留失败、重试、源码指纹及 dirty 状态，旧报告不能冒充当前源码验证。

Linux 夹具覆盖系统 TCP 客户端、IPv4/IPv6、1 MiB 双向、半关闭、握手丢包、数据丢包和返回包乱序；虚拟对端 Rust 测试不能替代它。macOS Network Extension、完整启动事务、系统路由保护、真实节点 TCP 路径及最低系统仍需各自验收；未执行项不得标成通过。

## 交付边界

本阶段版本按 SemVer 的兼容新能力推进到 dev 2.5.0。只提交、推送 dev；不改 main，不从 dev 发布 crates.io，不复用已发布标签。不修改 Fluxa 的固定 XCFramework URL/SHA-256，也不把旧二进制当作新 TCP 功能。正式发布及 App 实际接入须经过后续明确授权的 main 发布流程。

协议依据：[RFC 9293](https://www.rfc-editor.org/rfc/rfc9293)、[RFC 6298](https://www.rfc-editor.org/rfc/rfc6298)、[RFC 7766](https://www.rfc-editor.org/rfc/rfc7766)。仅依据协议编写自有实现，不复制外部 TCP 栈源码。

## UDP 返回分片

UDP 返回超过 MTU 时的有界源分片、队列语义与测试入口见 [出站 IP 分片](OutboundFragmentation.md)。TCP 仍按 MSS 分段。

## SACK

SACK 协商、接收端乱序区间与发送端选择恢复见 [SACK 说明](TCPSACK.md)。部分覆盖保守重传，不宣称完整 RFC 6675 恢复。

窗口扩大已与当前 SACK/分片路径整合，协商、资源及验收边界见 [TCPWindowScale.md](TCPWindowScale.md)。

ICMP 错误输入与 PMTU 调整见 [ICMPPMTU.md](ICMPPMTU.md)，不等同原始 Ping 代理。
