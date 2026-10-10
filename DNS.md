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
