# 路由引擎

作者：可莉唯一的狗。版本 v0.7.0。

## 语义

Router::compile 编译不可变快照，evaluate 接收域名/数字目标 IP（至少一个）、目标端口和 tcp/udp。模块没有 DNS 解析、嗅探、连接或系统路由副作用。Config 和 Target 拒绝未知字段；输入错误在任何模式下都拒绝。只有提供了 IP 才匹配 CIDR，不为了匹配规则自行解析 DNS；Fake-IP 到真实域名的接入留给共用出站阶段。

规则按原始顺序首次命中；一条规则匹配一种条件，不把相邻规则当成 AND。domain 精确匹配；domain-suffix 包含自身及有标签边界的子域。ASCII 大小写和单个末尾点归一化，国际域名使用显式 Punycode，拒绝数字地址写入域名字段。ip-cidr 支持 IPv4/IPv6，包括 /0 与单地址前缀，要求网络对齐，不混用地址族；IPv4 映射 IPv6 保留 IPv6 地址族。destination-port 为 1–65535 的单端口或升序闭区间，transport 为 tcp/udp。

模式 rules 使用规则和未命中策略，direct 明确直连，global 指向默认代理出站。未命中默认 reject，不能把未知规则或缺失代理出站转换为直连。代理规则可指定 outbound，未指定时使用配置默认 outbound；所有代理规则、代理未命中策略及 global 都必须有对应出站标识，缺少则编译拒绝。出站标识 1–64 个 ASCII 字母、数字或 -_.，不是 URL、凭据或可执行配置。

Decision 返回 policy、outbound、从零开始的 rule_index 和原因 mode/rule/fallback。返回代理决策只是解释规则；不表示该协议已经实现或出站存在。check_support 对 direct 允许，对 reject 返回 Rejected，对所有 proxy 返回 Unsupported，绝不隐式直连；也不执行连接。后续出站注册表须检查目标出站实际存在及协议能力。当前 HTTP/MITM 尚未自动执行该路由，共用出站阶段统一接入。

## YAML 与接口

用户主配置延续现有 rules，增加可选 routing。远程规则仍使用 type/value/policy，指定出站为可选 outbound；订阅/插件不得覆盖主配置 routing 设置。

```yaml
schema-version: 1
kind: configuration
name: 路由示例
routing:
  mode: rules
  fallback: reject
  outbound: node-default
rules:
  - type: domain
    value: api.example.com
    policy: reject
  - type: domain-suffix
    value: example.com
    policy: proxy
    outbound: node-specific
  - type: ip-cidr
    value: 192.168.0.0/16
    policy: direct
  - type: destination-port
    value: 80-443
    policy: direct
  - type: transport
    value: udp
    policy: reject
```

Rust Config 是 mode/fallback/outbound/rules 的类型结构；C ABI 复用 ABI 1 的 route_configure/evaluate/check/close，能力位 9 只代表路由决策。Swift 值类型 RoutingSettings、RoutingConfiguration 和 RoutingTarget 只传数据，NativeRoutingClient 管理 FFI。配置先完整编译再替换，失败保留旧快照；关闭可重复，关闭后评估拒绝。

## 资源及验证边界

每个快照最多 10000 条规则，每条值最多 253 字节，出站最多 64 字节；全局快照预算 16 MiB，基础预留 1 KiB，加每条 1 KiB，预算随最后一个所有者释放。规则编译后不可变，匹配过程 O(规则数)，不保存连接、正文或查询历史。CIDR 与 DNS 服务共用，主机规范化与 HTTP 共用；没有新增 unsafe。JSON 输入及返回句柄继续受共用 FFI 上限限制。

验证覆盖顺序与域名后缀边界、IPv4/IPv6 前缀、缺失元数据、端口范围、传输类型、模式、未命中和指定出站、无效配置、并发快照、最后所有者及限额回收、有界随机值不 panic。真实 Swift FFI 覆盖相同关键决策、失败保留、明确拒绝代理/拒绝策略、关闭及 100 次生命周期。路由没有独立网络协议，不能以决策测试冒充 TCP/UDP 转发互通；既有 HTTP/DNS/MITM 协议和 FFI 回归另行运行。只验证当前 macOS ARM64，最低 macOS 14、其他平台和系统 TUN 均未验收。

本轮发布前检查：70 项 Rust 测试、八组真实 Swift FFI、cargo fmt 与严格 Clippy、SwiftFormat 与严格 SwiftLint 已通过。远程二进制和 App 接入验证记录在发布后补充。

## 发布验证记录（2026-10-10）

v0.7.0 构建源码 793d9ad1753c6abe73011ca69e27188cb1e12daf，最终发布标签 07d9248dfb7eefef73cdd21e6374d54bdd2a48b9，二者仅有 Package.swift 的二进制校验值差异。crates.io 实际下载包核对 VCS 来源及 53 个文件；GitHub Actions 38032863652 成功，70 项 Rust 测试、八组真实 Swift FFI、严格 Clippy 与远程二进制包 11 项测试通过。发布前 SwiftFormat/SwiftLint strict 通过。

框架 SHA-256 cfb045a58f6d3eef5a958d07d34b911032d120b74118d3789b34cb7975f1643c，测试服务 SHA-256 fe35522671ccddb90a36a882c646a6b9f66176be622b366960c2690fdb260db7。Fluxa 固定远程 0.7.0，43 项客户端测试、严格 Swift 检查与 Xcode 包解析通过，新增诊断入口不建立连接。测试环境 macOS 27.2 ARM64 / Xcode 27.0；最低系统和其他平台尚未实际运行。原生界面检查遇到 Mac 锁定，GUI 操作未验收。没有真实系统 TUN/网关转发或代理出站互通结论，未合并 main。
