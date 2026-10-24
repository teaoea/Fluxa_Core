# DNS查询与日志消费者接口

API分类：外部调用API；来源src/outbound/resolution.rs；版本2.16.1-dev开发接口。调用对象outbound::Manager。

`pub async fn resolve_dns_query(&self, resolver:&dns::Resolver, query:dns::Query, real:bool) -> Result<dns::Answer>`：使用此Manager的实时日志消费者和已提交加密配置查询给定解析器。real=true等价resolve_real，不分配Fake-IP；false等价resolve，保留hosts/Fake-IP优先级。Manager已停止或解析器准入失效沿用InvalidState；名字/类型、上游/DNSSEC、超时和资源错误保持原协议。NXDOMAIN为rcode=3的Answer，不是成功解析到地址，也不是新增执行错误。

调用前通过Manager::set_log_output/Unix C接口注册消费者、set_log_level或主配置选择等级。查询没有代理连接时connection_id为null，每次query_id独立。Resolver本身不保存消费者或历史，因而两个Manager可使用同一解析器而不串日志；DNS答案缓存仍按既有Resolver实例共享。主配置DNS一致时带其revision，独立传入解析器不伪造配置关联。

示例：`cargo run --example dns_logging`，只创建本机UDP夹具，宿主示例读取其自建队列并打印JSON。Core不打印、不保存或导出日志。现有Resolver::resolve/resolve_real签名不变，直接调用不绑定日志消费者；需要实时日志时通过此Manager入口。加密/事件格式见[日志API](../logging_api.md)。测试为tests/test_dns_logging.rs、test_dns.rs及同名报告；外部DNS/Swift接入与最低系统未由此验证。
