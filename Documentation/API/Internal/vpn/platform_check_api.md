# 平台证据校验内部 API

- API 分类：内部自用 API，不对外提供证据提交或授权能力。
- 来源源码：`src/vpn/platform_check.rs`；调用方为 `src/vpn/preflight.rs`。
- 入口与可见性：`Request::new`、`Request::unavailable` 为 `pub(super)`，仅 `vpn` 模块内可调用；`assess`、`validate` 与 `VerifiedEvidence` 私有，仅模块实现及其子模块测试可访问。
- 契约版本：Nodus-Core 2.11.0-dev，未正式发布的内部契约。

## 输入与返回

`Request::new(transaction: u64, strategy: IpStrategy, dns: bool, egress: Option<&[IpAddr]>, now: Instant) -> Result<Request>`：事务必须非零，出站地址最多32个且不能重复、未指定或多播。`None` 表示尚未解析真实必要出站集合，`Some(&[])` 表示已知为空，两者不能互换。IPv4／IPv6要求由策略推导，DNS能力按配置要求；请求生成32字节随机挑战，有效期30秒，使用单调时间。

`Request::unavailable(&mut self, now: Instant, cancelled: bool) -> Assessment`：当前生产调用的唯一检查入口，通过同一私有校验路径处理缺证据、取消、到期及已消费请求。首次调用即消费请求，包括失败；重复调用拒绝。结果是可复制的只读摘要，不是授权令牌；其外部字段见[外部诊断契约](../../External/vpn/platform_check_api.md)。

## 权限、失败与生命周期

构造失败返回既有 `InvalidInput`、`ResourceLimit`、`Unsupported` 或随机源失败的 `Internal`，不另建数字错误协议。校验细分原因使用公开诊断 `Reason`；取消仍由上层保留 `Cancelled`。请求取消、到期、消费、挑战／事务／平台／观察时刻不符，均不能通过。来源权限锁中的代次或撤销检查失败拒绝；锁异常诊断 `source-failure`。来源 Authority 只提供生命周期约束，不能代替 OS 身份认证。

私有证据校验先权限、再 IP 包／地址族／DNS能力、再受保护出站覆盖；后续未检查项不伪造通过。防回环要求非零保护范围、非零接口、同一范围及必要地址的精确覆盖；即使集合已知为空也不能缺保护范围。数值非零本身不是 OS 事实证明。

请求及证据无反序列化／复制入口，固定容量32个出站见证。无文件写入、网络探测、权限申请、系统路由变更或系统接管副作用；内部摘要不能激活 VPN。完整配置／快照／网络变化的通用证据失效仍属后续任务。

## 调用示例与验证

实际生产调用见 `src/vpn/preflight.rs`：以当前事务和配置创建请求，当前物理出站未解析传 `None`，缺真实平台生产者时调用 `unavailable`。不提供外部可运行的成功证据构造示例；实际外部查询沿用 `examples/vpn_preflight.rs`。

[test_vpn_platform_check.rs](../../../../tests/test_vpn_platform_check.rs) 用私有替身覆盖请求关联、时间边界、取消／重放、撤销／锁异常、权限／能力、精确路由覆盖与有界输入；[同名报告](../../../../tests/test_vpn_platform_check.md)记录真实执行。替身成功不等于 OS 验收。

## 未实现与未验收

尚无生产证据构造器或原生平台适配。后续受信边界必须先认证真实 OS 能力通道并验证权限、接口、接管后仍有效的路由保护事实；不得接受 JSON 成功标志或仅凭 Authority 自证可信。当前整体仍拒绝接管，不能进入 Running。真实系统权限、路由、VPN数据路径、最低系统与其他平台未验收。实现细节见[同名代码文档](../../../../src/vpn/platform_check.md)。
