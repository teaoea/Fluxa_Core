# 出站建连DNS关联内部API

API分类：内部自用；来源src/outbound.rs；版本2.16.1-dev。

Manager::establish_with(snapshot,deadline,future)的内部拨号闭包形状为AsyncFnOnce(Option<&ResolverOperation>,connection_id:u64)->Result<Dialed>。connection_id由Core在路由/拨号前分配，闭包只能用于当前操作关联日志；ResolverOperation仍验证实际DNS权限，日志ID不授予资源权。已有establish_guarded、会话撤销、超时和失败返回保持原契约，不从日志决定DIRECT或成功。

resolve_bootstrap_system(name,owner,Option<dns::logging::Scope>)只给显式引导使用，复用受预算/会话约束的系统解析日志包装；不作为普通DNS失败回退。OS未提供的服务器、RCODE、TTL和DNSSEC为未知，不制造虚假数据。测试见test_outbound_dns/test_dns_main/test_dns_logging。公开DNS消费者接口单独位于External/outbound/resolution_api.md。
