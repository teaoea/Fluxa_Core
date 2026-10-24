# diagnostics.rs 代码说明

职责：输出当前固定分类诊断，不保存历史。目标：在保持原业务返回值的前提下观察I/O、工作任务与清理错误。入口set_output注册消费者提供的有界发送端，append仅try_lock/try_send；锁竞争、无接收方或满队列直接丢弃。禁止添加snapshot、Core缓冲、磁盘或远程收集。既有诊断字段是错误代码、位置、I/O分类及OS错误号。所有权与API见Documentation/API/Internal/logging_api.md，测试为test_error_contract与test_tun_udp。
