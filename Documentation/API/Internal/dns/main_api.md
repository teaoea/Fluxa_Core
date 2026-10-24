# 主配置DNS引导日志内部API

API分类：内部自用；来源src/dns/main.rs；版本2.16.1-dev。

Resolver::from_main(&Dns,nodes,tls,Option<Scope>) -> Result<Option<Arc<Resolver>>>：配置关闭仍返回None；准备时原子初始化必要端点，显式引导解析沿用Scope。成功仅构建候选，不发布日志配置或授予VPN接管。

Plan::lookup(index,name,kind,runtime,Option<Scope>)：按原DNSSEC、显式system、连接/端点有效期逻辑查询；空端点初始化及TTL到期build传递Scope到bootstrap。bootstrap对配置引导Resolver调用resolve_logged，显式OS引导调用受会话/预算约束的系统日志包装。每个实际子查询独立query_id，继承原连接ID，不把原父查询ID复用给子请求。

Plan不保存消费者；initialize/build/bootstrap只按调用传递Scope，不改变引导顺序、实际TTL或失败原子性。setting(index)只是借用原配置供日志标记，不授予任何权限。失败日志不延长Authority、不能改变Core错误或回退DIRECT。原始配置文本仍保留在有限元数据预算中，没有新增日志集合。例：providers::Runtime.apply_files传当前Journal，Controller驱动的候选初始化可传单独DNS观察Journal，不将候选配置切换到该Journal。测试为test_dns_main和test_vpn_preflight，实际报告同名。
