# Fluxa 网络内核

作者：可莉唯一的狗。

`libfluxa_core` 是使用 Rust 开发的跨平台网络内核，提供统一的网络处理与资源管理能力。

## 当前能力

- HTTP 代理：请求转发、CONNECT 隧道与分块解码。
- HTTPS：TLS 1.2/1.3、上游证书验证及请求/响应处理。
- MITM：精确域名解密、证书签发及请求/响应捕获。
- 流量改写：请求头、响应头和文本响应改写。
- JavaScript：受限脚本执行、JSON 转换、内置 JSON 响应字段处理及执行隔离。

- DNS：UDP/TCP、DoT/DoH、手动上游、正负缓存、DNSSEC、Fake-IP 双向映射及有访问控制的局域网 UDP/TCP 服务；详见 [DNS.md](DNS.md)。
- 路由：顺序匹配域名、IP/CIDR、目标端口与 TCP/UDP，显式模式和指定出站；当前只作决策，详见 [Routing.md](Routing.md)。
- TUN 基础：IPv4/IPv6、TCP/UDP 数据包检查、校验和验证与有界会话；尚不支持数据包转发。

通过所有权和 RAII 管理资源，为连接、缓冲区、日志和脚本运行时设置上限。

## 后续计划

完善共用出站、TUN 转发、网关与代理协议；首个协议目标为 VLESS + TCP + REALITY + Vision。

## 许可

自有代码采用 MIT，详见 [许可声明](LICESE.md)。

共用数字地址 TCP/UDP 出站支持双向数据、半关闭、超时及取消，拒绝和未实现代理不触网。当前不解析域名、不自动执行路由，详见 [Outbound.md](Outbound.md)。
