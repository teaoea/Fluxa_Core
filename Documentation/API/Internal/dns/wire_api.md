# DNS报文日志内部入口

API分类：内部自用；来源src/dns/wire.rs；版本2.16.1-dev。Resolver::respond_logged(bytes:&[u8], udp:bool, Option<Scope>) -> Result<Vec<u8>>。公开respond仍调用同一实现并默认无Scope。

保持原会话准入、12–4096字节查询报文、单问题/递归/IN类/EDNS与签名限制及FORMERR/REFUSED等响应规则。只有被协议层接纳的查询进入resolve_logged；畸形包与协议层拒绝不伪造DNS查询或上游事件。不得从报文自报connection_id或消费者身份。真实查询结果、缓存与错误沿用原有编码，不从日志字段重建DNS答案。用于本地UDP/TCP服务与TUN虚拟DNS端点；TUN DNS没有代理出站连接时connection_id为null，以query_id关联。测试见test_dns_service/test_tun_dns/test_tun_tcp_session。
