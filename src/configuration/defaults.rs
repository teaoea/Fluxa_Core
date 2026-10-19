//! Core 维护的安全完整默认配置；不含示例节点、策略组或内部存储位置。
/// 当前发行语言为简体中文与英语；其他设备语言明确回退英语。
pub(crate) fn yaml(language: &str) -> &'static str {
    if language.to_ascii_lowercase().starts_with("zh") {
        CHINESE
    } else {
        ENGLISH
    }
}
const CHINESE: &str = r###"# Fluxa 默认主配置
# GitHub：https://github.com/teaoea/fluxa-app
# 问题反馈：https://github.com/teaoea/fluxa-app/issues
# 反馈问题时请附日志，并先移除节点凭据、私钥和敏感内容。
# 日志等级，silent 不记录日志。
log-level: silent
# 系统代理。
system-proxy:
  # 监听端口，0 表示关闭；启用时最小 1054。
  port: 0
  # 绕过地址。
  bypass: ["127.0.0.1", "localhost", "::1", "*.local"]
# 局域网共享。
lan-share:
  # 监听端口，0 表示关闭，不得与系统代理端口相同。
  port: 0
  # 访问账号。
  account: ""
  # 访问密码，避免在反馈中泄露。
  password: ""
# 运行模式：rule、global、direct。
mode: rule
# GeoIP 数据库。
geoip:
  # 默认不下载数据库。
  enabled: false
  # 启用前替换为真实 MMDB HTTPS 地址。
  url: "https://example.com/geoip.mmdb"
  # 每日更新时间，采用设备本地时区。
  update-time: "00:00"
# 地址族策略，默认优先 IPv4；不会因拦截而直连放行。
ip-strategy: prefer-ipv4
# 节点提供者保留字段，当前仅支持空列表。
proxy-providers: []
# 插件提供者，导入后按已支持能力校验。
plugin-providers: []
# 远程规则集，默认不订阅或下载。
rule-providers: []
# 本地规则；无节点的初始配置拒绝未匹配流量。
rules:
  # 唯一兜底规则；添加节点后由用户明确修改策略。
  - final,reject
# 出站资源上限。
outbound-limits:
  # 最大连接数，范围 1–64。
  maximum-connections: 64
  # 空闲超时，单位毫秒。
  idle-timeout-ms: 300000
# DNS 设置。
dns:
  # 关闭时使用系统 DNS；启用后不自动增加系统兜底。
  enable: false
  # 仅用于解析加密 DNS 上游及其代理节点的域名。
  bootstrap: []
  # 按顺序设置上游；需要系统 DNS 时显式加入 system。
  server: []
  # 每个上游的独立超时，单位秒。
  timeout: 3
  # 是否启用 DNSSEC 验证。
  dnssec: false
  # Fake-IP 设置。
  fake-ip:
    # 默认关闭 Fake-IP。
    enable: false
    # IPv4 地址池。
    ipv4-range: 198.18.0.0/16
    # IPv6 地址池。
    ipv6-range: fd00:198:18::/64
    # 不使用 Fake-IP 的域名。
    bypass: ["*.local", "*.lan", "time.apple.com", "*.push.apple.com"]
  # 静态域名映射，仅允许 IP 地址。
  hosts: {}
# 系统隧道设置；创建配置不代表已启动 VPN。
tun:
  # 默认关闭，系统接管需要独立授权与启动检查。
  enable: false
  # Apple 平台忽略自定义虚拟接口名。
  name: utun
  # 隧道 MTU。
  mtu: 1500
  # 不接管的目标网段。
  bypass: ["192.168.0.0/16", "10.0.0.0/8", "100.64.0.0/10"]
# HTTPS 解密设置；私钥不得写入主配置。
mitm:
  # 默认关闭解密。
  enabled: false
  # 不得自动跳过上游证书校验。
  skip-server-cert-verify: false
  # 启用捕获时是否保存请求与响应正文。
  capture-body: true
  # 明确允许解密的域名。
  hosts: []
  # 解密排除域名，优先于解密允许项。
  bypass: []
# 正文改写规则，默认无规则。
rewrites: []
"###;
const ENGLISH: &str = r###"# Fluxa default configuration
# GitHub: https://github.com/teaoea/fluxa-app
# Issues: https://github.com/teaoea/fluxa-app/issues
# Attach logs to issue reports after removing credentials, private keys, and sensitive content.
# Log level; silent writes no log files.
log-level: silent
# System proxy.
system-proxy:
  # Listener port; 0 disables it; minimum enabled port is 1054.
  port: 0
  # Bypass addresses.
  bypass: ["127.0.0.1", "localhost", "::1", "*.local"]
# LAN sharing.
lan-share:
  # Listener port; 0 disables it; must differ from the system proxy port.
  port: 0
  # Access account.
  account: ""
  # Access password; omit credentials from issue reports.
  password: ""
# Operating mode: rule, global, or direct.
mode: rule
# GeoIP database.
geoip:
  # Disabled by default; no database download.
  enabled: false
  # Replace with a real HTTPS MMDB URL before enabling.
  url: "https://example.com/geoip.mmdb"
  # Daily update time in the device local time zone.
  update-time: "00:00"
# Address family policy; prefer IPv4 by default; rejection never permits direct fallback.
ip-strategy: prefer-ipv4
# Reserved proxy providers; only an empty list is supported.
proxy-providers: []
# Plugin providers; imported entries are checked against supported capabilities.
plugin-providers: []
# Remote rule providers; no default subscription or download.
rule-providers: []
# Local rules; the initial configuration without nodes rejects unmatched traffic.
rules:
  # The sole final rule; change its policy explicitly after adding nodes.
  - final,reject
# Outbound resource limits.
outbound-limits:
  # Maximum connections, from 1 to 64.
  maximum-connections: 64
  # Idle timeout in milliseconds.
  idle-timeout-ms: 300000
# DNS settings.
dns:
  # Uses system DNS while disabled; enabling never adds an implicit system fallback.
  enable: false
  # Bootstrap addresses for encrypted DNS upstreams and their proxy hostnames only.
  bootstrap: []
  # Ordered upstreams; add system explicitly when system DNS is intended.
  server: []
  # Independent timeout per upstream, in seconds.
  timeout: 3
  # Whether to enable DNSSEC validation.
  dnssec: false
  # Fake-IP settings.
  fake-ip:
    # Fake-IP is disabled by default.
    enable: false
    # IPv4 address pool.
    ipv4-range: 198.18.0.0/16
    # IPv6 address pool.
    ipv6-range: fd00:198:18::/64
    # Domains excluded from Fake-IP.
    bypass: ["*.local", "*.lan", "time.apple.com", "*.push.apple.com"]
  # Static hostname mappings; values must be IP addresses.
  hosts: {}
# System tunnel settings; creating this file does not start a VPN.
tun:
  # Disabled by default; system takeover requires separate authorization and preflight.
  enable: false
  # Apple platforms ignore custom virtual interface names.
  name: utun
  # Tunnel MTU.
  mtu: 1500
  # Destination networks excluded from tunnel takeover.
  bypass: ["192.168.0.0/16", "10.0.0.0/8", "100.64.0.0/10"]
# HTTPS decryption settings; never store private keys in the main configuration.
mitm:
  # Decryption is disabled by default.
  enabled: false
  # Never skip upstream certificate verification automatically.
  skip-server-cert-verify: false
  # Whether to capture request and response bodies when capture is enabled.
  capture-body: true
  # Explicitly permitted decryption hostnames.
  hosts: []
  # Decryption exclusions; these take precedence over permitted hosts.
  bypass: []
# Body rewrite rules; empty by default.
rewrites: []
"###;
