# logging.rs 代码说明

## 职责（干嘛的）
只生成并实时输出当前结构化事件，消费者拥有保存与访问。

## 目标（想达成什么）
提供失败关闭的可选公钥加密，保持代理处理无等待消费者的依赖。

## 禁止行为（不允许做什么）
不保留日志集合或历史、不写文件、不远程收集、不持有接收方私钥，不因发送失败降级明文或改变代理处理结果。

## 流程与不做的事
候选配置严格校验公钥并准备HPKE exporter密钥；提交后采用。事件分配递增序号、try_lock获取当前输出、固定栈缓冲编码、非阻塞发送。加密上下文共享nonce原子计数，环形队列、历史读取、文件、远程收集和解密均不属于Core。没有消费者时继续业务，发生背压直接丢弃事件。

所有权：Manager拥有Journal，Journal仅持消费者发送端与可选Cipher；Receiver由宿主持有，配置候选Arc保留有限密码上下文。私钥从不进入Core。没有新增业务unsafe。随机源使用ring；RngCore不能返回错误，因此随机源失败设置失败标志，setup结束后拒绝整份候选，绝不发布失败上下文。不能宣称所有依赖中间密钥均可靠擦除。

## API、错误与测试
完整字段、线协议、失败边界与未验收范围见[外部API](../Documentation/API/External/logging_api.md)；内部协作见[内部API](../Documentation/API/Internal/logging_api.md)。配置错误使用InvalidInput/ResourceLimit，密码准备失败Internal；发送失败仅丢弃当前事件。测试集中在tests/test_logging.rs及同名报告。

## DNS实时日志（2.16.1-dev）

DNS事件携带可选DnsDetails和query_id，使用同一4096字节JSON上限、加密协议和非阻塞消费者。 完整字段及验证边界见`Documentation/API/External/logging_api.md`和`src/dns/logging.md`。
