# DNS 解析

作者：可莉唯一的狗。

v0.4.0 提供 UDP/TCP、DoT 和 HTTP/2 DoH 查询。UDP 截断回复改用同一上游 TCP；多个上游按配置顺序、共享总期限切换。加密与明文上游不能混合，错误证书、错误域名和 DNSSEC 验证失败不降级。bootstrap 必须使用数字 IP 和端口，不使用系统 DNS/hosts。

支持 A、AAAA、CNAME 和标准记录查询；拒绝 ANY、AXFR、IXFR、OPT 和未知类型。正缓存最多一小时；NXDOMAIN/NODATA 使用 SOA 的 TTL/minimum，最多五分钟。每个实例最多 128 条缓存，全局预算 16 MiB，每条预留 64 KiB；全局最多 16 个客户端和 16 个在途查询，超过上限拒绝。查询总期限 100–30000 毫秒，上游 1–8 个。关闭后查询持有的资源在查询结束时释放。

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

Fake-IP 和局域网 DNS 服务按用户要求分别实现、测试和发布，尚未交付。未实现 DoQ/HTTP3、权威 DNS、mDNS 或根锚自动更新。HTTP 调试服务现有系统域名解析将在共用出站阶段接入本模块。查询能力不代表 TUN、网关或系统代理已可用。仅发布 macOS ARM64 切片；最低部署系统及其他平台尚未运行验证。

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
