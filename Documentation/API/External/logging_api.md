# 实时结构化日志 API

API 分类：外部调用 API。版本：2.16.0-dev；能力位33。源码：`src/logging.rs`、`src/outbound.rs`、`src/ffi.rs`。开发接口，GUI尚未接入验收。

## 职责与配置

Core只生成并实时输出事件，不脱敏、不存储、不提供历史读取、文件写入、远程收集或解密能力。接收方拥有队列、文件、搜索、筛选、轮转、清理及导出。无接收方、队列满、锁竞争、编码失败或套接字背压时直接丢弃当前事件，不重试、不缓存。事件序号可有缺口。CPU序列化/加密仍有有限同步成本，不承诺硬实时或零开销。

```yaml
log:
  encryption: false
  public-key: ""
  level: silent
```

`encryption`默认false；false时明确使用明文。true时`public-key`必填，为标准Base64编码、65字节、首字节04的P-256 X9.63非压缩公钥，且必须为有效曲线点。密钥文本上限1024字节。错误拒绝整份候选配置，旧配置保留；加密失败不回退明文。旧`log-level`、`encryption-log`、`log.enabled`均不接受。私钥由消费者独立保存，Core没有私钥参数。

`silent`不输出；`info`输出全部类别；`debug`、`warn`、`error`各自只输出同名类别。运行时修改等级不需要重启，不改变加密设置。

## Rust与C入口

- `Manager::set_log_output(SyncSender<Record>) -> Result<()>`：消费者创建有界队列并持有Receiver，Core仅持有try_send端。**仅在明文模式输出**；加密开启时禁止通过该入口发送明文。接收方队列不是Core历史缓存。
- `Manager::set_log_level(LogFilter) -> Result<()>`、`detach_log_output() -> Result<()>`：运行时等级调整与解除消费者。停止状态拒绝设置。解除不会销毁消费者已经收到的数据。
- Unix/macOS：`nodus_core_log_attach_unix(handle, descriptor) -> int32_t`：传入已连接AF_UNIX SOCK_DGRAM发送端。Core复制FD并设置非阻塞，调用者仍拥有原FD；复制FD共享文件状态标志，因此原发送FD也会变成非阻塞。接收端为另一个端点，不受此设置影响。调用期间FD必须有效且不能被并发关闭/重用。拒绝负FD、非数据报、非Unix、未连接、失效句柄或关闭中的引擎。函数不获取文件路径、不连接远程地址。返回0成功；失败沿用C ABI错误查询契约。
- JSON command_v2：`{"operation":"log_configure","filter":"info"}`；`{"operation":"log_detach"}`。成功返回success；错误沿用error_info。不接受密钥、路径或历史查询参数。使用主配置事务更新公钥/加密开关。

Rust结构化Record字段：schema_version=1、core_version、sequence（实例内递增，不回绕）、timestamp_unix_ms（系统时间不可用为null）、elapsed_us（单调经过时间）、level、module、event、transaction、revision、phase、error_info。可空关联字段：connection_id、node、next_node、policy、rule_index、rule_version、rule_source、rule_sequence、route_reason、uploaded、downloaded、duration_ms。node最多128字节，传递原名称；error_info使用既有稳定错误代码与参数。rule_index是索引，不表示优先级。

事件包括配置请求/提交/失败、实际VPN阶段/失败/清理结果、连接创建/建立/失败/关闭、路由命中、实际选择策略（route-selected）、策略拒绝、节点连接/握手/成功/失败/重试及Core停止。connection_id在路由和拨号前分配，地址族/节点重试不换ID，公开会话句柄id与connection_id分离。关闭事件只产生一次；统计锁竞争时指标允许null，不能为日志等待业务锁。VPN事件不意味着系统VPN已接管，必须依据实际事务结果。

## Unix数据报线协议

明文模式：一条UTF-8 JSON Record对应一条数据报，上限4096字节。加密模式：二进制记录，最长4197字节，布局：

| 偏移 | 长度 | 内容 |
|---|---:|---|
| 0 | 8 | ASCII NCLOG001 |
| 8 | 65 | HPKE P-256 encapsulated key，X9.63非压缩 |
| 73 | 12 | AES-GCM nonce：随机4字节前缀 + 8字节大端单调记录计数 |
| 85 | N | 加密后的JSON，N最多4096 |
| 85+N | 16 | GCM认证标签 |

固定HPKE Base套件DHKEM(P-256,HKDF-SHA256) / HKDF-SHA256 / AES-256-GCM（ID 0x0010/0x0001/0x0002）。info精确UTF-8字节为`Nodus Core log transport v1`；exporter_context为`Nodus Core log AES-256-GCM record key v1`；export长度32字节。**这是HPKE exporter派生AES-GCM记录密钥的版本化协议，不能直接以HPKE open解密记录正文**。接收方setup_receiver后export相同32字节，用AES-256-GCM、头部前85字节AAD及给定nonce解密。对端可按enc缓存派生记录密钥；丢包/乱序不依赖隐式HPKE消息序号。

每个已校验加密配置候选生成全新临时HPKE上下文；只有成功提交候选才切换输出。记录nonce计数由共享加密上下文原子分配；即使多实例共享候选，也不重复nonce。耗尽拒绝当前事件。新公钥提交后旧私钥不能解密新事件。运行时等级调整不更换密钥，解除/重新注册消费者不回放历史。

接收方必须校验长度、版本、曲线点及认证标签；按会话enc/nonce去重以拒绝重放，并限制派生上下文缓存。公钥加密提供机密性和完整性，**不认证发送进程身份**；可信本地通道与调用者认证由宿主平台负责。Core不持有接收方私钥，不声称进程内存、崩溃转储或消费者明文绝对不可被第三方访问。

## 验证与边界

`tests/test_logging.rs`覆盖精确等级、无消费者无历史、背压/锁竞争、原字段、失败保留、真实Unix输出、加解密/错误私钥/篡改/丢包/乱序、nonce耗尽、密钥轮换、默认配置及真实TCP连接关联。结果见同名报告。GUI/Swift解密互通、独立密码学审计、性能基准、所有P1/P2模块事件及真实系统VPN验收仍未完成。hpke依赖上游声明未审计，不宣称已经第三方安全审计。

兼容性变更：2.16明确移除底层diagnostics历史snapshot。2403错误代码、分类、原因与返回Err保持不变，仅将副作用说明从固定诊断环改为实时输出；历史契约夹具保留原文，测试只对这一项已授权变更作精确迁移比较。示例配置中的BASE64_PUBLIC_KEY是待替换占位符，启用加密时不能直接作为有效配置使用；默认模板关闭加密可直接校验。
