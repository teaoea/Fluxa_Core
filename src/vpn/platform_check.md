# 平台证据校验

对应源码：`src/vpn/platform_check.rs`；Rust开发契约2.11.0-dev。

## 职责（干嘛的）

本模块定义平台权限、TUN/IP包能力、必要地址族与系统DNS能力、出站防回环范围的纯内核校验。输入是Core内部创建的单次Request和不可从外部构造的VerifiedEvidence，输出是无敏感数据的Assessment。当前没有生产证据生产者，真实网络Preflight只走缺证据分支，不产生平台成功或接管资格。

## 目标（想达成什么）

已实现的契约目标是：网络成功不替代平台权限；错误事务、挑战、平台、观测时间、撤销的来源、权限未决/拒绝、缺少必要能力和不完整防回环范围均不能通过。生产缺接线时明确Unavailable。真实OS权限核验、已签名能力通道身份认证和系统接管不属于本轮已经实现的能力，需由后续原生边界生成真实观察并单独验收。

## 禁止行为（不允许做什么）

禁止给VerifiedEvidence增加公开构造、Deserialize、Clone或普通JSON/FFI成功标志入口；YAML、订阅和诊断摘要都不能提供可信证据。禁止把当前物理路由存在、端口存在、签名存在或一个非零整数视为防回环成功。禁止下载、写盘、创建目录、申请系统权限、修改路由/DNS或发布Running。来源异常、未知出站集合或不匹配都必须拒绝，不回退DIRECT。

## 类型、输入与上限

Platform是当前编译目标的诊断类别，不由输入字符串选择。Request在Core内分配32字节随机挑战，关联非零事务ID、单调创建时刻及30秒期限；不进行墙钟比较。必要能力依ip-strategy决定IPv4/IPv6，DNS启用时还要求系统DNS接线能力。仅IPv4/仅IPv6不强制另一地址族，prefer策略要求两个族。

防回环目标最多32个唯一单播IP，在复制前拒绝超量、重复、未指定和组播；固定数组避免不可信数量触发动态分配。None表示尚未固定实际物理出站集合，Some空集合表示Core已确定无此类出站，二者不能互换。当前Preflight尚未具备完整物理出站计划，因此传None。

VerifiedEvidence私有且无生产构造器。其来源Authority/Identity必须仍Active；观测应属于同次随机挑战、事务和编译平台，时间位于请求创建后且不晚于校验时刻。Authority只代表Core对未来能力通道的生命周期约束，不能单独当成真实OS身份认证；后续原生生产者必须先完成OS身份与能力核验，才可创建这种值。现在只有模块子测试直接构造替身。

## 流程、取消与失败

Request第一次校验即被消费，包括缺证据、取消或失败；重试必须创建新挑战，不能再次接受同一请求。30秒边界即失效，早于请求创建的校验和超前/陈旧观测拒绝。来源校验与撤销使用同一个Authority锁；锁失败单独报告source-failure，不冒充有效权限。

Granted权限后才检查IP包、必要地址族和DNS能力；Denied与Undetermined分别返回Rejected和Unavailable，其余检查保持NotRun。实际出站范围未知时，权限和能力通过也不能通过防回环。

防回环要求非零已验证保护范围，且每个目标恰有一条同范围、有效接口索引的见证，额外、重复、遗漏或错误范围拒绝。空集合也仍需受信保护范围。范围ID和接口索引只是已认证生产者交付的引用，不能靠数字校验独立证明OS路由安全；生产者必须验证该范围在本次接管后仍保证必要出站不会进入TUN（例如受保护的提供者出站或经验证的排除计划），不得把当前路由查询结果直接包装成见证。实际条件化系统操作由后续平台接线完成。

Assessment只包含事务ID、平台、三项状态和细分reason；它可以复制、序列化，但无挑战、身份、地址、租约或激活标志，不能作为后续阶段授权。跨配置/完整快照与网络环境变化的统一失效仍属于后续TODO；本模块的请求相关校验不冒称已完成那一项。

## 调用关系、示例和验证

preflight::Driver在真实网络检查后调用Request::new与unavailable，填充Report.platform_check和既有Permission/SystemCapability/LoopProtection Finding；仍以Unsupported结束，预算Partial及无节点授权要求不变。现有`examples/vpn_preflight.rs`会输出新增诊断字段，没有跳过检查的开关。

无新错误码或ABI。非法构造输入沿用InvalidInput/ResourceLimit，不支持的平台沿用Unsupported；报告reason是诊断状态不是第二套错误协议。测试为`tests/test_vpn_platform_check.rs`及既有`tests/test_vpn_preflight.rs`，运行事实与未测边界记同名文档。平台权限、实际签名、真实路由、最低系统和跨平台执行不由纯校验测试证明。
