# 真实启动前置检查

## 职责（干嘛的）

对应源码：`src/vpn/preflight.rs`。通过 `vpn::transaction::Controller::begin_preflight` 接受调用方明确提供的 YAML、只读资源和 HTTPS 探测地址，在独立 Manager 中验证实际网络条件。它是四阶段事务的真实检查驱动，不提交活动配置、不执行系统接管。`preflight_status` 返回同一事务的只读 Report，不返回可提交的授权或候选令牌。

## 目标（想达成什么）

严格配置及全部来源先完成校验和编译；显式启用 GeoIP 必须读取有效数据库；每个必要 DNS 上游均需实测；探测经过同一配置快照的真实路由、代理协议、目标 TLS 和 HTTP，不能将 DIRECT 成功当成其他代理已验证。任何失败保留原错误并清理候选资源。网络成功与完整启动资格严格区分。

## 禁止行为（不允许做什么）

不得绕过 DIRECT／REJECT、替换选定代理、使用系统 DNS 作为已启用上游的失败回退、关闭 TLS 校验、执行复写伪造探测响应，或跟随重定向扩展探测范围。不得下载、创建、修改或修复配置／资源文件。不得接受 YAML、普通 IPC 或外部 Report 作为授权、权限或系统检查成功证明。缺少平台能力与预算证据时禁止进入 Prepare、Commit、Verify 或 Running。

## 输入、边界与预算

YAML 沿用配置模块最大正文和严格解析。Resources 由宿主明确授予只读文件引用，沿用现有文件类型、长度、变更和授权检查。探测地址必须是1–32个不同的 HTTPS URL，每个最多2048字节，不含用户信息、查询参数、片段或零端口；不内置第三方网址。探测使用 GET，成功要求目标 TLS 校验和 HTTP 2xx，响应正文最多4096字节，不跟随重定向。只证明所给目标在本次环境的可用性，不证明所有互联网目标或 UDP 路径。

入口在校验及复制输入前预留8 MiB全局输入预算中的 YAML、地址、资源引用真实容量及固定控制开销；事务槽全局只允许一个启动／恢复事务，旧运行与其他模块仍保留各自实际预算。配置解析、提供者文件、规则快照、DNS、GeoIP、连接和正文继续使用既有预算。这个组合不是进程 RSS、OS 文件描述符余量或完整启动峰值证明，因此 ResourceBudget 报 Partial，不能伪称全部满足。

## 流程与调用关系

1. begin 校验探测计划、期限和资源引用，经 Controller 的同一全局槽准入，绑定内核分配的事务标识；公开入口仅使用系统 TLS 根。
2. Driver 严格解析 Document，并执行 VpnCandidate 支持检查。显式 GeoIP 缺少授权数据库直接报 GeoipInitializationFailed。其余资源由 Manager::prepare_vpn_candidate 和现有 Runtime 只读加载，编译来源完整快照；失败不发布部分配置。
3. GeoIP 成功加载后才报告 Ready；未启用产生现有 WARN/Disabled。初始化未完成时 geoip_status 为 None，不把失败写成 Disabled。
4. Router::preflight_outbounds 从编译后的有效规则、运行模式、默认出站以及主配置／插件中启用的原生代理复写引用取保守必要代理集合，不读取原始规则字符串推断优先级。Direct 模式不要求未使用代理；Global 使用默认代理；规则模式要求仍保留的 Proxy 规则及兜底代理；活动原生代理复写另计必要集合。编译器未证明不可达的保留规则仍要求覆盖，最多32个必要代理，超限拒绝。
5. DNS 启用时，使用第一个非 IP 探测主机名逐个实测全部配置上游，遵循 IP 策略的 A／AAAA 顺序。不走主配置 hosts、Fake-IP、前台查询缓存；Hickory 缓存与 hosts 已禁用。localhost 保留名不能充当真实 DNS 上游探测，须换成可由上游解析的主机名；仅 IP 地址无法完成启用 DNS 的探测。system 代表真实 OS 解析能力，不代表 DNSSEC 证明，沿用现有验证失败语义。
6. HttpsClient::probe 复用受管理连接和 HTTPS 分帧。ManagedStream 保留实际连接 Decision，只有代理握手、目标 TLS 和 HTTP 均成功才将该代理计入已验证集合。全部探测必须成功，且必要集合必须被覆盖；缺覆盖返回 InvalidInput，不改路由或偷偷增加请求。仅由复写引用、却不能被任何探测的正式路由覆盖的代理同样拒绝，不能用合成复写结果或绕过正式DIRECT规则替代验证。
7. 记录 network_checks_passed；当前权限、系统能力和防回环没有真实平台实现，报告 Unavailable；整体预算 Partial；无节点报告 AuthorizationRequired。最终返回 Unsupported，仍停在 Preflight 故障并清理，而不是推进系统接管。

## 生命周期、取消与错误

Driver 是密封 Steps 的内核实现。外部不能实现步骤或提交“检查通过”标志。取消撤销当前网络检查，Manager::stop 主动关闭并等待内部工作；事务驱动等待步骤确定结束再回滚。期限触发的 Cancelled 转换为监督层 NetworkTimeout，用户取消保留 Cancelled，真正的底层错误不被超时覆盖。

配置未知字段、缺失节点、资源错误、GeoIP 错误、DNS、TLS、REJECT 与网络故障沿用 src/error.rs，Report 的 Finding 保留实际子检查错误，事务 Status.failure 是整体终止原因；超时导致内部取消时两者可分别是 Cancelled 和 NetworkTimeout。Report 不包含原始 URL、节点名称、正文或凭据，不能用其克隆恢复活动权限。检查项使用固定10项数组；终态摘要单独持有64 KiB预算直到最后在途引用释放。全局终态只保留有界摘要；成功清理释放 Driver，回滚失败遵循原恢复契约。

## 示例、测试和未完成范围

可运行示例：`examples/vpn_preflight.rs`。对应测试：`tests/test_vpn_preflight.rs`；同名报告记录真实回环 DNS/TLS、拒绝、缺覆盖、证书与响应失败、只读规则以及取消证据。代码测试不证明真实 VLESS/REALITY 远端互通、所有地址族、最低系统或实际系统 VPN。

尚未实现：系统 VPN 权限／平台能力核验、系统路由防回环证据、完整峰值及系统 FD 余量、可信无节点授权，以及后续 Prepare／Commit／Verify。完整“真实启动前置检查”仍受这些缺口阻塞；本轮交付网络内核检查部分，不增加 C ABI 或接管能力位，不更新 App 二进制。

## 平台证据契约接入（2.11.0-dev）

网络检查后的平台拒绝路径统一经过[platform_check](platform_check.md)。Report.platform_check是新增的无敏感数据摘要。当前无生产证据源、尚未固定物理出站范围，传None明确表示未知；不能因空清单、探测成功或配置的权限字段推进Prepare。整体失败仍为Unsupported，预算Partial和可信无节点授权缺口保持。实际权限与系统能力接线仍待验收。

## 启动预算核算诊断

统一内存/FD峰值核算通过[startup_budget](startup_budget.md)接入。Report.startup_budget在检查尚未执行时为None；当前生产完整需求及可信系统余量未知，生成IncompletePlan而不是用零冒充已检查。ResourceBudget继续Partial，activation_permitted不变；完整预算任务仍待计划归集、平台观测和真实预留。

## DNS日志观察（2.16.1-dev）

Driver独立保留Controller当前Journal，用于候选DNS引导和逐上游实测。候选加密/等级不发布到该Journal；source=preflight保留必须返回真实地址的原检查语义，不因日志成功授予接管。 契约见`Documentation/API/Internal/outbound/configuration_api.md`。
