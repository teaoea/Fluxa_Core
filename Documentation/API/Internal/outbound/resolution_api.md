# 出站DNS日志内部关联

API分类：内部自用；来源src/outbound/resolution.rs；版本2.16.1-dev。resolve(resolver,name,family,real,permission,Option<Scope>)复用同一DNS桥接，已登记令牌仅在owns_operation为真时使用。A/AAAA并行或顺序行为、失败优先级、地址上限和显式系统回退保持原契约；每次DNS查询独立query_id，共用真正connection_id。resolve_system_logged包裹现有有预算/会话令牌的OS解析，只有显式授权路径调用；不推测OS实际DNS服务器、RCODE/TTL/DNSSEC。Future被取消只标记观察者取消，不假装阻塞OS调用已经结束；真实权限/配额仍移动至阻塞线程并按原契约等待释放。

establish_with与系统引导契约见[出站内部API](../outbound_api.md)。低层connect/connect_fake_ip的DNS解析关联同一次连接，不二次分配连接ID。日志失败不改变结果或默认DIRECT。测试见test_outbound_dns/test_outbound_execution/test_dns_logging。外部入口见External/outbound/resolution_api.md。
