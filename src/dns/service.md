# DNS UDP/TCP服务

## 职责（干嘛的）
来源src/dns/service.rs。Config验证监听范围与来源网段；Service提供受控启动、状态与停止，Control由解析器登记并确定性关闭。start_logged与公开start使用同一监听实现，仅增加本次日志Scope。

## 目标（想达成什么）
维持实际双协议绑定、服务/连接/请求预算、来源限制和停止后资源释放；UDP请求与TCP长度帧复用respond_logged，查询事件流由消费者控制。TCP多问题各有独立query_id，不伪造代理出站connection_id。

## 禁止行为（不允许做什么）
不扩大LAN授权、不绕过配额、不用端口存在证明服务完整可用。日志满、无消费者或失联不得阻塞/改变应答，不写日志文件、收集历史或配置远程日志服务器。旧Resolver Arc不能维持已撤销监听活动权限。

## 流程与生命周期
校验配置和ResolverOperation，取得有界服务配额，真实绑定UDP/TCP并登记Control；后台任务受取消信号与有界JoinSet管理。UDP克隆本次Scope至每个请求，TCP传入会话并按帧克隆，实际问题由Resolver完成。停止发送取消、等待任务和子任务退出，再释放句柄与配额。错误保留原InvalidInput/ResourceLimit/DnsTimeout等协议，不因日志构造成功转成业务成功。

内部API见Documentation/API/Internal/dns/service_api.md，标准日志字段见External/logging_api.md。调用示例为Engine DnsServiceStart传Scope、普通Resolver::serve默认None。测试见test_dns_service/test_dns_logging；网络授权及关闭语义回归沿用既有用例，不宣称最低系统或外部DNS互通。
