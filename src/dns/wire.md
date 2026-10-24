# DNS报文响应

## 职责（干嘛的）
来源src/dns/wire.rs。Resolver::respond/ respond_logged校验标准DNS请求，复用解析器并编码标准响应；encode、synthetic和malformed提供有界报文处理。日志只跟随通过协议检查的查询，不从展示字段重建DNS答案。

## 目标（想达成什么）
保留既有事务ID、EDNS容量、负答案与标准响应语义；本地服务和TUN虚拟DNS的每次实际解析均使用独立query_id。respond_logged携带当前调用的消费者，respond默认无消费者。无运行Scope时不保存或回放历史。

## 禁止行为（不允许做什么）
不能在退休/关闭后继续服务，不能绕过12–4096字节和协议校验，不把畸形包、非IN类、签名或非递归拒绝伪造成正常查询。不能从报文读取自报connection_id、权限或消费者。FORMERR/REFUSED等保持原响应，不产生虚构上游成功。

## 流程与资源
校验活动状态、帧长和消息，再校验单问题/操作码/EDNS。实际问题调用resolve_logged(Query,true,None,scope)，复用原DNS答案报文；响应保留既有截断与大小限制。错误沿用InvalidInput/DnsResponse等既有契约。临时缓冲跟随本次调用，Core无日志列表或持久化。调用方例：TUN虚拟DNS端点传入当前Manager的Scope；未持有代理出站连接时connection_id为null。

API见Documentation/API/Internal/dns/wire_api.md；验证见test_dns_service/test_tun_dns/test_tun_tcp_session及同名报告。仅覆盖本次DNS日志接入，不把回环报文验证称为真实系统VPN验收。
