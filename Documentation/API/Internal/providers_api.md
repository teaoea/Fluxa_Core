# 配置资源准备的DNS观察API

API分类：内部自用；来源src/providers.rs；版本2.16.1-dev。Runtime::new(owner,tls)默认使用owner的Journal观察配置引导；Runtime::candidate(owner,tls,Option<Arc<Journal>>)可使用独立传入的当前消费者观察DNS。apply_files只将该引用变成本次Scope，传给Resolver::from_main；它不向Journal应用候选level/encryption/public-key。

日志引用不包含Manager活动资源权，不改变原Weak<Manager>所有权、只读文件访问、编译、原子提交或停止机制。该引用不保存事件/查询历史，也不引入文件写入/远程收集。消费者关闭时Journal仍按原非阻塞丢弃策略工作，不能由日志失败伪造配置成功。测试见test_providers/test_dns_main/test_vpn_preflight及同名报告。
