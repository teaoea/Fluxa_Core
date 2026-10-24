# DNS实时日志内部API

API分类：内部自用。来源：src/dns/logging.rs。版本2.16.1-dev。

- Scope::new(Arc<Journal>, Option<u64>, Option<u64>)：由内核入口传递本次消费者及真实连接/配置关联，不授予查询权限，不能来自普通JSON输入。
- Trace::begin(Option<Scope>, &Query) -> Option<Trace>：无Scope或ID耗尽返回None，仅停止本次日志，不改业务返回值。begin生成query_id并输出started。
- hit(Event, source, &Answer)：只在已实际命中的hosts/Fake-IP/TTL缓存分支调用，保留真实rcode/TTL/缓存/DNSSEC摘要。
- upstream(index, label, Option<Error>)：开始实际尝试时selected，失败时failed；索引从0开始，label是原配置项。它不是实际线协议/服务器身份的证明。
- finish(&Result<Answer>)、finish_system(&Result<Vec<IpAddr>>)及finish_probe(&Result<()>)：正常返回前记录终态。系统接口不提供的TTL/RCODE/DNSSEC字段为null。Drop在未finish时记录cancelled，不另起任务、不等待消费者。

Scope仅持有日志输出Arc，不能延长Resolver活动权限；ResolverOperation/Authority仍验证所有查询。不得绑定可变日志接收方到共享Resolver，不收集历史或新增密钥副本。事件字段/长度/加密协议见[外部日志API](../../External/logging_api.md)。错误沿用实际ErrorInfo；负答案不伪造Error。例：连接管线传入Scope::new(manager.logs.clone(),Some(connection_id),Some(revision))；独立API无连接ID。测试见tests/test_dns_logging.rs及同名报告。

Scope通过Trace::scope克隆给运行期重建的引导查询，子查询各自生成query_id并继承真实connection_id。预检final source为preflight；其目标是验证真实地址，否定/无地址沿用原预检错误；普通解析的否定Answer仍为completed。

Rust unwind导致未完成作用域销毁时输出dns-query-failed/Internal，普通Future丢弃输出cancelled；不把panic混同用户取消，也不宣称可捕获进程abort或OS崩溃。验证见panic_during_query_trace_is_failed_not_cancelled。
