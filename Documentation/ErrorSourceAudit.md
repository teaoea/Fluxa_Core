# 错误体系源码审查索引

日期：2026-10-14。范围：当前 src 下全部 Rust 文件。此表记录错误创建、传播与边界核验的文件覆盖；自动提取的变体和 SHA-256 不是语义正确的证明。具体独立预期、复现与未验证事项见 [审查报告](ErrorSystemReview.md)。

| 文件 | 直接使用的内核错误变体 | 审查职责 |
| --- | --- | --- |
| `src/budget.rs` | ResourceLimit | 原子预算 checked_add 与 ByteLease Drop；错误拒绝不旁路配额。 |
| `src/dns/config.rs` | InvalidInput、ResourceLimit | 期限、DNSSEC、负响应、合成地址及缓存；统计锁失败保留 Internal，服务失败受控记录。 |
| `src/dns/fake_ip.rs` | Internal、InvalidInput、InvalidState、ResourceLimit | 期限、DNSSEC、负响应、合成地址及缓存；统计锁失败保留 Internal，服务失败受控记录。 |
| `src/dns/mod.rs` | DnsResponse、DnsTimeout、DnsValidation、FakeIpUnavailable、Internal、InvalidInput、InvalidState、ResourceLimit、Unsupported | 期限、DNSSEC、负响应、合成地址及缓存；统计锁失败保留 Internal，服务失败受控记录。 |
| `src/dns/observation.rs` | Internal、InvalidInput、ResourceLimit | 期限、DNSSEC、负响应、合成地址及缓存；统计锁失败保留 Internal，服务失败受控记录。 |
| `src/dns/service.rs` | DnsTimeout、Internal、InvalidInput、Network、ResourceLimit | 期限、DNSSEC、负响应、合成地址及缓存；统计锁失败保留 Internal，服务失败受控记录。 |
| `src/dns/wire.rs` | DnsResponse、Internal、InvalidInput、ResourceLimit、Unsupported | 期限、DNSSEC、负响应、合成地址及缓存；统计锁失败保留 Internal，服务失败受控记录。 |
| `src/error/diagnostics.rs` | Internal | 集中整数映射、受限对象解析与固定诊断；无敏感文本或业务所有权。 |
| `src/error.rs` | InvalidInput、ResourceLimit | 集中整数映射、受限对象解析与固定诊断；无敏感文本或业务所有权。 |
| `src/ffi.rs` | Certificate、DnsResponse、Internal、InvalidInput、InvalidState、ResourceLimit、Unsupported | 响应预留先于副作用；旧 ABI/JSON 保留，新机器字段与线程局部查询。 |
| `src/geoip.rs` | Internal、InvalidInput、Network、ResourceLimit | I/O 类别保留；后台 JoinError 是 Internal，不误报为连接故障。 |
| `src/http.rs` | InvalidInput、ResourceLimit、Unsupported | 报文分帧及长度限制；I/O 与输入失败分开，无错误默认放行。 |
| `src/https.rs` | Cancelled、Certificate、InvalidInput、NetworkTimeout、ResourceLimit、Tls、Unsupported | 配置 DNS/路由与 TLS；外层期限返回 NetworkTimeout，不改连接策略。 |
| `src/lib.rs` | 无直接错误构造；检查调用传播/模块边界 | 模块与 unsafe 边界入口；不另建平行错误模块。 |
| `src/lifecycle.rs` | 无直接错误构造；检查调用传播/模块边界 | 句柄不复用；Option 耗尽由调用边界转为资源错误，不是成功。 |
| `src/mitm/authority.rs` | Certificate | Authority 验证、代理任务、捕获；可选机器错误同一失败生成，不新增认证入口。 |
| `src/mitm/capture.rs` | Internal、InvalidState、ResourceLimit | Authority 验证、代理任务、捕获；可选机器错误同一失败生成，不新增认证入口。 |
| `src/mitm/mod.rs` | Internal、InvalidInput、InvalidState、NetworkTimeout、ResourceLimit、Tls、Unsupported | Authority 验证、代理任务、捕获；可选机器错误同一失败生成，不新增认证入口。 |
| `src/network.rs` | InvalidInput | 主机、地址及资源校验；类型转换拒绝非法输入。 |
| `src/outbound/configuration.rs` | Internal、InvalidInput、InvalidState、ResourceLimit | 快照固定、取消、协议失败及预算；不把 ERROR 转为 DIRECT。正式传输先行的既有 Debug 问题另列。 |
| `src/outbound/execution.rs` | DnsResponse、Internal、InvalidInput、InvalidState、Network、Rejected、Unsupported | 快照固定、取消、协议失败及预算；不把 ERROR 转为 DIRECT。正式传输先行的既有 Debug 问题另列。 |
| `src/outbound/resolution.rs` | DnsResponse、DnsValidation、Internal、InvalidInput、InvalidState、Network、ResourceLimit、Unsupported | 快照固定、取消、协议失败及预算；不把 ERROR 转为 DIRECT。正式传输先行的既有 Debug 问题另列。 |
| `src/outbound/resources.rs` | Internal、InvalidState | 快照固定、取消、协议失败及预算；不把 ERROR 转为 DIRECT。正式传输先行的既有 Debug 问题另列。 |
| `src/outbound/stream.rs` | Cancelled、Internal、InvalidInput、InvalidState、Unsupported | 快照固定、取消、协议失败及预算；不把 ERROR 转为 DIRECT。正式传输先行的既有 Debug 问题另列。 |
| `src/outbound.rs` | Cancelled、Internal、InvalidInput、InvalidState、Network、NetworkTimeout、Rejected、ResourceLimit、Unsupported | 快照固定、取消、协议失败及预算；不把 ERROR 转为 DIRECT。正式传输先行的既有 Debug 问题另列。 |
| `src/protocol/config.rs` | InvalidInput、Unsupported | 输入、TLS/密钥材料、加密计数、协议约束；XUDP 拒绝动作不变，改为 ProtocolViolation。 |
| `src/protocol/crypto.rs` | Internal、InvalidInput、ResourceLimit、Tls、Unsupported | 输入、TLS/密钥材料、加密计数、协议约束；XUDP 拒绝动作不变，改为 ProtocolViolation。 |
| `src/protocol/datagram.rs` | InvalidInput、InvalidState、Network、ProtocolViolation、ResourceLimit、Unsupported | 输入、TLS/密钥材料、加密计数、协议约束；XUDP 拒绝动作不变，改为 ProtocolViolation。 |
| `src/protocol/fingerprint.rs` | Internal、InvalidInput、ResourceLimit、Unsupported | 输入、TLS/密钥材料、加密计数、协议约束；XUDP 拒绝动作不变，改为 ProtocolViolation。 |
| `src/protocol/hybrid.rs` | Internal、Tls | 输入、TLS/密钥材料、加密计数、协议约束；XUDP 拒绝动作不变，改为 ProtocolViolation。 |
| `src/protocol/inner_tls.rs` | InvalidInput、ResourceLimit | 输入、TLS/密钥材料、加密计数、协议约束；XUDP 拒绝动作不变，改为 ProtocolViolation。 |
| `src/protocol/mod.rs` | 无直接错误构造；检查调用传播/模块边界 | 输入、TLS/密钥材料、加密计数、协议约束；XUDP 拒绝动作不变，改为 ProtocolViolation。 |
| `src/protocol/quic.rs` | 无直接错误构造；检查调用传播/模块边界 | 输入、TLS/密钥材料、加密计数、协议约束；XUDP 拒绝动作不变，改为 ProtocolViolation。 |
| `src/protocol/reality.rs` | Certificate、Internal、InvalidState、ResourceLimit、Tls、Unsupported | 输入、TLS/密钥材料、加密计数、协议约束；XUDP 拒绝动作不变，改为 ProtocolViolation。 |
| `src/protocol/stream.rs` | Internal、InvalidState、ResourceLimit、Tls、Unsupported | 输入、TLS/密钥材料、加密计数、协议约束；XUDP 拒绝动作不变，改为 ProtocolViolation。 |
| `src/protocol/vision.rs` | Internal、InvalidInput、ResourceLimit | 输入、TLS/密钥材料、加密计数、协议约束；XUDP 拒绝动作不变，改为 ProtocolViolation。 |
| `src/protocol/vless.rs` | InvalidInput | 输入、TLS/密钥材料、加密计数、协议约束；XUDP 拒绝动作不变，改为 ProtocolViolation。 |
| `src/rewrite.rs` | Internal、InvalidInput、Rejected、ResourceLimit、Unsupported | 请求拒绝和复写结果继续返回原失败；没有添加 Continue 回退。 |
| `src/routing/background.rs` | Cancelled、Internal、InvalidInput、InvalidState、Rejected、ResourceLimit | 输入严格解析、语义/结构信任分离、快照/文件提交；不改类别排序、覆盖及 Debug 入口。 |
| `src/routing/cache.rs` | InvalidInput、ResourceLimit、Unsupported | 输入严格解析、语义/结构信任分离、快照/文件提交；不改类别排序、覆盖及 Debug 入口。 |
| `src/routing/debug.rs` | Internal、InvalidInput、InvalidState、ResourceLimit、Unsupported | 输入严格解析、语义/结构信任分离、快照/文件提交；不改类别排序、覆盖及 Debug 入口。 |
| `src/routing/flat.rs` | InvalidInput、ResourceLimit | 输入严格解析、语义/结构信任分离、快照/文件提交；不改类别排序、覆盖及 Debug 入口。 |
| `src/routing/index.rs` | ResourceLimit | 输入严格解析、语义/结构信任分离、快照/文件提交；不改类别排序、覆盖及 Debug 入口。 |
| `src/routing/runtime.rs` | Internal、InvalidInput、InvalidState、ResourceLimit | 输入严格解析、语义/结构信任分离、快照/文件提交；不改类别排序、覆盖及 Debug 入口。 |
| `src/routing/store.rs` | Cancelled、Internal、InvalidInput、InvalidState、Network、ResourceLimit、Unsupported | 输入严格解析、语义/结构信任分离、快照/文件提交；不改类别排序、覆盖及 Debug 入口。 |
| `src/routing/yaml.rs` | Internal、InvalidInput、ResourceLimit | 输入严格解析、语义/结构信任分离、快照/文件提交；不改类别排序、覆盖及 Debug 入口。 |
| `src/routing.rs` | InvalidInput、Rejected、Unsupported | 输入严格解析、语义/结构信任分离、快照/文件提交；不改类别排序、覆盖及 Debug 入口。 |
| `src/script.rs` | Allocation、InvalidInput、ResourceLimit、Script、ScriptTimeout | 运行时/堆预算、截止时间、结果验证；超时与脚本失败保持失败。 |
| `src/subscription.rs` | Internal、InvalidInput、Network、ResourceLimit、Unsupported | 严格规则解析、下载与计划；后台 JoinError 是 Internal，不放宽数量限制。 |
| `src/traffic_script.rs` | InvalidInput、ResourceLimit、Script、Unsupported | 运行时/堆预算、截止时间、结果验证；超时与脚本失败保持失败。 |
| `src/tun/udp.rs` | Cancelled、Internal、InvalidInput、InvalidState、NetworkTimeout、ResourceLimit、Unsupported | 包检查、会话取消及返回包；队列空/正常关闭不登记为执行错误，错误发布保留真实原因。 |
| `src/tun.rs` | InvalidInput、InvalidState、ResourceLimit、Unsupported | 包检查、会话取消及返回包；队列空/正常关闭不登记为执行错误，错误发布保留真实原因。 |
