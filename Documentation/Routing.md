# 路由引擎

作者：可莉唯一的狗。

当前 dev 来源及优先级以 [运行时规则说明](RuntimeRules.md) 为准；本文件后续版本记录描述历史行为，不作为新主配置的现行规则。版本 v0.7.0。

> 当前开发中的运行时规则语义与索引见 [RuntimeRules.md](RuntimeRules.md)。下文为已发布旧版历史记录，首次命中与旧配额不适用于新运行时索引。

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
  - type: DOMAIN
    value: api.example.com
    policy: reject
  - type: DOMAIN-SUFFIX
    value: example.com
    policy: proxy
    outbound: node-specific
  - type: IP-CIDR
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

## 路由执行（v1.1.0）

新增 Manager::connect_routed、outbound_route C ABI 与同引擎的路由/DNS配置及实际连接。目标只能包含一个域名或数字 IP，端口单独给出；Fake-IP 输入必须属于当前解析器并携带原映射代次。返回实际地址、会话标识与最终决策，未配置路由、未配置所需 DNS、被拒绝或逻辑代理出站未实现时明确失败。

先按规则顺序尝试不依赖 DNS 的决策。遇到尚无地址的前序 CIDR 规则，暂停匹配并解析，绝不跳过它匹配后面的域名规则。能确定的拒绝与未实现代理在 DNS 前拒绝；真实 DNS 得到候选后，对每个实际地址按原顺序重新匹配。当前采用保守策略：任何候选被拒绝或指向不可用代理，则整次连接失败，不通过切换地址族绕过规则。所有候选均允许后才拨号，复用有界候选连接、截止时间与取消。UDP socket 创建仍不保证可达。

Fake-IP 还原为原域名后匹配域名及真实地址的 CIDR，不对合成地址误做真实出口匹配；未知/过时代次不触发连接。正常域名路由使用真实 DNS，即使解析器配置启用合成服务，也不把合成结果当对端。本次连接固定传入的不可变路由及解析器快照，配置更换不会改变已建立的目标。活动连接的统一配置与资源生命周期下一项单独交付。

Fake-IP 专用预分配复用 DNS 的输入验证和合成响应构造，排除项直接拒绝，不再为无法生成映射的请求向上游查询；常规 DNS 查询的排除项仍真实解析。新增七项真实套接字测试覆盖前置拒绝、规则优先级、数值 UDP/模式、混合地址策略、Fake-IP/代次与无网络预分配。HTTP/MITM 尚未统一接入，不代表 TUN 或网关转发。

发布前本机验证：94 项 Rust 测试、严格 Clippy、九组实际 Swift FFI、SwiftFormat 及严格 SwiftLint（零违规）通过。包含 DNS 阶段的截止时间和跨线程取消；最低系统及其他平台未运行。

## 路由决策执行（2026-10-10）

内核 v1.1.0 构建源码 8731c18c8937b355c4afb9e45dcd425e38966a96，最终标签 6b41fedb9ed2e82527f2bfa210b11738d6d1ab0c，仅有 Package.swift 二进制校验值差异。实际 crates.io 包 VCS 一致，61 个归档条目不包含 Swift 或二进制；GitHub Actions 38048325048 成功，94 项 Rust 测试、九组真实 Swift FFI、严格 Clippy 与远程 Swift 包 11 项测试通过。发布前 SwiftFormat 和严格 SwiftLint 零违规。

框架 SHA-256 60fe0b3f6fb0f4d9f3bfa157eafef10d615a2f9e5c6b4ac0dc0cafe70fa0d913；测试服务 SHA-256 48e91317264f5b4edc4222fcbbea82b597e4df7b7259da3756821fc3943088d0。Fluxa 0.9.0 / build 9 固定远程 1.1.0，49 项客户端测试零失败、无跳过；验证 YAML → 四种 DNS 传输 → TCP/UDP 实际路由连接、前序地址拒绝、域名放行优先、未实现代理拒绝及 Fake-IP 还原后匹配。格式、严格 SwiftLint、资源一致性和 Xcode 27 固定依赖解析通过，无本地内核目录引用或 Rust 构建。

规则按来源优先、同来源类型优先执行；域名阶段命中跳过 IP，未命中才解析真实地址。IP REJECT 仅排除该候选，允许其他候选继续；不可用代理明确失败，不默认直连。已能确定的拒绝在 DNS 前失败。Fake-IP 专用预分配不访问上游，排除项明确拒绝；连接阶段还原域名并用真实 DNS。界面展示真实对端及命中规则，仅手动发送本次输入，不保存历史。

环境为 macOS 27.2 ARM64 / Xcode 27.0，页面与 macOS 宿主已编译。最低 macOS 14 实际运行、其他架构/平台、GUI 交互、真机 VPN、TUN/网关及远端代理协议未验收。活动连接管理与 HTTP/MITM 共用链迁移待独立交付。main 未合并，用户标注图不提交。
