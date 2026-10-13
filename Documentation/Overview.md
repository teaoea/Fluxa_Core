# Fluxa 网络内核

作者：可莉唯一的狗。

[English](../README_ENGLISH.md)

`libfluxa_core` 是使用 Rust 开发的跨平台网络内核，提供统一的网络处理与资源管理能力。

## 当前能力

- 自研 VLESS + TCP + REALITY + Vision 出站：固定 Chrome、Firefox、Safari、iOS、Android Chrome、Edge 指纹；现代 Chrome 支持 X25519MLKEM768，详见 [Protocols.md](Protocols.md)。

- HTTP 代理：请求转发、CONNECT 隧道与分块解码。
- HTTPS：TLS 1.2/1.3、上游证书验证及请求/响应处理。
- MITM：精确域名解密、证书签发及请求/响应捕获。
- 流量改写：请求头、响应头和文本响应改写。
- JavaScript：受限脚本执行、JSON 转换、内置 JSON 响应字段处理及执行隔离。

- DNS：UDP/TCP、DoT/DoH、手动上游、正负缓存、DNSSEC、Fake-IP 双向映射及有访问控制的局域网 UDP/TCP 服务；详见 [DNS.md](DNS.md)。
- 路由：来源感知的运行时规则生成、规范化、去重、完全覆盖裁剪与索引匹配，支持域名、IPv4/IPv6 CIDR、关键词、端口与 TCP/UDP；域名命中后跳过 IP 规则，同类型本地优先，详见 [RuntimeRules.md](RuntimeRules.md)。缺失或不支持的代理出站明确拒绝。
- TUN 基础：IPv4/IPv6、TCP/UDP 数据包检查、校验和验证与有界会话；新增有界 UDP/IP 双向转发；TCP 与系统隧道尚未完成。

通过所有权和 RAII 管理资源，为连接、缓冲区、日志和脚本运行时设置上限。

## 配置示例

完整主配置位于根目录 [fluxa.yaml](../fluxa.yaml) 与 [fluxa_english.yaml](../fluxa_english.yaml)，字段及行为一致，注释为中文。网络配置字段新增、修改或删除时同步更新并校验两份示例。

远程订阅支持 `update-time: "12:00"`（每天 UTC 12:00），未配置时仅手动更新；单项及更新全部都属于手动操作。恢复运行仅补最近到期一次，不逐天补跑；旧更新间隔字段已移除。计划和有界 HTTPS 下载由 Rust 共用实现，详见 [Subscriptions.md](Subscriptions.md)。

## 后续计划

完善 TUN 转发、网关及其他代理协议。

## 许可

自有代码采用 MIT，详见 [许可声明](../LICESE.md)。

共用 TCP/UDP 出站支持数字地址、配置 DNS 的域名及带代次的 Fake-IP 连接，提供双向数据、半关闭、超时、取消和实际路由执行；HTTP/HTTPS/MITM 共用同一连接管线。详见 [Outbound.md](Outbound.md) 和 [Routing.md](Routing.md)。

共用连接提供有界会话状态、流量统计、自动空闲回收和原子网络配置；旧连接及在途请求保留原快照，新请求使用新配置。详情见 Outbound.md。

规则订阅采用 Clash classical `payload`，类型严格使用 DOMAIN、DOMAIN-SUFFIX、DOMAIN-KEYWORD、IP-CIDR（统一支持 IPv4/IPv6）；动作由订阅 policy/outbound 指定。合并后的规则保存至 runtime-rules.bin，匹配直接使用二进制平铺索引；格式与范围见 [BinaryRuleCache.md](BinaryRuleCache.md)。订阅示例见 [rules.yaml](../rules.yaml)。

离线 GeoIP：有界读取 MMDB 并查询 IPv4/IPv6，提供免费公网 IP 回显与 TCP 建连测量。数据库与网络结果均不可信，失败返回明确错误。详见 [GeoIP.md](GeoIP.md)。


默认 DNS 最近解析统计：Rust Resolver::last_query 与 C ABI dns_last_query 提供同配置存活解析器共享的只读结果；Swift 可读取 durationMilliseconds、fromCache、successful。不主动探测；缓存命中显示 0 ms，Fake-IP 合成不更新；记录默认首服务器的实际尝试（包括失败），回退服务器不冒充默认服务器。不同配置隔离，最后一个解析器释放后统计消失，最多 16 项，每个键最多 8192 字节。不记录域名和响应；进程外解析不计入。


TUN UDP v1.9.0：Rust 负责有界流表、共用路由/出站、返回包生成和取消回收；二进制 C ABI 不使用 Base64。没有新增主配置字段；TCP、系统隧道与 DNS 截获仍不支持。详见 TUN.md。

XUDP v2.0.0：单目标 UDP 代理复用 REALITY/Vision、路由与取消机制，TUN UDP 使用同一出站。报文上限 7526 字节，空包明确拒绝；任意 Mux 与系统 TUN 未实现。详见 Protocols.md。

协议拦截支持 transport 的 tcp、udp、quic，仅允许 reject。QUIC 按 v1/v2 报文及连接内关联识别，不按 UDP/443 推断；详见 ProtocolBlocking.md。

TUN DNS/Fake-IP v2.2.0：显式虚拟 DNS 端点复用配置上游与标准 DNS 响应；合成目标由所属解析器自动校验代次并恢复真实地址。解析器更换后旧会话拒绝新映射请求，需要重开会话；存量 UDP 流保留旧连接。TCP TUN、DNS over TCP 入口和系统隧道尚未实现。详见 TunDNS.md。

v2.2.1 修复普通 API 与 TUN 地址池的交叉复用，历史地址退役、后续分配使用同一安全高水位，外来映射不能用于本会话。

### v2.2.2：FFI 响应配额修复

响应句柄与预算在命令执行、TCP 读取和 TUN 取包之前预留，配额不足时不消费数据。释放旧响应后可重试。可运行例程为 `cargo run --example ffi_response_retry`；详细约束及未测边界见 Outbound.md。

## 统一整数错误协议

错误类型与公开代码集中在 `src/error.rs`；代码是显式固定的 `u16`，每项有中文注释，不按枚举顺序推导，不用字符串识别。`Error::public_info()` 返回 2 字节固定对象；JSON 中 code 是整数，params 当前为空对象。完整语义、源码、Schema、兼容性及实际后果见 [错误代码查询表](错误代码查询表.md)。

C ABI v1 数值与旧 JSON 保持兼容；`fluxa_engine_command_v2` 显式返回结构化字段，`fluxa_engine_last_error_code` 在同一线程直接读取整数，零是无错误状态，不分配、不加锁。错误 JSON 查询也不分配响应句柄。能力位 25 声明该扩展。

运行 `cargo run --example error_protocol` 查看整数分类、往返和 Job/Capture 字段迁移；运行 `cargo run --example error_contract -- --check` 及 `cargo test --test error_contract` 核验契约与独立语义；`bash check-ffi.sh error_code` 执行真实 Swift FFI。测试不等于全平台或系统 VPN 验收，审查边界见 [ErrorSystemReview.md](ErrorSystemReview.md)。开发强制遵循 [错误体系规范](Rust网络代理内核统一错误代码体系设计、实现与审查.md)。
