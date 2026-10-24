# DNS解析日志内部入口

API分类：内部自用；来源src/dns/mod.rs；版本2.16.1-dev。调用对象Resolver。

resolve_logged(Query, synthetic:bool, Option<&ResolverOperation>, Option<Scope>) -> Result<Answer>：已有令牌必须属于本解析器；否则仍经operation登记准入。已退休实例拒绝新查询，获准在途事务按原契约继续，显式关闭仍撤销。日志begin覆盖准入失败，resolve_admitted/resolve_allowed维持原hosts→Fake-IP→TTL缓存→顺序上游逻辑；结果及取消通过Trace输出。synthetic=false禁止合成地址，等价原resolve_real；不是绕过DNSSEC或改系统回退。

serve_logged(self:Arc<Self>, Option<Scope>) -> Result<Service>：保持原服务授权与资源预算，把Scope逐次克隆至监听任务；它不把查询连接ID伪造成出站连接ID。公开resolve/resolve_real/serve保持原签名，无消费者Scope时不记录历史。既有resolve_operation/commit_operation仍验证原ResolverOperation，日志不能替代授权。失败不发布新资源、不更换有效配置。外部事件见[日志API](../../External/logging_api.md)。测试见test_dns_mod/test_dns_logging/test_dns_service。

preflight_upstreams(name,strategy,Option<Scope>)逐个实测所有必要上游并绕过hosts/Fake-IP/普通缓存，逐地址族生成探测query_id，保留原有超时、DNSSEC错误立即失败和无地址族回退规则。source=preflight表示地址可用性探测，不把普通NXDOMAIN改成全局DNS错误。
