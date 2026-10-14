# Fluxa Network Core

Author: 可莉唯一的狗.

[中文](../README.md)

`libfluxa_core` is a cross-platform network core written in Rust. Network operations share connection management, bounded resources, and explicit failure handling.

## Capabilities

- Native VLESS + TCP + REALITY + Vision outbound, fixed Chrome/Firefox/Safari/iOS/Android Chrome/Edge ClientHello templates, and X25519MLKEM768 for modern Chrome. See [Protocols.md](Protocols.md) for verified versions and unsupported handshake behavior.

- HTTP request forwarding, CONNECT tunnels, and HTTP/1.1 framing.
- HTTPS with TLS 1.2/1.3 and upstream certificate verification.
- MITM for explicitly selected domains, certificate generation, and bounded request/response capture.
- Header and response rewriting, bounded JavaScript execution, and isolated JSON transformations.
- UDP/TCP DNS, DoT/DoH, configurable upstreams, positive/negative caching, DNSSEC, Fake-IP mappings, and access-controlled LAN DNS service.
- Runtime rule generation, normalization, deduplication, proven full-coverage pruning, and indexed matching. Domain targets use domain rules first; a domain match skips IP rules. Missing or unsupported proxy outbounds fail explicitly.
- Shared TCP/UDP outbounds, cancellation, half-close, idle expiry, and atomic configuration snapshots.
- Daily remote subscription schedules in UTC and bounded HTTPS subscription downloads. A missing schedule means manual updates only. On resume, only the most recent missed occurrence is handled; missed days are not replayed as separate jobs.
- TUN packet inspection and checksum validation. Bounded UDP forwarding is supported; TCP and system tunnels remain unsupported.

## Configuration examples

The complete host configuration examples are in the repository root:

- [fluxa.yaml](../fluxa.yaml): Chinese display names and Chinese comments.
- [fluxa_english.yaml](../fluxa_english.yaml): English display names and Chinese comments.

Use `update-time: "12:00"` for a daily update at 12:00 UTC. Omit the field to disable automatic updates. Individual updates and Update All are manual actions. The removed `update-interval-hours` field is rejected. Failed attempts retain the previous content and retry with a minimum 60-second backoff; a successful catch-up is not repeated. Schedules require a running host and network access; they do not promise operating-system wakeup while the host is closed or suspended.

Configuration fields and both complete examples are updated together. See [Subscriptions.md](Subscriptions.md), [RuntimeRules.md](RuntimeRules.md), [DNS.md](DNS.md), and [Outbound.md](Outbound.md) for semantics and current limitations.

## Roadmap and license

TUN forwarding, gateway operation, and additional proxy protocols remain in development. VLESS + TCP + REALITY + Vision is implemented for TCP outbounds; Single-target XUDP datagrams are supported over the same transport. General Mux, TCP TUN forwarding and system tunnels remain unsupported.

Original code is licensed under MIT; see [LICESE.md](../LICESE.md). Third-party dependencies retain their upstream licenses.

Rule subscriptions use Clash classical `payload` with uppercase DOMAIN, DOMAIN-SUFFIX, DOMAIN-KEYWORD and IP-CIDR (IPv4/IPv6). Subscription policy/outbound selects the action. 合并规则使用 runtime-rules.bin 二进制平铺缓存，加载不重建匹配索引；见 BinaryRuleCache.md。 See [rules.yaml](../rules.yaml).

Offline GeoIP uses bounded MMDB loading and IPv4/IPv6 lookup, with fixed free public-IP echo endpoints and TCP connection timing. Databases are supplied separately by the host. Failures are explicit; these features do not imply system TUN support. See GeoIP.md for resource limits, privacy and licensing.


默认 DNS 最近解析统计：Rust Resolver::last_query 与 C ABI dns_last_query 提供同配置存活解析器共享的只读结果；Swift 可读取 durationMilliseconds、fromCache、successful。不主动探测；缓存命中显示 0 ms，Fake-IP 合成不更新；记录默认首服务器的实际尝试（包括失败），回退服务器不冒充默认服务器。不同配置隔离，最后一个解析器释放后统计消失，最多 16 项，每个键最多 8192 字节。不记录域名和响应；进程外解析不计入。


TUN UDP v1.9.0 adds bounded IPv4/IPv6 UDP packet forwarding through the shared routing/outbound manager, return packet generation, cancellation and binary C ABI. No main configuration fields are added. TCP forwarding, system tunnels and DNS interception remain unsupported. See TUN.md.


## 本地规则审查修订（未发布）

规则类别编号仅作为索引槽位，优先级独立表达。普通端口/传输规则不能覆盖域名/IP 普通规则，全域传输直连/代理被拒绝。API 专用临时调试 Overlay 可覆盖正式拦截；每条必须提供 1–3600 秒生命周期，内部存过期时间，到期失效并丢弃。配置和订阅不允许声明调试规则。原始来源可用时重新验证诊断；没有原始来源的生成文档仅标记结构验证。可运行示例为 cargo run --example runtime_rules_audit。C ABI/Swift 调试接入、控制接口认证及 App 集成尚未交付，本地修改暂缓提交与发布。

Protocol blocking accepts transport tcp/udp/quic with reject only. QUIC v1/v2 identification uses packet structure and connection-local association, not UDP/443. Unknown standalone short headers, migration and unrecognized versions remain identification boundaries. Shared outbounds and TUN UDP enforce the same immutable connection policy. See ProtocolBlocking.md.

TUN DNS/Fake-IP v2.2.0 intercepts explicitly configured virtual DNS endpoints using the existing resolver, encrypted upstreams and DNSSEC. Fake targets restore their domain using the bound resolver generation before real DNS and routing. Resolver replacement requires reopening the session; existing UDP flows retain their connection. TCP DNS ingress and system tunnels remain unsupported. See TunDNS.md.

v2.2.1 includes ordinary API allocation history in the bounded pool high-water mark. Once a pool is bound to TUN, ordinary allocations share its cursor and foreign resolver mappings remain rejected by the owning session.

## v2.2.2 FFI response admission

FFI reserves a response slot, byte quota, and serialization buffer before executing a command or consuming a TUN return packet. On response-admission exhaustion the command returns zero before its side effects; release previous response handles before retrying. Other boundary failures must not be assumed safe to retry. Run `cargo run --example ffi_response_retry` for a local TCP demonstration. Large responses reserve conservatively within the existing 32 MiB global quota; no unbounded wait queue is introduced. This example is local ABI evidence, not system VPN validation.

## 统一整数错误协议

错误类型与公开代码集中在 `src/error.rs`；代码是显式固定的 `u16`，每项有中文注释，不按枚举顺序推导，不用字符串识别。`Error::public_info()` 返回无堆分配的有界上下文对象；JSON 中 code 为四位整数，前两位类别、后两位具体错误。必需上下文按登记 Schema 提供，不能全部置空。完整语义、源码、Schema、兼容性及实际后果见 [错误代码查询表](错误代码查询表.md)。

按用户要求全部已有接口统一新方案，ABI 版本为 2，旧错误编号不再兼容；command 与 `fluxa_engine_command_v2` 均返回结构化字段，`fluxa_engine_last_error_code` 在同一线程直接读取整数，零是无错误状态，不分配、不加锁。错误 JSON 查询也不分配响应句柄。能力位 25 声明该扩展。

运行 `cargo run --example error_protocol` 查看整数分类、往返和 Job/Capture 字段迁移；运行 `cargo run --example error_contract -- --check` 及 `cargo test --test error_contract` 核验契约与独立语义；`bash check-ffi.sh error_code` 执行真实 Swift FFI。测试不等于全平台或系统 VPN 验收，审查边界见 [ErrorReworkReview.md](ErrorReworkReview.md)。开发强制遵循 [错误体系规范](错误代码体系设计.md)。
