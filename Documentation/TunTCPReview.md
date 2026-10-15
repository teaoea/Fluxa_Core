# TCP/TUN 数据路径独立语义审查

作者：可莉唯一的狗。

## 范围与依据

本次工作基于 dev `85f8a4ea1141c68ce5a931bde09859ae722d88b2`，工作区有本阶段修改。版本为开发态 2.5.0；不合并 main、不创建正式发布标签、不发布 crates.io，不改变 App 固定远程二进制依赖。审查依据为《API全生命周期管理》《会话生命周期与资源所有权隔离原则》《错误代码体系设计》《测试文件与报告统一管理原则》及《VPN启动技术规范》。本文件不能替代同名测试报告的执行结果。

范围为自研 TCP 状态机、TCP/UDP 数据包会话、DNS/Fake-IP/路由/共用出站衔接、C ABI 及相邻 App 的薄 Swift 门面。不创建 macOS VPN，不申请系统权限，不改变宿主代理或路由。没有引入第三方 TCP 栈。

## L1：类型、边界与资源

- `src/tun/tcp/wire.rs` 在进入状态机前检查 IPv4/IPv6 长度、协议、校验和、TCP 头、选项及端点。业务模块未新增 unsafe、跨语言业务指针或 unchecked。
- `buffer.rs` 使用固定收发环及接收位图，处理回绕、重叠和乱序；`state.rs` 限制在途记录，发送 FIN 也必须取得记录空间。
- `session.rs` 每流预留 256 KiB，覆盖环、位图、发送记录、DNS 原响应和分帧副本等；全局 8 MiB，最多 32 条。TCP 输入/返回及 UDP 返回的完整包另持有共用 4 MiB 字节预算。出站及解析器继续遵守原独立预算，不把上述数字冒充进程 RSS 上限。
- 状态机、工作任务和编码器保持内部可见；没有仅为单元测试扩大生产 API。公开数据包入口不代表系统接管。
- 静态检查以实际执行的 fmt、Clippy 及测试报告为证据；最低部署系统及全部平台运行未验证。

## L2：断言与真实执行

| 契约 | 对应测试 |
| --- | --- |
| 缓冲区、序号回绕、重叠数据不重复交付 | `tests/test_tun_tcp_buffer.rs` |
| 握手、确认、分段、RTT/Karn、重传预算、零窗口、半关闭、FIN 记录准入 | `tests/test_tun_tcp_state.rs` |
| 严格编码、损坏校验和、畸形选项、紧急数据和截断输入 | `tests/test_tun_tcp_wire.rs` |
| 真实回环 IPv4/IPv6、TCP DNS 分帧、Fake-IP 返回地址、规则拒绝 | `tests/test_tun_tcp_session.rs` |
| 并发关闭、满队列、配额耗尽、实际 Socket EOF、预算回收、旧引用及重建 | `tests/test_tun_tcp_session.rs` |
| 规则更新不改变存量连接，新流按新快照拒绝且不创建 Socket | `tests/test_tun_tcp_session.rs` |
| 独立系统 TCP：IPv4/IPv6、1 MiB 双向、握手/数据丢包与返回乱序、半关闭 | `tests/test_tun_tcp_kernel.py` |
| Swift 独立构造包、二十次生命周期、半关闭和 REJECT | Fluxa 适配包 `tests/test_ffi_tun_tcp.swift` |

每个 Rust 测试显式登记于 `Tools/test-suites.json`，同名 Markdown 由执行器生成，原始退出码和历史失败保留。最新结果以对应报告的源码指纹及运行记录为准；普通全量 Rust 运行中独立协议节点用例仍可能为 IGNORED，不能据此宣称实际协议互通。独立代理互通通过 App 的 `check-protocol.sh tun_tcp` 另行执行，必须核实全部实际结果。

## L3：以业务约束独立核实

| 输入或状态 | 应有后果及实现检查 |
| --- | --- |
| 不可信 IP/TCP 包 | 校验失败返回既有 InvalidInput/Unsupported；不生成无依据的成功连接，不拨号 |
| 规则 REJECT | 连接前由共用出站执行决策；数据面返回 RST 和独立规则拒绝错误，不降级 DIRECT |
| DNS 失败或 Fake-IP 无有效映射/代次 | 使用所属解析器和现有共用出站校验，失败终止；不能连接合成地址。共用映射失效场景另见 `test_tun_dns` / `test_outbound_fake_ip`；不能将这些用例冒充每一种 TCP 故障均独立测试 |
| 新规则提交 | 每条新流捕获一致快照；连接建立后释放完整快照，存量流保留原真实连接，不重拨绕过规则 |
| 会话关闭/取消 | 通过共享 Authority 撤销身份并登记任务；等待真实任务/连接/解析器释放后返回，Drop 只提交关闭意图 |
| 旧 Arc 长期存活 | 不能继续活动或妨碍已释放资源重新绑定；实际保留内存仍持有其租约，不能以引用计数决定活动占用 |
| 输出拥塞/资源不足 | 有界队列、字节及流预算拒绝超限；发送等待有取消和空闲期限，不能无界堆积 |
| FFI 响应预算不足 | 复用先预留响应再取包的流程，失败可重试，不消费未交付的包；关闭使用既有控制路径 |
| 系统 TUN 启动请求 | 仍拒绝未实现的系统入口；能力位 28、SYN-ACK 或端口存活都不构成 Running 证明 |

新增命令复用既有结构化错误类型及代码；未新增、改号或重用公开错误代码。配置格式未变化，不新增主 YAML 字段。新增选项为严格命令数据，未知/越界字段拒绝。本次未改变规则优先级、Debug 授权或订阅语义。

审查确认并修复一处资源契约问题：当数据段已占满 64 条在途记录时，FIN 原先仍可追加，且若只限制发送而不调整 deadline 会产生即时空转。现对 FIN 及其 deadline 同时施加容量准入，回归 `finish_waits_for_bounded_flight_ledger_space` 要求满额时不发送、不即时唤醒，确认一段后才允许 FIN。

另外确认并修复半开流拒绝服务缺陷：重复 SYN 原先清除 `sent_at`，立即重发未增加重试计数，任意入包又刷新空闲时间。现保留发送记录并消耗五次重试预算，增加包含真实出站准备在内、不可延长的 30 秒握手期限；未建立阶段的入包不再刷新空闲时间。`duplicate_syn_consumes_retry_budget_and_handshake_has_absolute_deadline` 以确定性时间验证预算与绝对期限，`invalid_handshake_packets_do_not_refresh_idle_deadline` 以真实 Socket 验证持续无效 ACK 仍超时并释放连接；`absolute_handshake_deadline_cancels_pending_real_dns_before_dial` 验证真实 DNS 等待也被同一绝对期限取消，不依赖已配置的较长 DNS 超时。

## L4：证据、异常与边界

执行入口：

```sh
cargo fmt --check
cargo clippy --locked --offline --all-targets -- -D warnings
python3 Tools/test_reports.py --external test_test_reporting -- python3 tests/test_test_reporting.py
python3 Tools/test_reports.py --run
python3 Tools/test_reports.py --check-reports
FLUXA_CORE_ROOT=/实际受测内核目录 bash Packages/FluxaKernelBridge/check-ffi.sh all
FLUXA_CORE_ROOT=/实际受测内核目录 bash Packages/FluxaKernelBridge/check-protocol.sh tun_tcp
```

最后两项在相邻 Fluxa 仓库执行，仅为显式源码联调；生产构建不使用该路径。Linux 通过 OrbStack Debian arm64 的独立 `unshare --net` 网络命名空间运行 `/dev/net/tun` 夹具；不修改 macOS 系统网络。独立 REALITY/Vision 对端为固定版本 Xray v26.3.27 可执行测试夹具，不链接、不分发、不当作内核实现；其下载归档 SHA-256 为 `2e93a67e8aa1936ecefb307e120830fcbd4c643ab9b1c46a2d0838d5f8409eaf`。

已发生的异常保留：加入握手硬期限后，原先在 SYN 阶段验证 63 秒重传周期的测试实际失败；把指数退避用例放到已建立的数据阶段，仍验证五次重试及最终失败，新增用例独立验证握手 30 秒硬截止，没有放宽重试预算或删除断言。最初 Clippy 发现复用测试夹具的重复模块声明，已改为引用原公共夹具；Linux 在 1 GiB cgroup 内把工具链和编译缓存放在 tmpfs 时两次 OOM，被系统 SIGKILL。将本任务工具链/缓存移到虚拟机磁盘、保持单编译任务后构建成功，没有提高内核预算或放宽断言。一次代理测试期间补充测试/头文件导致源码指纹变化，执行器返回 STALE；旧通过不能算作新源码通过，冻结后重新运行。

开发原始证据保存于本机 `/tmp/fluxa-tcp-*`，报告登记每次实际路径、源码指纹、基线提交及 dirty 状态；不是已上传的 CI 工件。CI 工作流新增独立 Linux TCP TUN 任务，但本地通过不代表远端 CI 已运行。最终准确数量及结果见同名报告，不将夹具、NOT RUN、IGNORED 或历史结果计为通过。

尚未验证/未完成：macOS Network Extension、Preflight/Prepare/Commit/Verify、无节点可信确认、权限与路由回环保护、系统 DNS/路由回滚、接管后验证、休眠及网络切换、最低 macOS/Intel/Windows/iOS 运行、真实公网节点及高负载长期压力。窗口扩大、SACK、时间戳、ECN、IP 分片/扩展头、ICMP/PMTU、紧急数据和数据型 SYN 未实现；TIME-WAIT 不保证跨进程旧包隔离。不能据本阶段认定完整 TCP RFC 合规或系统 VPN 可用。
