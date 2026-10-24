# 出站解析桥接

## 职责（干嘛的）
来源src/outbound/resolution.rs。Family/DomainTarget/FakeIPTarget/Opened表达请求与结果；Manager域名/Fake-IP入口把已验证DNS结果转换为真实出站候选。resolve_dns_query允许宿主使用Manager的日志消费者执行DNS查询，不暴露内部授权对象。

## 目标（想达成什么）
保持DNSSEC/资源等故障优先级、地址上限、Fake-IP代次恢复及受管理建连；实际目标/节点解析与连接日志共享同一connection_id。独立DNS查询只分配query_id，解析器不保存消费者。系统解析只输出已知结果，不猜测服务器、TTL或DNSSEC。

## 禁止行为（不允许做什么）
启用配置DNS后不得隐式回退系统解析；不得把合成地址当真实出站，不以另一族成功掩盖DNSSEC/资源错误。日志Scope不替代ResolverOperation，不因消费者断开改业务结果；不写入、历史查询或远程收集。

## 流程、接口及资源
resolve按原Family构造A/AAAA，继承属于该解析器的已准入令牌，其他调用重新准入。connect_domain通过establish_with传入的真实connection_id构造Scope，再按既有候选拨号。系统阻塞线程仍持原会话操作与内存配额，日志Future取消不表示OS调用已结束。相同已捕获快照提供revision；独立传入Resolver不伪造配置归属。

公开API与示例见Documentation/API/External/outbound/resolution_api.md；内部Scope桥接见Internal/outbound/resolution_api.md。测试为test_dns_logging/test_outbound_dns/test_outbound_execution。DNS真实成功不表示节点可用或系统VPN已接管。
