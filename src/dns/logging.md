# DNS 日志作用域

## 职责（干嘛的）

来源src/dns/logging.rs。Scope携带本次调用的Journal、可选connection_id/revision；Trace只在单次查询Future内生成并输出事件，Drop记录未正常返回的取消。它不绑定解析器，不持有查询历史、答案缓存、套接字或活动授权，不裁决解析结果。

## 目标（想达成什么）

同一DNS查询的所有事件共享唯一query_id；出站目标/节点解析继承当前真实连接ID，多地址族查询拥有不同query_id。共享解析器的两个Manager各自使用本次Scope，不替换解析器全局日志消费者。事件只使用现有非阻塞Journal和已提交的加密设置，失败不改变原DNS结果。上游选择只代表实际开始该配置项尝试，不推定操作系统的实际服务器或UDP/TCP底层重试。

## 禁止行为（不允许做什么）

不新增全局查询缓存/日志历史/磁盘/远程收集/私钥；不为日志等待消费者，不反向从消费者决定DNS成功。不得将hosts、Fake-IP、真实缓存、否定答案、超时及DNSSEC失败混为一种结果，不隐式改DNS顺序、TTL、重试、验证或系统回退。不得把日志Scope当作解析器操作令牌；真正准入仍经ResolverOperation和Authority验证。

## 流程、资源和错误

begin从内核全局不可复用ID分配器取得query_id，保存有界原始文本、单调起点并输出dns-query-started。记录真实分支命中/上游索引和失败；finish保持原Result，输出completed或failed。Future提前丢弃只输出一次cancelled，不能宣称已经终止OS阻塞调用或所有底层任务。普通显式关闭返回InvalidState时为failed；外部取消丢弃Future为cancelled。

文本按UTF-8字节设上限，完整保留合法范围内的输入，不截断/替换：域名253、查询类型16、上游2048。超限字段为null，不改变原查询校验；整个事件JSON仍受4096字节限制，超限直接丢弃。堆中没有日志列表；固定Trace/Record空间跟随调用栈，消费者队列属于消费者。已有DNS答案TTL缓存与用户授权的日志历史是不同职责，不删除DNS缓存。

## API、调用和测试

内部接口见Documentation/API/Internal/dns/logging_api.md；外部字段见Documentation/API/External/logging_api.md。Resolver在配置查询、缓存及上游真实分支调用Trace，系统解析只记录已知事实。测试集中tests/test_dns_logging.rs；真实DNSSEC在test_dns，加密载荷在test_logging，C ABI/TUN入口在test_ffi/test_tun_dns/test_tun_tcp_session。原始结果见各同名报告。不证明外部DNS服务互通、最低系统或性能指标。

Rust unwind导致未完成作用域销毁时输出dns-query-failed/Internal，普通Future丢弃输出cancelled；不把panic混同用户取消，也不宣称可捕获进程abort或OS崩溃。验证见panic_during_query_trace_is_failed_not_cancelled。
