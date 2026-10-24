# 出站配置与DNS观察

## 职责（干嘛的）
来源src/outbound/configuration.rs。NetworkSnapshot承载一次捕获的配置、DNS/路由/节点/资源参数与代次；Manager完成候选准备与受约束原子提交。本页聚焦DNS日志观察参数，不把其他未实现能力宣传为已完成。

## 目标（想达成什么）
prepare_vpn_candidate(document,client,resources,dns_logs)仍只在新建未发布候选管理器上准备。额外Journal只观察DNS初始化；Runtime.candidate保存独立日志引用并按查询传递Scope，候选日志字段不覆盖当前运行消费者。旧/新配置、DNS退休、会话操作与原子性规则保持原契约。

## 禁止行为（不允许做什么）
不得用日志引用授予活动权限，不提前发布候选加密密钥或等级，不把准备完成当系统VPN运行；不以日志失败掩盖真实配置错误，不写配置、下载落盘或保留DNS日志历史。

## 流程、API与验证
Controller Driver向prepare_vpn_candidate传当前Journal，独立无消费者Candidate传None。方法仍先检查文档的VpnCandidate准备边界和管理器状态，创建资源Runtime并按原取消/准备方式完成候选。普通配置准备使用owner自身Journal。活动配置日志切换仍只由完整配置提交控制。来源、错误、资源与关闭细节沿用src/outbound.md及src/providers.md，不以Arc引用数量推定权限或清理完成。

内部契约见Documentation/API/Internal/outbound/configuration_api.md与Internal/providers_api.md；测试见test_outbound_configuration/test_providers/test_vpn/test_vpn_preflight及同名报告。示例调用为Driver在不可外部伪造的作用域下传入dns_logs，普通JSON不接受该参数。
