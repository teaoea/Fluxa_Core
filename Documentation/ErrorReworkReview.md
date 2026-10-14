# v2.4.0 错误模块重构与独立语义审查

日期：2026-10-14。项目：`/Users/klee/Documents/FLuxa_Core`，分支 dev。依据用户提供的《错误代码体系设计》全部 23 章及后续整数分段、现有接口统一、注释与示例、Documentation 约定。本报告只说明本次修复范围，不认证整个内核不存在缺陷。

## 实际变化及兼容性

权威定义仍在 src/error.rs，没有 error_code.rs。28 个代码为固定 u16 整数，前两位类别、后两位具体错误；编译期检查四位非零开头及本版保留尾号 00，测试独立核对类别前缀。没有按枚举顺序或运行时数字运算生成编号。类别 10 INPUT、20 CONFIG、40 DEBUG、50 DNS、60 IO、70 TLS、80 RESOURCE、90 LIFECYCLE、11 POLICY、12 ABI、13 SCRIPT、14 PROTOCOL、99 INTERNAL；30 等未实现独立分类的号段不虚构登记。

用户明确要求旧接口改为新方案。本版没有旧错误映射或 command_v3。现有 command、command_v2、线程本地查询、TUN、Job/Capture 使用同一整数及上下文体系。TUN 和标量失败不再使用原 1–10 错误状态，成功 0 与队列空 11 为非错误状态。ABI 从 1 升至 2；Swift 门面验证版本 2，并将机器错误保存在 NativeKernelError 及 Job/Capture 的 errorInfo 中。顶层失败不为自然语言额外分配字符串，文本不用于判断类别。

Rust 新增 Error 变体，ErrorInfo 增加固定上下文，json_bytes 对需要上下文的代码返回 None。对 Error 作穷尽匹配及按旧整数判断的调用方须迁移。这是用户授权的协议/源码变化，不能声称旧错误协议兼容。客户端和二进制须整体更新，回滚同时固定上一版本客户端和二进制。配置、规则缓存格式、路由优先级、策略及预算数值不修改。

## 上下文与资源

公开 Schema 定义在 error.rs，生成 Documentation/error-codes.json 和查询表。field 限 1–32 字节 ASCII 标识，不公开字段值；actual/limit/expected 使用固定宽度整数。代码与参数组合作为整体校验：必填值缺失、未知或重复键、错误类型、越界值和数组替代对象均拒绝；只有 Debug 缺失生命周期允许 actual:null，仍要求显式提供该字段。没有任意 JSON、路径、凭据、指针或虚构 trace_id。

Error/ErrorInfo 保持 Copy、无上下文堆分配，固定大小有上限；不再声称完整错误对象只有 2 字节。直接整数查询读取线程本地值，无分配、不加锁。结构化查询使用固定 512 字节栈缓冲区，不占响应句柄；容量不足不写入。错误编码失败返回固定 1202 失败对象、保留原失败诊断，不返回零伪装无错误。原诊断环仍为固定 64 条，错误不会持有快照。

ByteBudget 仍先原子准入再创建租约，失败记录申请量和预算上限。actual 不一定大于 limit，因为并发占用也会使申请失败；不能把限额解释为进程总 RSS。字段名不符合安全限制时返回 field:null，未知或重复字段类别保持不变，不把不受限的键直接公开。

## 独立业务核验

| 业务要求与真实源码 | 实际结果与独立预期 | 证据 |
| --- | --- | --- |
| 分类分段，error.rs::define_codes | 同一前缀仅属于登记类别；错误值四位，原混合编号已迁移 | segmented_codes_have_one_category_per_prefix；完整机器契约与中文注释检查 |
| YAML 拒绝非法输入，routing/yaml.rs | Parser 语法错误 2001；unknown_field 回调 2002；重复映射键 2003，不按文本猜类型 | invalid_config_keeps_snapshot_and_machine_failure；debug_closed_and_yaml_syntax_return_real_failures |
| 来源格式版本，runtime.rs::GenerationInput::from_yaml | 实际版本非 1 返回 2004、actual 与 expected；数量限制不混为版本失败 | 严格配置测试，旧有效 Arc 保留 |
| Debug TTL 与状态，debug.rs::Session | 缺失、0、超过 3600 返回 4001，整次失败不替换旧方案；已关闭 API 返回 4002 | TTL 矩阵、旧版本保持、真实 close 后 clear/status/replace |
| 资源准入，budget.rs::ByteBudget::reserve | 返回 8002 及申请量/限额，原预算与 Drop 规则保持；其它数量/长度失败保留 8001 | 全局脚本预算、后台 worker 配额、密集规则预算及恢复测试 |
| 执行错误不等于策略，error.rs::classify_io、outbound | ConnectionRefused 6003、规则 Reject 1101、XUDP 约束 1401；均按原流程失败或拒绝 | 真实关闭 TCP 端口、独立非法 XUDP 帧、规则及出站回归 |
| 标量与查询原因一致，ffi.rs::start/udp_session/tun_receive | 锁/处理失败不再被错误归为句柄或资源失败；返回实际四位代码和受限上下文 | 真实 Swift 统一命令、查询、TUN 与配额检查 |
| 栈及任务生命周期，ffi.rs::run_network | 大型 DNS/路由 future 在既有 Rust 工作线程轮询，命令配额仍限制为 8；同步等结果，退出守卫中止任务 | macOS 崩溃报告与修复后原出站 FFI 全量复验 |
| 错误对象完整性，error.rs::ErrorInfo | 不能用无参数静态 JSON 构造有必填上下文的错误；Serde 失败仍是失败 | typed_context_is_required_bounded_and_specific_to_code；serialization_failure_remains_a_failure |

初次 Swift 出站复验在宿主约 544 KiB 工作线程栈上触及 Stack Guard，调用栈包含 Hickory DNS 和 openRouted 的取消测试。不是推测或通过静态检查宣称修复：实际加入有界运行时轮询与所有权取消守卫后，用原完整出站测试及全部 FFI 重新运行通过。没有通过扩大宿主栈、删除测试或降低取消断言规避问题。

## 输入信任边界及未完成事项

| 输入 | 必查内容 | 本轮状态 |
| --- | --- | --- |
| YAML/JSON | 字段、类型、值域、未知/重复字段、嵌套深度 | 已执行相关严格解析及参数负向测试；配置 JSON 的全部细粒度原因未逐一新增类别 |
| 远程订阅 | 真实性、规则权限、大小、非法策略 | 原 TLS、来源配置及体积/策略拒绝回归；没有把 HTTPS 等同于来源签名或宿主授权 |
| IPC/API | 身份、授权、参数、状态、请求大小 | 现有 API/FFI 边界测试；没有实现独立 IPC 鉴权服务，不能宣称权限验收完成 |
| Debug | 授权、TTL、数量、优先级、隔离 | TTL/数量/关闭/失败保留与 API-only 回归；宿主鉴权未实现及正式传输先行的完整执行问题未解决 |
| DNS/网络 | 长度、编码、边界、畸形数据 | DNS/TUN/协议回归和真实 Swift 数据面通过；不等同所有公网或系统 VPN |
| 文件/缓存 | 版本、偏移、长度、校验、完整性 | 原二进制故障注入、校验重算、截断及加载拒绝测试通过；校验不等于认证 |
| 恢复 | 损坏、版本、非法历史状态 | 失败保留、恢复、缓存回归通过；真实断电及所有历史格式未测 |
| FFI/系统 | 指针、长度、生命周期、状态 | 空指针/长度/句柄/配额/真实 Swift 通过；非空指针实际可读性仍是调用者契约，不能宣称能隔离恶意同进程内存访问 |

已知 H1 Fake-IP 历史池生命周期问题没有在本轮修改；Debug 最高覆盖在完整出站执行顺序的既有问题仍待单独修复。宿主认证/授权、全部平台、最低系统、系统 VPN、全部分配故障、断电耐久性和故障风暴性能尚未验证。泛化 INPUT/LIFECYCLE/IO 类别保留归类边界，不声称它们能定位每个具体字段、版本竞争或文件路径；必要细化仍需逐路径核实。

## L1–L4 验收证据

L1：cargo fmt --check、cargo check --offline --all-targets、cargo clippy --offline --all-targets -- -D warnings 实际执行通过。修改的 Swift 源码用 SwiftFormat 格式化并在真实 FFI 中编译。

L2：cargo test --offline --no-fail-fast，217 通过、0 失败、1 忽略（独立服务端测试由互通脚本另行执行）；错误契约 17 项；编码失败不冒充无错误另有边界测试。全部 17 组真实 Swift FFI 最终通过，包含 1000 次句柄创建释放、DNS/路由/出站/后台任务/缓存/订阅/GeoIP。查询表 --check 通过；实际删除 1001 一条记录后检查失败，恢复后通过。历史 fixture 未改，新方案完整契约另有冻结基线。

L3：上表以用户业务要求给出独立预期及实跑结果，本次已验证分类与参数、拒绝不变、原子失败保留、预算不绕过及具体调用边界。已有 Debug 完整执行问题和宿主鉴权未完成，不能把整个规范最终验收宣布通过。

L4：证据为代码、Tests/error_contract.rs、Swift FFI、生成查询表与 Schema、冻结记录、示例、实际日志。日志：/private/tmp/fluxa-rework-final-tests.log、fluxa-rework-final-clippy.log、fluxa-rework-all-ffi-final.log、fluxa-rework-contract-negative.log、fluxa-rework-protocol.log。发布版本与最终来源/哈希另记发布记录，不能把准备版本或提交当作已上传。

## 文档一致性

公开 28 项，登记 28 项，重复/遗漏/未定义为 0；缺说明、触发条件、后果、源码或参数 Schema 为 0。8 个参数条目生成自权威定义。该统计不代替语义正确性证据。

独立官方对端互通已实际执行：37 轮 Rust 与 37 轮真实 Swift 协议测试通过；不将回环互通视为真实用户节点或系统 VPN 验收。

发布前补充核验：发现 macOS 大小写兼容掩盖了两个未显式登记的 Tests 路径，已补齐 Cargo 的明确测试目标；未知字段标识被安全过滤时不再退回通用输入错误，返回原类别与 field:null。未上传候选先取消，后续发布必须固定新提交；历史 v1 冻结记录不修改，新方案候选基线在首次正式发布前更新并复验。
