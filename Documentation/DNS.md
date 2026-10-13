# DNS 解析

作者：可莉唯一的狗。

v0.4.0 提供 UDP/TCP、DoT 和 HTTP/2 DoH 查询。UDP 截断回复改用同一上游 TCP；多个上游按配置顺序、共享总期限切换。加密与明文上游不能混合，错误证书、错误域名和 DNSSEC 验证失败不降级。bootstrap 必须使用数字 IP 和端口，不使用系统 DNS/hosts。

支持 A、AAAA、CNAME 和标准记录查询；拒绝 ANY、AXFR、IXFR、OPT 和未知类型。正缓存最多一小时；NXDOMAIN/NODATA 使用 SOA 的 TTL/minimum，最多五分钟。每个实例最多 128 条缓存，全局预算 16 MiB，每条预留 128 KiB；全局最多 16 个客户端和 16 个在途查询，超过上限拒绝。查询总期限 100–30000 毫秒，上游 1–8 个。关闭后查询持有的资源在查询结束时释放。

DNSSEC 使用 Hickory 签名和信任链验证及内置根锚；Rust 私有部署可显式提供根锚。安全状态 secure、insecure、disabled；AD 标记不能代替验证。不可信根、篡改和过期签名拒绝返回答案。已验证的否定结果只按验证后的 SOA/NSEC 有效 TTL 缓存。

## YAML 配置

App 主配置采用 dns 字段；timeout-ms 默认 5000、dnssec 默认 false。

```yaml
dns:
  timeout-ms: 5000
  dnssec: true
  servers:
    - transport: doh
      url: https://cloudflare-dns.com/dns-query
      bootstrap: 1.1.1.1:443
```

udp/tcp 使用 address（IP:端口）；dot 使用 address、server-name；doh 使用 url、bootstrap。IPv6 地址带方括号。URL 禁止凭据和 fragment，bootstrap 端口必须与 URL 一致。配置保存不触发查询；用户执行查询时，域名会发给所选服务商。

## 验证与边界

真套接字测试覆盖四种传输、CNAME、AAAA、TXT、截断、正负缓存和到期、上游切换、证书信任和域名、事务编号、超时恢复。签名根区域测试覆盖合法、篡改、过期、不可信根与 NSEC 否定证明。真实 Swift FFI 覆盖四种传输、缓存、关闭、100 次释放及不信任 AD 标记。不修改系统信任或 DNS 设置。

Fake-IP 与局域网 DNS 服务已分别实现，详见下文。未实现 DoQ/HTTP3、权威 DNS、mDNS 或根锚自动更新。HTTP 调试服务现有系统域名解析将在共用出站阶段接入本模块。查询能力不代表 TUN、网关或系统代理已可用。仅发布 macOS ARM64 切片；最低部署系统及其他平台尚未运行验证。

## v0.4.0 发布记录（2026-10-10）

- 固定源码与 crates.io 包：df3ce4606337f5a18350450832e50af51f06b4fc。实际下载 crate 的 .cargo_vcs_info.json 与 45 个文件已核对，不含 Swift、原生二进制或私钥。
- 发布标签：c893764fa9e862a36041f4518c5a6e05496482dd；与源码仅差 Package.swift 二进制校验值。
- GitHub Actions 38026554786 成功：53 项 Rust 测试、Clippy、六组真实 FFI、静态框架构建与远程 Swift Package 测试。
- 框架 SHA-256：39b3676d8ee000ed4750ee2cf96614cbf3f2bf63550362282f9a9f5f0d721a8c；测试服务 SHA-256：7f701dded3016816ab83c860a6486f002b2c5c0d78f95b9f01cc187ec65d0baa。
- 本机真实 Swift FFI → Cloudflare HTTP/2 DoH：cloudflare.com 验证为 secure，dnssec-failed.org 被拒绝。此为特定时点的公开服务互通，不代表长期服务可用性。
- 本机 macOS 27.2 ARM64 及 CI macOS ARM64 验证；macOS 14 运行、其他架构和 Apple 平台切片尚未验收。Fake-IP、局域网服务仍待单独交付。

## Fake-IP（v0.5.0）

DNS 配置可增加 fake-ip。A/AAAA 返回合成地址，其他记录类型与排除域名使用所选真实上游。合成结果的验证状态明确为 synthetic，不代表原始域名签名验证成功，也不会被标记为 DNSSEC secure。

```yaml
fake-ip:
  ipv4-range: 198.18.0.0/16
  ipv6-range: fd00:198:18::/64
  capacity: 4096
  exclude:
    - localhost
    - local
```

IPv4 范围限 198.18.0.0/15 的对齐子网，前缀 15–30；IPv6 限 fd00::/8 的对齐子网，前缀 64–120。容量 1–4096 且不得大于可用地址。域名大小写、末尾点统一归一化；排除项按域名标签边界匹配。映射保留到显式重置/关闭，不按响应的 60 秒 TTL 自动淘汰。正反向查询、A/AAAA 共享域名编号；反查不存在返回空值，输入错误或未启用则明确拒绝。

同一实例重置后只清除映射，不倒退地址游标，旧地址不会分配给其他域名。地址池耗尽后拒绝分配；全局映射预算 16 MiB，每条预留 1 KiB，关闭与重置归还预算。关闭或替换配置创建新实例可能重新使用地址，因此调用者必须先停止使用旧池的流量；当前没有把映射直接接到系统 TUN 或网关。

Rust 测试覆盖稳定双向映射、IPv4/IPv6、排除边界、容量/范围/地址池耗尽、重置不复用、并发分配、排除项真实解析、合成查询不联系上游及不伪造 DNSSEC。真实 Swift FFI 覆盖配置、A/AAAA、反查、容量拒绝、重置与关闭。TUN/路由/出站接入仍为后续任务。

## v0.5.0 发布记录（2026-10-10）

源码及 crates.io 包来源 4112bfb0ab0322447b6533f2e1d59c17fc121521，实际下载包的来源和 46 个文件已核对。标签 d931caeb4a7749ca02baf5b6b0be98dfa2c98f35 只更新二进制下载校验值。GitHub Actions 38027774048 成功：57 项 Rust 测试、Clippy、六组真实 Swift FFI、框架构建及远程 Swift Package 测试。

框架 SHA-256：8086fbabd3c247580fe16f354f0ec35048deacaf1dd7610792ff7fe10c888f65；测试服务 SHA-256：9b66df3ecd32c5213aab46bbe1b75fb6b1e8779e9e0bd8559f1f26bde74c2569。框架附带本仓库许可清单和 Hickory MIT 通知。最低 macOS 14 运行、其他平台和 TUN/网关转发仍未验收；局域网 DNS 服务是下一项。

## 局域网 DNS 服务（v0.6.0）

服务复用客户端的显式上游、正负缓存、DNSSEC 和 Fake-IP，不另建解析规则。仅支持递归转发，不提供权威区域、区域传送、mDNS 或迭代解析。配置校验不监听端口，调用 serve/显式启动才监听；同一端口同时支持 UDP 和带长度帧的 TCP，端口 0 用于系统分配。

```yaml
service:
  listen: 0.0.0.0:5353
  allow-lan: true
  allowed-clients:
    - 192.168.1.0/24
```

默认 127.0.0.1:0，仅本机可访问。局域网监听必须显式启用并提供 1–32 个对齐 CIDR；仅接受私有、回环或链路本地范围，拒绝公网及全网访问，IPv4 映射 IPv6 客户端按 IPv4 校验。不自动修改系统 DNS、路由器、防火墙或系统信任。固定监听 53 端口所需权限由宿主处理。

全局最多 4 个服务、16 个 TCP 连接及 16 个在途服务请求；单服务最多 128 个近期来源，每来源每秒最多 32 个问题。输入 12–4096 字节，单 TCP 会话总期限 60 秒、等待问题 15 秒、正文读写 5 秒。计数区分已接受与拒绝问题，不表示上游成功数。UDP 输出默认 512 字节，EDNS 最高 1232 字节，大响应返回 TC，客户端可重试 TCP；TCP 输出最高 65535 字节、最多 256 条记录。

保留请求编号和问题，过滤不支持的附加记录/TSIG，拒绝非 IN、非递归、ANY 和区域传送；错误返回 FORMERR/REFUSED/NOTIMP/SERVFAIL，EDNS 版本错误返回 BADVERS。AD 只对本地验证 secure 且客户端请求 AD/DO 时设置；合成 Fake-IP 不设置 AD。CD 不绕过服务端强制 DNSSEC 策略；非 DO 请求省略未显式查询的 DNSSEC 证明记录。这里没有实现 TSIG 身份认证，CIDR 访问范围不是密码学认证。

C ABI 提供 dns_service_start/stop/status；运行中重配置明确拒绝。停止等待监听与连接子任务退出，关闭 DNS 一并停止服务；Drop 发取消作为异步兜底。Rust 不持有跨等待的 FFI 注册表或配置锁，业务没有新增 unsafe。端口冲突、服务配额及取消路径均归还资源。

验证覆盖真实四种上游到下游 UDP/TCP、CNAME/NXDOMAIN、EDNS/大响应截断和 TCP 重试、DNSSEC 好/坏签名、Fake-IP 无 AD、CIDR 拒绝、速率限制、IPv6、停止持久/空闲连接、绑定失败清理、Drop 与全局配额。独立系统 dig 客户端验证 UDP/TCP Fake-IP 互通。真实 Swift FFI 另测启停、状态、关闭、配置拒绝和重复生命周期。其他物理局域网设备、最低 macOS 14 运行和其他平台未验收；此服务能力仍不代表网关或 TUN 转发已实现。

本机发布前检查：62 项 Rust 测试、Clippy -D warnings、SwiftFormat/SwiftLint 严格检查通过；七组真实 Swift FFI 通过，服务组含 20 次启停/关闭及系统 dig UDP/TCP 互通。运行环境为 macOS ARM64；发布后的 CI、版本来源与校验值另行记录。

## v0.6.0 发布记录（2026-10-10）

固定源码及 crates.io 包来源 b167478d1da18b067cf82f6ce7da318fede5247f，实际下载包的 VCS 来源与 49 个文件核验通过。发布标签 86a334272d0c8665b857fb776a1c2936baa62048 与该源码仅差 Package.swift 的下载校验值。GitHub Actions 38030081694 成功：62 项 Rust 测试、Clippy、七组真实 Swift FFI、静态框架构建及远程包测试。

框架 SHA-256：ff294571334f78f72104a94e1d4f3193eb1fa9e39b6d6c0aa1b2f439f8ea02ac；测试服务 SHA-256：e24a1ed4e1b1c8f0fd5065bde809834d7154f9ab0acb4c308547dd34cbd2e7c4。三种交付物对应相同版本及确定源码，没有提交二进制到源码树。

本机真实 Swift FFI 另验证私有网卡 UDP/TCP 到 Rust 服务的 Fake-IP，以及同一路径转发至 Cloudflare DoH 的有效域名和错误 DNSSEC 签名拒绝。初次执行曾超时，后续测试成功但原因尚未定位；此互通不是长期可用性保证。另一台物理局域网设备、防火墙权限交互、最低 macOS 14 运行及其他平台切片仍未验收。范围不包含 TUN/网关转发。

授权后复测（2026-10-10）：用户确认系统曾申请两次权限，现已授权。相同代码在本机私有网卡上重新执行 TCP/UDP Fake-IP，以及 TCP/UDP 到公开 Cloudflare DoH 的转发和错误 DNSSEC 签名拒绝，均通过，本轮未出现超时。初次超时可能与尚未授权有关，但没有权限拒绝日志及提示类别证据，尚不能确认根因。不替代拒绝/撤销权限、其他物理设备或最低系统验收；功能版本及发布二进制不变。


默认 DNS 最近解析统计：Rust Resolver::last_query 与 C ABI dns_last_query 提供同配置存活解析器共享的只读结果；Swift 可读取 durationMilliseconds、fromCache、successful。不主动探测；缓存命中显示 0 ms，Fake-IP 合成不更新；记录默认首服务器的实际尝试（包括失败），回退服务器不冒充默认服务器。不同配置隔离，最后一个解析器释放后统计消失，最多 16 项，每个键最多 8192 字节。不记录域名和响应；进程外解析不计入。

## v1.8.0 验证

作者：可莉唯一的狗。2026-10-11，Apple Silicon / macOS 本机。

Rust 148 项测试通过（私有节点互通 1 项未提供环境而跳过）；严格 Clippy 通过。真实 Swift FFI 十二组全部通过，其中 DNS 检查实际 UDP、TCP、DoT、HTTP/2 DoH，以及最近查询、跨句柄同配置共享和缓存 0 ms。独立固定服务端 37 个指纹/密码套件组合互通通过。资源注册表使用弱引用，不延长解析器生命周期，注册项最多 16 个、键最多 8192 字节，统计不含域名或响应。

这项能力不增加 YAML 字段，完整示例配置不变。退出进程后统计清空；取消途中尚未完成的查询不当成已完成记录；配置不同或没有查询时返回空。仅观察本进程 Rust 默认 DNS，不包含 macOS 其他进程和回退服务器的耗时；最低 macOS 14、Intel、移动平台与系统 TUN 未测。发布来源及二进制校验以同版本 Release provenance.json 为准。

## 错误协议示例与失败边界

`Resolver::respond(&[], true)` 返回标准 FORMERR 报文，这是 DNS 协议负响应，不是错误默认放行。构造该响应的编码失败必须返回 `Error::DnsResponse`（1003），不能转换成空报文成功。无回复上游使用 1002，统计锁中毒使用 1014，系统报告内存不足使用 1006。可运行验证见 `cargo test --test error_contract`、`cargo test --lib dns::error_semantics_tests` 和 `cargo test --test dns_service`。
