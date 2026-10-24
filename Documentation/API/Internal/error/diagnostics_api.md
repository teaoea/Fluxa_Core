# 底层实时诊断 API

API分类：内部自用，来源src/error/diagnostics.rs。set_output注册消费者持有Receiver的SyncSender<Diagnostic>，None解除。record/record_code/record_io/observe/observe_join生成当前固定分类诊断并try_send；不改变被观察操作的真实返回值。Diagnostic仅含code/site/io_kind/raw_os_error，等级由ErrorCode集中定义。无snapshot或历史查询，无Core缓冲、磁盘或远程收集。队列满、失联或try_lock失败直接丢弃，禁止阻塞业务。详细所有权与版本迁移见[日志内部API](../logging_api.md)。测试见tests/test_error_contract.rs、tests/test_tun_udp.rs及同名报告。
