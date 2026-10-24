# TCP会话中的DNS日志接入

## 职责（干嘛的）
来源src/tun/tcp/session.rs。本页聚焦本次DNS分帧日志改动：TCP会话在已有Context、取消信号和虚拟DNS所有权校验下，取出完整DNS长度帧，交给Resolver::respond_logged，再沿原TCP响应路径发送。既有普通TCP转发不因本次文档新增能力承诺。

## 目标（想达成什么）
保持既有握手/帧长度/缓冲限制和所有权检查；每个完整DNS问题生成独立query_id，并使用Context内Manager的消费者。没有代理出站连接的虚拟DNS帧不伪造connection_id。日志继续由原Journal非阻塞输出。

## 禁止行为（不允许做什么）
不得在DNS归属或会话已撤销后继续解析，不因日志绕过取消或原缓冲上限，不把报文/状态机成功称为系统接管。不得向Core添加日志文件、历史或远程收集；消费者失联不改变TCP/DNS处理。

## 流程、错误与验证
Context持现有Manager和DNS解析器引用，活动权限仍由既有会话检查。完整2字节长度头及12–4096字节问题到齐后，再在取消分支中调用respond_logged；返回前后复核DNS所有权，沿原有响应长度限制和TCP序号逻辑发送。原InvalidInput、Cancelled、DNS失败分类保持不变。使用示例：虚拟DNS地址的TCP查询，Scope来自当前Context；外部报文不能提供日志ID。

日志内部API见Documentation/API/Internal/dns/wire_api.md。验证见test_tun_tcp_session::dns_tcp_length_framing_and_real_resolver及既有关闭/取消回归；不代表真实系统TUN验收。
