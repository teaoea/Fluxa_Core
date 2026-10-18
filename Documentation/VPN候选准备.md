# VPN 前置检查：隔离候选资产准备

作者：可莉唯一的狗。新增开发接口版本：2.6.0-dev。

## 真实范围

`vpn::Preparation` 是前置检查的资产准备步骤，复用现有主配置解析、DNS、GeoIP、提供者下载及规则编译。候选拥有独立管理器，不向现有管理器发布规则，不退休现有解析器；不创建/改写正式规则缓存、尝试时间文件或 GeoIP 文件，不启动自动更新计划。

本阶段没有权限、确认、Commit 或 Verify 接口；`activation_permitted` 恒为 false。`candidate-ready` 只证明本次候选资产编译完毕，**不证明必要代理链路和 DNS 上游可用，不代表无节点已获确认，更不代表 VPN Running**。未完成的检查由下一步骤补齐，不能用“节点存在”“解析器初始化”或“端口可连接”代替。

主配置 `tun.enable: true` 只在内核选择的隔离候选路径允许准备；现有 `configuration_apply` 仍拒绝未接入的活动 TUN。YAML 不能选择准备用途或声明已授权；系统代理、局域网共享、MITM、复写和文件日志等尚未接入活动能力继续整体拒绝，不部分应用。

## API、上限与关闭

- Rust：`Preparation::begin(yaml)` 异步启动，`status()` 查询只读摘要，`close().await` 撤销并等待真实资源关闭。
- C ABI：现有严格命令入口新增 `vpn_preparation_begin`（yaml）、`vpn_preparation_status`、`vpn_preparation_close`；能力位 29 仅表示候选资产准备。
- Swift：`NativeVPNPreparationClient(yaml:)`、`status()`、`close()`；仅传递意图和显示状态，不处理规则或管理内核资源。
- 全局最多 2 个准备任务；每个 FFI 引擎只保存 1 个候选。输入最多 4 MiB，输入副本计入独立 8 MiB 全局预算；解析、索引、DNS、GeoIP 和下载仍保留现有各自预算，不提高规则上限。
- 准备总期限 120 秒；到期返回独立四位错误 `9003`，取消并关闭候选资源。下载/DNS 专用错误继续原样传播，不改为普通未命中或 DIRECT。
- Closing 不等于 Closed。关闭取消正在等待的下载；阻塞解析/编译持有操作登记，关闭等待其完成。并发关闭等待同一结果；仍持有旧对象时归还业务槽，实际保留的只读内存租约继续计费。
- 原始准备故障和清理故障分别记录为 `error_info` 与 `cleanup_error_info`；清理失败不能冒充资源已释放。
- 宿主销毁句柄也请求候选关闭；短关闭命令不受普通请求配额阻塞。已销毁引擎拒绝迟到的新候选登记。

## 示例与边界

执行 `cargo run --offline --example vpn_candidate` 只编译本地候选并输出摘要，始终不授予接管权限；示例不是系统 VPN 启动命令。

FFI 示例请求：

```json
{"operation":"vpn_preparation_begin","yaml":"tun: {enable: true, name: fluxa0}\nrules: ['final,direct']"}
```

轮询 `vpn_preparation_status` 观察 preparing/candidate-ready/failed；使用 `vpn_preparation_close` 取消并等待完成。重复 begin 拒绝；关闭后旧候选状态查询返回 9001，需要提交新意图。配置原文和节点凭据不会出现在状态摘要中。

GeoIP 显式启用时必须真实加载成功才发布候选；未配置沿用 2401/WARN 禁用语义，不创建默认数据库或日志设施。DNS/Fake-IP 准备受现有资源租约约束；冲突时明确失败，不撤销旧会话让位。

下一步骤仍需必要链路及 DNS 真实探测、系统权限/扩展身份、可信无节点授权、路由保护、四阶段启动及回滚。此次没有修改主 YAML 字段，也没有改变规则优先级。

## 2.6.1-dev 小栈修订

真实 Swift 的 256 KiB 线程揭示通用 ffi::command 调试栈帧约 510 KiB，普通线程上的通过不足以覆盖该边界。将命令分发改为各分支独立栈帧，并在配置/刷新控制面进入 Rust 运行时后构造有界 future；不增加线程或改变准入、同步返回、取消与错误语义。新增真实 ABI 小栈回归；不将系统崩溃当作可捕获 Rust 错误。2.6.0-dev 首轮候选不用于 App 更新，固定依赖需等待本修订版本完成门禁和发布。
