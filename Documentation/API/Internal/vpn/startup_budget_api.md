# 启动预算内部校验 API

- API分类：内部自用；来源 `src/vpn/startup_budget.rs`。
- 入口：`Request::new`、`Request::assess`；调用对象为 Core preflight::Driver；契约版本 2.15.0-dev。
- `Plan`/`Request`/`VerifiedHeadroom` 仅 vpn 模块内部可见；余量字段私有，无生产构造器、Deserialize或外部提交入口。公开只读诊断见 [External](../../External/vpn/startup_budget_api.md)。

## 输入输出及信任

new接收非零事务ID、Core完整计划和单调时间；0返回既有InvalidInput。计划的retained/candidate/buffers/temporary均为Option<Usage>，None不得视为零。safety_margin为附加保留量，process_limit为总峰值限额。Usage内存单位字节，FD单位个，均u64，所有加法检查溢出。

assess消费本次请求，接收内部可信观测、当前时间及Core取消状态。重复消费返回既有InvalidState；校验失败返回budget_satisfied=false及结构化Reason，不另分配错误码。观测应来自真实可信系统边界，available为扣除现有占用后的可追加余量；只能验证同一事务、请求之后且不晚于当前时刻、年龄小于5秒的观测。

## 计算与失败后果

peak = retained + candidate + buffers + temporary + safety_margin；additional = candidate + buffers + temporary + safety_margin。两个维度分别对比总进程限额与系统可追加余量；等于边界允许，任一维度超限、未知、溢出、取消、缺观测或观测无效均拒绝。旧快照仍实际存活时必须计入retained。所有分项必须互斥，不可重复计费。

示例生产调用：`Request::new(id, Plan::incomplete(), now)?.assess(None, now, cancelled)?`，结果IncompletePlan；不存在可公开调用的成功示例。测试中使用隔离计划验证375字节/38FD总峰值、275字节/28FD新增需求，仅为算术边界。

## 生命周期、副作用与未实现边界

请求单次消费；诊断不授予权限、不分配系统资源、不预留配额。预算满足不等于Prepare/Commit可用，真实预留及状态变化重检仍必须实现。生产完整计划、可信观测生产者、真实峰值测量尚未接入；不得宣称P0任务完整完成。无文件、系统设置或网络副作用。回归见 `tests/test_vpn_startup_budget.rs` 和同名报告。
