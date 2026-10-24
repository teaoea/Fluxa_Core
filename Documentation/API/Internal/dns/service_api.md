# DNS服务日志内部入口

API分类：内部自用；来源src/dns/service.rs；版本2.16.1-dev。Service::start_logged(Config,Arc<Resolver>,Option<Scope>) -> Result<Service>。公开start默认无Scope。保留原监听网段授权、每来源限制、服务/连接/请求配额和确定性停止；日志Scope只随已授权任务传递。UDP一包一次查询，TCP一帧一次查询，各查询独立query_id，不伪造出站connection_id。Resolver.close仍取消并等待服务任务，旧Arc不延长服务活动权限。日志满/断连不改变DNS应答或重启监听。无文件写入、历史或远程日志功能。C入口DnsServiceStart由当前引擎传Scope。验证见test_dns_logging/test_dns_service/test_ffi。
