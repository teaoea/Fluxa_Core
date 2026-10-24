# VPN候选DNS观察内部接口

API分类：内部自用；来源src/outbound/configuration.rs；版本2.16.1-dev。Manager::prepare_vpn_candidate(document,client,resources,Option<Arc<Journal>>)仅供新建未发布候选管理器使用。额外参数只供DNS初始化观察，不将外部Journal替换成候选管理器的日志状态，也不应用候选密钥/等级。事务检查与失败拒绝保持原有语义，不能把候选准备成功称为VPN运行。

Controller的preflight Driver传入当前消费者日志；独立vpn::Candidate未提供消费者时传None，不建立Core日志历史。取消/停止、资源所有权、准备预算与只读输入均保持原契约；日志不能授予活动权限。测试见test_vpn_preflight/test_vpn/test_providers。
