# 启动预算统一核算

## 职责（干嘛的）

`startup_budget.rs` 对一份 Core 内部运行计划计算内存与文件描述符的共存峰值，以及需要追加的系统余量。输入计划分为 retained（包含仍存活旧快照的现有占用）、candidate、buffers、temporary 四个互斥部分，加上 safety_margin。各部分 None 表示未知，Some(0) 表示已确认不需要；不得重复计费。内存与 FD 两个维度独立 checked_add，溢出拒绝。

## 目标（想达成什么）

固定峰值公式为 retained + candidate + buffers + temporary + safety_margin；新增需求为 candidate + buffers + temporary + safety_margin。总峰值对比进程限额，新增需求对比已经扣除现有占用的系统可追加余量，避免把旧快照漏算或在系统空闲量中重复扣算。等于限额允许，超过一单位拒绝。

Request 为内部一次性检查对象，非零事务身份，单调时间；可信余量观测必须属于同一事务、不早于请求、不晚于当前时刻且年龄小于5秒。消费、取消、未知量、缺观测、过期及溢出均不能满足预算。Assessment 为可复制只读诊断，不是租约、授权或可重放的启动票据。即使 budget_satisfied=true，也只表示本次瞬时预算检查满足要求；后续真实资源预留、计划/系统状态变化重检与四阶段事务仍为必要条件。

## 禁止行为（不允许做什么）

不得接收配置、订阅、普通JSON或FFI中的“系统余量/预算通过”声明；VerifiedHeadroom无公开构造器且不反序列化，生产尚无采集器。不得把组件ByteBudget高水位当进程RSS或完整系统余量，不得逻辑关闭后扣掉仍存活对象的内存，不得用总内存/FD总上限冒充空闲余量。模块不修改系统设置，不获取root，不写文件，不降级DIRECT。

## 调用与当前边界

preflight::Driver 在网络检查通过后建立 incomplete 计划，经同一 assess 路径生成 Report.startup_budget。生产报告因此 reason=incomplete-plan、budget_satisfied=false、peak/additional=null，既有 ResourceBudget Finding 仍为 Partial；没有新增接管权限。网络检查提前失败则字段为null。可信OS余量采集、完整运行计划归集、资源真实预留与压力实测仍待实现，本模块不是完整启动预算任务验收。

内部入口见 [Internal API](../../Documentation/API/Internal/vpn/startup_budget_api.md)；外部诊断见 [External API](../../Documentation/API/External/vpn/startup_budget_api.md)。测试位于 `tests/test_vpn_startup_budget.rs` 和 `tests/test_vpn_preflight.rs`。示例 `examples/vpn_preflight.rs` 输出同一报告，不提供预算绕过参数。
