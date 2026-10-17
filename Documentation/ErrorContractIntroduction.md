# 错误代码查询表

作者：可莉唯一的狗。

本文是内核公开错误协议的技术契约。权威定义为 `src/error.rs` 的 `define_codes!`，生成 `ErrorCode`、固定整数值、参数 Schema 与下表；`error-codes.json` 是同一契约的机器可读快照，不是另一套独立定义。每个变体有中文说明性注释。

## 协议、用途及兼容性

公开 code 为固定 u16 四位整数，前两位类别、后两位具体错误。类别登记：10 INPUT、20 CONFIG、40 DEBUG、50 DNS、60 IO、70 TLS、80 RESOURCE、90 LIFECYCLE、11 POLICY、12 ABI、13 SCRIPT、14 PROTOCOL、24 GEOIP、99 INTERNAL。尾号 00 本版不分配；类别为当前实际归类，不虚构 AUTH/IPC。兼容接口的旧代码是历史协议，不能按新分段解释。

权威定义在 src/error.rs；强类型枚举显式登记，不根据排列或运行时算术推导。内部 Error 保留旧变体，并为已核实的 YAML、Debug、预算及 I/O 类型增加独立失败及必要上下文。类别前缀、唯一性、源码注释、必要元数据和文档自动核验；技术检查不代替 L3。

结构化示例：`{"code":2002,"params":{"field":"debug"}}`。code 与 params 必填，错误对象限 512 字节；按代码检查参数组合、类型、数值范围，拒绝未知/重复字段。field 限 1–32 字节 ASCII 标识；actual/limit/expected 为固定整数，不提供密码、Token、路径、配置正文、对象地址或任意 JSON 参数。Debug 缺失生命周期明确发送 actual:null；trace_id 本版未实现，不虚构关联记录。

params 的真实 Schema 逐条列于下表。不需要上下文的错误使用空对象，有必要上下文的错误不能空对象。固定上下文在栈内，无新增第三方依赖或堆分配；ErrorInfo 不再只有 2 字节，增加的有界空间用于保留真实失败信息。纯匹配成功路径不创建错误对象。

## 协议迁移与兼容策略

用户已授权旧接口改为新方案。所有 command、command_v2、last_error、TUN 失败及 Job/Capture 统一新代码和上下文，不保留旧错误映射，不新增 v3。命令新增 error_info；文本仅为诊断。TUN 成功 0、队列空 11 为非错误状态；全部失败为四位代码。ABI version 从 1 升至 2，Swift 门面验证版本并保留 NativeKernelError。

Rust 新增 Error 变体、ErrorInfo 上下文及 json_bytes 的 Option 返回为源码变更。调用方须更新穷尽匹配及整数预期，整体固定新版 crate/XCFramework/Swift Package；不能用旧表解释新编号。回滚同时还原前一二进制和客户端。历史冻结文件保持原内容核实类别未消失，新方案另有冻结基线，不能篡改历史掩盖变化。

REJECT 是策略，连接 ConnectionRefused 是 I/O 执行失败；旧版为 1007，本版全部入口为 6003。YAML 语法、未知/重复字段和来源版本由实际类型/事件校验产生，不解析错误文本。预算失败保留申请量和上限，actual 不一定大于 limit，因为并发已占用预算也能拒绝。Debug TTL 错误不替换旧状态，关闭状态不重新开启。Debug 专用本地 IPC 的系统对端认证和操作授权由内核执行，远程规则提供者不能获得该权限；具体边界见 Debug本地IPC安全契约.md。

规则优先级、裁剪、DNS 决策、资源准入及状态提交顺序不因分类变化更改。源码位置为真实函数；当前能力及未完成事项见 [待办清单](TODO.md)，各项测试的实际证据见 `tests/` 中的同名报告。

## 自动维护

执行 `cargo run --example error_contract -- --check` 与 `cargo test --test test_error_contract`。新增代码、Schema、说明、触发条件、后果或源码未同步会失败；兼容基线不可通过更新文件来掩盖已发布语义变更。数量一致及自动生成不证明业务正确。

## 非错误状态

AbiStatus::Success=0 为成功，AbiStatus::Empty=11 为接收队列空，不登记为错误，不用于执行失败。

## 已发布旧编号永久保留

1001 保持原通用输入含义；1002–1020 作为 v2.3.x 历史编号退役，永久禁止复用。新方案的能力不支持为 1021，不能用旧 DNS 超时号 1002 表达新含义。当前解析拒绝退役号，不把旧诊断误读为另一类错误。退休号在 src/error.rs 集中登记并经编译期及测试检查，完整旧语义保留在未修改的 v1 历史契约中。新分段规则只适用于当前有效代码，历史混合编号仅作为归档查询。

## GeoIP 可选功能状态

GeoIP 使用用户明确指定的 2401–2409 错误，前两位 24 为 GEOIP 类别。状态代码 2410（WARN + Disabled）、2411（INFO + Ready）、2412（合法 no-match，非错误）也在中央契约登记，不能把它们视为执行 Err。缺少 geoip_url 时返回 success:true、geoip_status 和 warn，不初始化数据库、不下载、不查询 DNS。明确配置但执行失败返回 success:false、level:error 和 error_info，禁止静默降级。所有参数为空，不公开 URL、IP 或凭据。完整性验证指响应分帧及 MMDB 结构验证，不声称实施外部签名或预期校验和验证。

## DNS 驱动准入故障的作用域

`src/dns/runtime.rs` 的准入失败通过 watch 版本作为事件传播，不作为永久健康标记。后台任务数耗尽返回 `8001`，任务字节预算不足返回 `8002` 并携带实际申请量与预算上限。受同一解析器新故障影响的在途上游查询保守失败；之后开始的查询重新接受原有资源、会话身份与代次验证，不需要重建解析器。事件不会被新查询清空，因此不会吞掉并发旧查询的错误。同一次轮询中产生的准入失败也不得返回成功。

资源未释放时再次准入仍应失败；这不是取消限制或无界重试。会话权限已经撤销时继续返回 `9001`；锁中毒等持续故障仍按实际失败传播，不能借恢复逻辑复活旧对象。缓存与 Fake-IP 的独立成功路径不再被历史上游驱动故障阻断。运行约定见 [DNS 技术说明](DNS.md)；实际验证证据见 `tests/` 中对应的 DNS 测试报告。

## 主配置入口的复用映射

`src/configuration.rs Document::from_yaml/validate` 和 `src/configuration/rules.rs validate` 复用既有公开代码，不新增编号、不改变已发布条目的定义。实际路径：

| 代码 | 新入口真实触发条件 | 当前处理后果 | 源码位置 |
| --- | --- | --- | --- |
| 1001 | 端口冲突/值域、节点引用、规则语法、final 位置、DNS 地址或时间格式违反约束 | 拒绝整份待校验配置；不修改当前网络版本 | `src/configuration.rs validate/validate_dns/daily_time`；`src/configuration/rules.rs validate` |
| 2001 | 主配置 YAML 事件语法非法 | 拒绝输入，不执行网络操作 | `src/routing/yaml.rs Decoder::next`，由 `Document::from_yaml` 复用 |
| 2002 | 主配置或嵌套字段未知，包括被移除的旧字段与 Debug 配置入口 | 拒绝输入，只返回有界安全字段名 | `src/configuration/models.rs` 的严格模型及 `src/routing/yaml.rs unknown_field` |
| 2003 | 主配置任一映射重复字段 | 拒绝整个文档，不允许后项覆盖 | `src/routing/yaml.rs Mapping::next_key_seed` |
| 1021 | 非空代理提供者、活动插件提供者、非当前 VLESS 组合，或启用未实现 system/gvisor 栈 | 拒绝相应待校验配置，不虚构能力或替换栈 | `src/configuration.rs validate/validate_proxy` |
| 8001 | 主配置超过输入/解析结构数量/深度/标量长度门禁 | 解析拒绝，已有预算租约释放 | `Document::from_yaml`、`src/routing/yaml.rs` |
| 8002 | 主配置解析驻留预算准入失败 | 不开始解析，不产生租约或网络副作用 | `src/configuration.rs Document::from_yaml` → `src/budget.rs ByteBudget::reserve` |
| 2401/2402 | GeoIP 启用但 URL 缺失/非法或协议非 HTTPS | 校验失败，不下载、不回退内置来源 | `src/configuration.rs validate` → `src/geoip.rs Config::validate` |

`configuration_validate` C ABI 命令返回相同整数 `error_info`。校验成功不是 Running、节点在线、数据库 Ready 或新规则已经生效的证明。错误对象不包含配置原文、URL 或凭据；详细业务约束见 [主配置说明](主配置.md)。

## 规则提供者入口的复用映射

`src/providers/document.rs parse` 复用严格 YAML 2001/2002/2003 与有界解析 8001/8002，结构失败拒绝整批，合法结构内的自带策略/非法条目按规则规范单行忽略；这不是将 YAML 错误变为成功。`src/providers/cache.rs open/verify/stage` 的来源/完整性失败为 1001、格式版本不支持 1021、真实文件锁冲突 9001、系统 I/O 沿用 6001 等既有映射；不覆盖旧文件或新建不可信快照。`src/providers.rs` 的关闭/取消、版本竞争、来源预算分别复用 9001/9002、9001、8001/8002。下载超时经 `src/subscription.rs fetch` 返回 6002；后台错误只读保存 ErrorInfo，不公开 URL/正文。目录已交换后的 fsync 失败显示已提交版本与 directory_synced=false，不伪称回滚。所有 code 的既有含义、参数 Schema 和冻结基线保持不变。

同步 C ABI 的配置提交/刷新调用 `src/ffi.rs run_network`，处于 Tokio 线程时在启动任务前返回 9001；Rust 异步调用方使用 Manager 的 async API，或通过不带 Tokio 上下文的独立系统线程调用 C ABI。不能在已发生副作用后才用运行时嵌套 panic 表达拒绝。

## 主配置 DNS/GeoIP 编排复用映射（2026-10-17）

src/dns/main.rs 的端点、自举和专用出站复用既有 DNS 5001/5002/5003、网络 6001/6002 等分类；资源事件保留 8001/8002，关闭/代次失败 9001/9002，不转换 DIRECT。操作系统 DNS 不提供 DNSSEC 证明，强制验证时返回 5003。src/providers.rs 的 GeoIP 下载沿用 2403/2404，MMDB 格式/完整性/预算/初始化/查询沿用 2405–2409；缓存二进制摘要错误 2406、大小超限 2407、文件 I/O 保留原系统类别。全部参数仍为既有受约束 Schema，不新增编号。Disabled/Ready/no-match 分别为 2410/2411/2412 状态，不混为执行 Err。具体后果见 [编排契约](DNSGeoIP编排.md)。
