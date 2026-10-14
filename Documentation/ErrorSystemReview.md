# 统一整数错误体系独立语义审查报告

日期：2026-10-14。正式内核目录：`/Users/klee/Documents/FLuxa_Core`。本报告记录正式同步后的发布前审查；后续用户已授权按流程发布 v2.3.0 并更新 App 固定依赖，实际交付来源以发布记录为准。

## 审查范围与依据

覆盖 `src` 下 52 个 Rust 文件，逐项核对错误创建、传播、转换、公开边界、预算及生命周期。文件索引见 [ErrorSourceAudit.md](ErrorSourceAudit.md)。业务预期来自用户确认的错误协议、安全及兼容规范，不能由生产实现或参考实现反推。没有调用其他 AI 审查结论作为证据。

公开整数错误的唯一权威定义是 `src/error.rs`。没有 `error_code.rs`。保留原有 17 个 Error 变体，新增已授权的 ProtocolViolation；20 个公开整数代码为 1001–1020。InvalidHandle 和 SerializationFailed 是 ABI 边界代码，不冒充新的业务 Error 变体。每项有中文注释。编号不表示顺序或优先级。

## 独立语义核验

| 规范与源码证据 | 实际行为及独立预期 | 最小复现／回归证据 | 结论 |
| --- | --- | --- | --- |
| 错误是机器协议；error.rs::Error::code、ErrorInfo | Error 映射固定整数；code 不通过文本或枚举位置推断，ErrorInfo 为 2 字节 | error_contract::every_internal_error_maps_to_the_independent_protocol_identity 独立列出 18 个 Error 对应值 | 本轮映射确认 |
| REJECT 是策略；routing::Router::evaluate、outbound::execution::validate_decision | 纯规则匹配成功返回 Reject；执行拒绝 1017；远端拒绝连接为 1007 | policies_errors_and_debug_lifecycle_have_independent_expected_semantics；真实关闭 TCP 端口连接失败 | 不混同策略与执行错误 |
| DNS 上游失败不得默认直连；dns::Resolver::resolve | 超时 1002、DNSSEC 失败 1004；已有合法 NXDOMAIN 按负响应处理；不会自动选择 DIRECT | real_dns_deadline_and_closed_port_fail_without_policy_success；完整 DNS 回归 | 已运行回环期限验证；真实公网 DNSSEC 条件未测 |
| 内部锁失败不是 DNS 超时；dns/mod.rs::resolve_inner | 原统计锁失败误转 NetError::Timeout；现保留 Error::Internal，仍拒绝解析 | observation_lock_failure_is_internal_not_dns_timeout 注入统计锁中毒，实际返回 1014 | 确认缺陷已修复 |
| TUN 响应失败保留原因；ffi::fluxa_engine_tun_receive | write_all/finish 的具体错误不再经 ok() 合并；旧失败数值状态仍兼容，新查询保留实际代码 | 静态调用链、FFI 回归及响应资源耗尽测试 | 原因传播确认；所有内部发布故障尚未逐个注入 |
| XUDP 安全约束；protocol/datagram.rs::decode、Datagram::fail；outbound::Manager::read | 非零会话、非法标志、返回来源不符仍拒绝并标记连接失败；整数 1020，不再误报规则拒绝 | invalid_session_and_peer_are_not_policy_rejects 三个独立非法帧 | 约束与拒绝行为不变 |
| 配置更新原子拒绝；routing::Store::update_inner | 校验失败保留原 Arc；rename 成功后切换，目录刷盘失败仍报告 directory_synced=false，不能伪造回滚 | invalid_config_keeps_snapshot_and_machine_failure；runtime_rules 的文件提交、恢复和损坏回归 | 已验证非法输入及旧快照保留；断电耐久性未做实机故障注入 |
| Debug API-only、TTL 1–3600、最高覆盖；routing/debug.rs | 非法 TTL／超配额不替换已生效规则；已关闭会话失败；没有 Deserialize、订阅、持久化或 FFI 新入口 | failed_debug_replacement_keeps_the_authorized_active_rules；已有 debug_not_configurable 测试 | 本次保持入口与失败语义；宿主授权见下节边界 |
| 响应预算必须先于副作用；ffi::OutputReservation | 预算/名额不足不执行命令、消费 TCP 或取走 TUN 包；Drop 归还未发布资源 | admission_failure_keeps_tcp_bytes_and_returns_reservations、error_query_remains_available_when_response_registry_is_full；真实 Swift 测试 | 本轮资源入口确认 |
| 旧接口兼容；ffi::command_boundary | 指针／长度及输出预留之后，旧 command 先查句柄再解析 JSON；旧 JSON 不含新增字段 | legacy_validation_order_preserves_handle_before_json 四种组合；真实 Swift 1018／1001 校验 | 保持既有顺序；早前错误测试预期已纠正 |
| 错误不泄密；error.rs::ErrorInfo、diagnostics | params 只允许空对象；不公开原始链、凭据、路径、指针；固定 64 条诊断仅有代码、位置、IO kind 和 OS 数字 | embedded_kernel_failure_survives_io_wrapper_without_sensitive_text；严格 Schema 及容量测试 | 当前受控错误字段确认；Capture 正文是已有调试数据，非错误字段安全保证 |
| 不能丢失包裹的内核错误；Error::from(io::Error) | 按类型寻找至多 16 层 cause，保留包裹的内核分类；超深／循环受控归入原 I/O 类别 | nested_and_cyclic_io_causes_preserve_classification_and_remain_bounded | 已验证直接、嵌套、循环、TimedOut 及 OutOfMemory |
| 不新增默认 Continue/DIRECT；script、rewrite、https、outbound | 本次差异只涉及分类、结构化信息和有界诊断；规则编译、优先级、覆盖及 DNS 前后匹配算法不修改 | 修改清单与源文件差异；既有规则和脚本回归 | 本次未引入放行回退；不是整个内核无业务缺陷的声明 |

DNS wire::malformed 的编码失败不再 unwrap_or_default 为成功空报文，而是传播 DnsResponse；正常 FORMERR 行为保持。系统明确报告的 io::ErrorKind::OutOfMemory 从普通 I/O 分类修正为 ResourceLimit，仍保持失败；不把配置的 ByteBudget 当作操作系统总 RSS 上限。HTTPS/MITM 外层 timeout 从 Network 修正为 NetworkTimeout；geoip/subscription 后台 JoinError 从 Network 修正为 Internal。失败结果保持不变。服务并发配额拒绝、可选缓存失败、清理和非取消 JoinError 记录受限诊断，保留原处理后果。主动任务取消、接收者已退出、无匹配、队列空和正常流过期属于生命周期或协议状态，不把它们虚构为成功执行的错误。

## 公开协议与兼容性

- Rust：Error::public_info() 返回 `{code: ErrorCode, params: ErrorParams}`；ErrorCode 是显式 `u16`，ErrorParams 是零大小空对象。没有动态参数，无 trace_id、message；不能把未知字段、数组、null、未知整数、字符串代码或重复键当作有效对象。受控 JSON 入口解析前限 512 字节。
- C ABI：旧符号、ABI version=1、原能力位和状态 0–11 保留；新增能力位 25、新 command_v2、last_error_code 和 last_error_copy。查询使用调用线程局部状态，不创建响应句柄、不进行堆分配或自然语言格式化。成功操作清除旧状态；查询本身不清除。错误发生在线程 A 时应在线程 A 查询。
- JSON：v1 仍旧格式并去掉 Job/Capture 新字段；v2 提供整数 error_info。Job/Capture 获授权增加可选 error_info；Rust 结构体字面量需要新增字段，迁移示例见 examples/error_protocol.rs。直接序列化 Rust Job/Capture 会包含已设定的可选字段。
- ProtocolViolation 是已授权 Rust 源码兼容变更；穷尽匹配须加分支。旧 Error 变体不删除。分类修正可能改变诊断文本，文本不是稳定协议。
- 配置、二进制规则缓存、规则索引、来源与优先级不变；没有新增第三方依赖，不改变主版本或远程 App 依赖。技术文档迁移 Documentation，README/AGENTS/许可证保留必要入口。
- Tests/fixtures/error-contract-v1.json 是尚未发布首版的冻结候选，不能声称已经发布。审查中纠正了 Unsupported 的实际 cache 源码位置和 Certificate 的 REALITY 密钥材料语义；整数不变。首版发布后禁止静默修改语义或重用编号。

## 自动一致性与负向验证

权威 Rust 定义生成 Documentation/error-codes.json 与 Documentation/错误代码查询表.md。运行 example error_contract --check 校验完整内容，error_contract 测试校验唯一性、分类、Schema、必要字段与基线。CI 增加一致性命令。实际删除查询表一条记录后的 --check 返回失败，恢复后通过，证明新增或漏登不能只靠数量相同蒙混通过。

| 统计 | 数量 |
| --- | --- |
| 公开整数代码／文档登记 | 20／20 |
| 重复／遗漏／未定义 | 0／0／0 |
| 缺说明／缺触发条件／缺后果／缺源码／缺 Schema | 0／0／0／0／0 |

## L1–L4 和实际执行

L1：cargo fmt --check、cargo check --offline --all-targets、cargo clippy --offline --all-targets -- -D warnings 实际通过。

L2：最终隔离目录 cargo test --offline 全部 210 通过、0 失败、1 忽略；专项 error_contract 13 项。真实 Swift FFI 核验旧 JSON、新整数、配额耗尽、无效输入/句柄顺序及成功清除。示例 error_protocol 及契约 --check 实际执行。正式同步后从实际内核目录再执行检查，结果追加在下节。忽略项为既有真实远端服务互通测试，未伪造为通过。

L3：上表列出独立规范和实际证据。本轮已确认并修复 DNS 分类、TUN 原因丢失、XUDP 误标及外层任务/期限映射；既有处理后果保留。下列原有业务缺陷、宿主认证与未运行的平台不能标为通过。整个项目“无默认放行、无安全缺陷”无法据此认证。

L4：证据为源码、测试文件、查询表、迁移示例、实跑命令及同步哈希。日志路径见正式同步验收；文件范围见下一节。没有以编译、差分或文档一致替代业务语义正确性。

## 未解决与未验证

1. 已确认原有 H1：dns/fake_ip.rs 的 TUN_POOLS 保留历史 Namespace，重叠新区间与 16 个历史名额限制会阻止后续 TUN 建立。错误体系未修改这个生命周期设计；它仍是独立修复任务。
2. 已确认静态调用顺序风险：outbound/execution.rs::connect_with_snapshot 先调用 Router::check_transport；routing.rs::check_transport 使用正式 runtime，可能先拒绝而不进入覆盖后的 Debug 决策。违反“Debug 可以覆盖正式拦截”的完整执行预期。本轮没有擅自改变路由执行顺序，不能宣称 Debug 全链路已正确。
3. Debug 当前是可信 Rust 本地 API，认证/授权明确属于宿主；内核不存在可审查的身份验证服务。没有虚构 DEBUG_UNAUTHORIZED，也不能宣称未经授权的宿主访问已经被内核阻止。宿主真实权限集成未测。
4. InvalidInput/InvalidState/Network 保留旧通用归类，不能准确声称未知字段、特定状态或文件与网络错误的更细子类；公开 params={}，已有 ValidationDiagnostic 继续保留来源和序号。未完整保留任意上游原始字符串或整条链，这是安全和兼容边界，不声称恢复已丢失的底层原因。
5. 真实公网节点、Linux/Windows/Intel、最低 macOS、系统 VPN、全部锁/分配失败注入、断电恢复、OOM 和故障风暴的锁竞争性能均尚未验证。固定诊断避免无界存储，但不等于证明异常高频下不存在锁竞争。
6. 先前编号审查中的其余业务修复按既定顺序另行推进；这份报告不把它们计为已解决。本轮只声明错误体系审查与实际目录同步，不声明全部最终验收条件已满足。

## 修改文件范围

- `.github/workflows/release.yml`
- `AGENTS.md`
- `BinaryRuleCache.md`（迁移后根目录删除）
- `BinaryRuleCachePerformance.md`（迁移后根目录删除）
- `Cargo.toml`
- `DEPENDENCIES.md`（迁移后根目录删除）
- `DNS.md`（迁移后根目录删除）
- `DebugProxy.md`（迁移后根目录删除）
- `Documentation/BinaryRuleCache.md`
- `Documentation/BinaryRuleCachePerformance.md`
- `Documentation/DEPENDENCIES.md`
- `Documentation/DNS.md`
- `Documentation/DebugProxy.md`
- `Documentation/ErrorContractIntroduction.md`
- `Documentation/ErrorSourceAudit.md`
- `Documentation/ErrorTestFailureRepair.md`
- `Documentation/GeoIP.md`
- `Documentation/HTTPS.md`
- `Documentation/JS.md`
- `Documentation/MITM.md`
- `Documentation/MemorySafety.md`
- `Documentation/Outbound.md`
- `Documentation/Overview.md`
- `Documentation/Overview_ENGLISH.md`
- `Documentation/ProtocolBlocking.md`
- `Documentation/Protocols.md`
- `Documentation/Publishing.md`
- `Documentation/Routing.md`
- `Documentation/RuntimeRules.md`
- `Documentation/RuntimeRulesAudit.md`
- `Documentation/Rust网络代理内核统一错误代码体系设计、实现与审查.md`
- `Documentation/Subscriptions.md`
- `Documentation/TUN.md`
- `Documentation/TrafficProcessing.md`
- `Documentation/TunDNS.md`
- `Documentation/Versioning.md`
- `Documentation/error-codes.json`
- `Documentation/错误代码查询表.md`
- `GeoIP.md`（迁移后根目录删除）
- `HTTPS.md`（迁移后根目录删除）
- `JS.md`（迁移后根目录删除）
- `LICESE.md`
- `MITM.md`（迁移后根目录删除）
- `MemorySafety.md`（迁移后根目录删除）
- `Outbound.md`（迁移后根目录删除）
- `ProtocolBlocking.md`（迁移后根目录删除）
- `Protocols.md`（迁移后根目录删除）
- `Publishing.md`（迁移后根目录删除）
- `README.md`
- `README_ENGLISH.md`
- `Routing.md`（迁移后根目录删除）
- `RuntimeRules.md`（迁移后根目录删除）
- `RuntimeRulesAudit.md`（迁移后根目录删除）
- `Subscriptions.md`（迁移后根目录删除）
- `TUN.md`（迁移后根目录删除）
- `Tests/error_contract.rs`
- `Tests/ffi_error_code.swift`
- `Tests/fixtures/error-contract-v1.json`
- `TrafficProcessing.md`（迁移后根目录删除）
- `TunDNS.md`（迁移后根目录删除）
- `Versioning.md`（迁移后根目录删除）
- `check-ffi.sh`
- `examples/error_contract.rs`
- `examples/error_protocol.rs`
- `include/fluxa_engine.h`
- `src/dns/mod.rs`
- `src/dns/service.rs`
- `src/error.rs`
- `src/error/diagnostics.rs`
- `src/ffi.rs`
- `src/geoip.rs`
- `src/https.rs`
- `src/mitm/capture.rs`
- `src/mitm/mod.rs`
- `src/outbound.rs`
- `src/outbound/execution.rs`
- `src/outbound/resources.rs`
- `src/outbound/stream.rs`
- `src/protocol/datagram.rs`
- `src/routing/background.rs`
- `src/routing/store.rs`
- `src/subscription.rs`
- `src/tun/udp.rs`

## 正式同步验收记录

已正式同步到 `/Users/klee/Documents/FLuxa_Core`。同步前检查文件基线并备份，按清单复制 84 项（含原技术文档的迁移删除），没有写入 .git、其他未列出文件或 App 依赖。随后在正式目录完成最后的分类与注释检查修订。本节及源码文件哈希以正式工作树为准。

环境：macOS 27.2（26B5091g），rustc/cargo 1.98.1。正式目录执行的命令（Rust 命令均使用 `--target-dir /private/tmp/fluxa-errors-target`）：

| 实际命令 | 结果与证据 |
| --- | --- |
| cargo fmt --check | 通过；命令正常退出 |
| cargo check --offline --all-targets | 通过；/private/tmp/fluxa-errors-formal-check.log |
| cargo test --offline | 210 通过、0 失败、1 忽略；/private/tmp/fluxa-errors-formal-final-tests.log |
| cargo clippy --offline --all-targets -- -D warnings | 通过；/private/tmp/fluxa-errors-formal-final-clippy.log |
| cargo run --offline --example error_contract -- --check | 20 代码一致，全部登记，重复/遗漏/未定义/空字段 0；/private/tmp/fluxa-errors-formal-contract.log |
| cargo run --offline --example error_protocol | 整数、结构化往返、Job/Capture 迁移通过；/private/tmp/fluxa-errors-formal-example.log |
| cargo build --offline --lib，swiftc 实际链接该静态库，执行 Tests/ffi_error_code.swift | 通过；/private/tmp/fluxa-errors-formal-swift.log |
| cargo package --offline --allow-dirty | 打包及解包后的 crate 编译通过；/private/tmp/fluxa-errors-formal-package.log；只本地打包，不发布 |
| 删除查询表 1001 行，运行 error_contract --check，再恢复检查 | 删除后实际非零退出，恢复后通过；/private/tmp/fluxa-errors-final-negative-contract.log |
| 文档本地 Markdown 链接检查、git diff --check | 通过，本地失效链接 0 |

每个 ErrorCode 的中文注释检查已纳入 error_contract 一致性测试。当前 error.rs SHA-256：`43b5d7614b87d80efbec0dd975630a59d899347f9459717d254a06b6b68462cf`。

完整的实际修改范围另存 [ErrorChangedFiles.md](ErrorChangedFiles.md)，技术文档已在 Documentation，查询表的正式路径为 `/Users/klee/Documents/FLuxa_Core/Documentation/错误代码查询表.md`。

最终构建缓存异常记录：补充 OutOfMemory 预期后一次增量测试仍返回旧分类；清理该 crate 构建缓存，重新编译专项及完整回归后通过。最终验收仅采用清理后的正式目录结果。SwiftFormat 与 SwiftLint strict 最新示例检查通过，0 格式差异、0 违规。

## v2.3.1 TLS 传播补充审查

实际源码 `https.rs::tls_error` 只保留 Cancelled，包裹的 ResourceLimit／Internal 会变成 Tls，违反真实原因传播规范。已把共用 I/O 分类集中到 error.rs，并在 HTTPS 与 MITM 的 TLS 边界复用。原编号和失败结果不变；只有泛化网络 I/O 在 TLS 阶段归类为 TLS，明确内核原因不被覆盖。独立测试以预算、状态、取消、策略、能力及协议原因作预期，系统内存不足和超时亦保持真实分类。完整 Rust 回归 211 通过、0失败、1既有远端测试忽略，严格 Clippy 通过。错误代码仍20个，首版基线语义没有修改。
