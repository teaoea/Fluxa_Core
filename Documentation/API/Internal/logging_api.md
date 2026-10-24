# 日志内部 API

API分类：内部自用。`Journal`为每个Manager持有；`emit/event`只try_lock及非阻塞发送，不执行回调、文件或网络连接。控制入口attach/apply_settings允许持锁；apply_settings只复制候选阶段生成的Cipher Arc，成功完整配置提交才采用。分模块更新不得重置动态等级或重发主配置提交事件。`Cipher::new`在严格候选校验时准备，seal对每条事件原子分配nonce并认证固定头部；接口和字节协议见[外部API](../External/logging_api.md)。

依赖调用关系：主配置提交配置日志；连接管线在路由前分配ID，所有候选共用；Connection负责真实建立/关闭；VPN事务只记录真实阶段结果；FFI关闭完成后输出CoreStopped并解除消费者。事件输出失败不改变业务返回值。禁止新增Core历史队列、read/clear API、磁盘路径、发送回调或私钥。

`error::diagnostics::set_output(Option<SyncSender<Diagnostic>>)`为进程内受信宿主的现有底层分类诊断入口，发送端try_lock/try_send，无历史snapshot。它只传递code/site/io_kind/raw_os_error既有分类字段，不含Record正文/节点名称/私钥，不是加密Record替代通道；消费者自建队列。真实工作任务panic回收仍只输出一次诊断。验证见test_error_contract和test_tun_udp。
