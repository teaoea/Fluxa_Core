# 测试报告：test_vpn_transaction

<!-- nodus-test-report:{"command": ["/Users/klee/Documents/Nodus Core/target/debug/deps/nodus_core-8399fc01bd4f1237", "::test_vpn_transaction::", "--test-threads=1"], "exit_code": 0, "duration_seconds": 0.0842620839999988, "status": "PASS", "discovered": ["vpn::transaction::test_vpn_transaction::cancellation_during_commit_waits_for_completion_then_restores", "vpn::transaction::test_vpn_transaction::controller_drop_cancels_running_work", "vpn::transaction::test_vpn_transaction::deadline_signals_cancellation_without_detaching_work", "vpn::transaction::test_vpn_transaction::each_stage_failure_cleans_and_preserves_cause", "vpn::transaction::test_vpn_transaction::four_stages_verify_before_running_and_stop_once", "vpn::transaction::test_vpn_transaction::limits_reject_before_execution", "vpn::transaction::test_vpn_transaction::old_transaction_cannot_stop_new_start", "vpn::transaction::test_vpn_transaction::panicking_step_still_rolls_back", "vpn::transaction::test_vpn_transaction::rollback_failure_retains_slot_and_explicit_retry", "vpn::transaction::test_vpn_transaction::terminal_cleanup_releases_steps_and_failure_retains_recovery"], "results": {"vpn::transaction::test_vpn_transaction::cancellation_during_commit_waits_for_completion_then_restores": "PASS", "vpn::transaction::test_vpn_transaction::controller_drop_cancels_running_work": "PASS", "vpn::transaction::test_vpn_transaction::deadline_signals_cancellation_without_detaching_work": "PASS", "vpn::transaction::test_vpn_transaction::each_stage_failure_cleans_and_preserves_cause": "PASS", "vpn::transaction::test_vpn_transaction::four_stages_verify_before_running_and_stop_once": "PASS", "vpn::transaction::test_vpn_transaction::limits_reject_before_execution": "PASS", "vpn::transaction::test_vpn_transaction::old_transaction_cannot_stop_new_start": "PASS", "vpn::transaction::test_vpn_transaction::panicking_step_still_rolls_back": "PASS", "vpn::transaction::test_vpn_transaction::rollback_failure_retains_slot_and_explicit_retry": "PASS", "vpn::transaction::test_vpn_transaction::terminal_cleanup_releases_steps_and_failure_retains_recovery": "PASS"}, "fingerprint": "a9a7c5f1f766dccced8fab47462550d12fd300ee94da91bb7a05779ee42ec509", "time": "2026-10-23T21:28:20.957371+08:00", "attempt": 3, "suite": "test_vpn_transaction", "file": "tests/test_vpn_transaction.rs", "environment": {"os": "macOS-27.2-arm64-arm-64bit", "architecture": "arm64", "rust": "rustc 1.98.1 (48a229cea 2026-09-01)", "cargo": "cargo 1.98.1 (797e8a9bc 2026-08-05)", "commit": "8b8c30c0e31631aadda5c4cd97c17e8f8b800cc2", "dirty": true, "features": "Cargo 默认 features；未启用 --all-features", "optional_environments": {"NODUS_DEBUG_TEST_REQUIREMENT": false, "NODUS_DEBUG_TEST_SWIFT": false, "NODUS_PROTOCOL_PROFILE": false, "NODUS_PROTOCOL_FIXTURE": false}, "ci_run_id": null, "ci_run_attempt": null}} -->

## 基本信息

- 测试文件：`tests/test_vpn_transaction.rs`
- 测试对象：src/vpn/transaction.rs
- 执行机制：unit
- 测试范围：本文件登记且在当前平台实际发现的全部用例
- 实际命令：`/Users/klee/Documents/Nodus Core/target/debug/deps/nodus_core-8399fc01bd4f1237 ::test_vpn_transaction:: --test-threads=1`
- 执行时间：2026-10-23T21:28:20.957371+08:00
- 实际耗时：0.0842620839999988 秒
- 运行环境：`{"os": "macOS-27.2-arm64-arm-64bit", "architecture": "arm64", "rust": "rustc 1.98.1 (48a229cea 2026-09-01)", "cargo": "cargo 1.98.1 (797e8a9bc 2026-08-05)", "commit": "8b8c30c0e31631aadda5c4cd97c17e8f8b800cc2", "dirty": true, "features": "Cargo 默认 features；未启用 --all-features", "optional_environments": {"NODUS_DEBUG_TEST_REQUIREMENT": false, "NODUS_DEBUG_TEST_SWIFT": false, "NODUS_PROTOCOL_PROFILE": false, "NODUS_PROTOCOL_FIXTURE": false}, "ci_run_id": null, "ci_run_attempt": null}`
- 代码/测试/配置指纹：`a9a7c5f1f766dccced8fab47462550d12fd300ee94da91bb7a05779ee42ec509`
- 总体状态：**PASS**
- 原始进程退出状态：0

## 实际用例与结果

| 实际发现的用例 | 独立目标 | 状态 |
| --- | --- | --- |
| `vpn::transaction::test_vpn_transaction::cancellation_during_commit_waits_for_completion_then_restores` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `vpn::transaction::test_vpn_transaction::controller_drop_cancels_running_work` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `vpn::transaction::test_vpn_transaction::deadline_signals_cancellation_without_detaching_work` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `vpn::transaction::test_vpn_transaction::each_stage_failure_cleans_and_preserves_cause` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `vpn::transaction::test_vpn_transaction::four_stages_verify_before_running_and_stop_once` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `vpn::transaction::test_vpn_transaction::limits_reject_before_execution` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `vpn::transaction::test_vpn_transaction::old_transaction_cannot_stop_new_start` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `vpn::transaction::test_vpn_transaction::panicking_step_still_rolls_back` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `vpn::transaction::test_vpn_transaction::rollback_failure_retains_slot_and_explicit_retry` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `vpn::transaction::test_vpn_transaction::terminal_cleanup_releases_steps_and_failure_retains_recovery` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |

## 执行汇总

- 实际发现：10
- 实际执行完成：10
- 通过：10
- 失败：0
- 忽略/跳过：0
- 未取得完整执行结果：0

## 失败、异常与限制

本次未观察到断言失败不等于不存在缺陷；平台门控的未发现用例不计通过。
未覆盖范围以本文件实际断言和前置条件为界，不宣称全模块或全平台覆盖。
不自动重试；若手动重跑，旧报告及原始退出结果保存在证据目录。缺少执行结果时不推定成功。

## 独立 L1–L4 审查

- L1：本次测试构建结果见 build 日志；格式/Clippy 等需另行记录，不自动推断通过。
- L2：已登记用例按 Cargo 工件、发现列表与显式文件映射归属；实际结果见上表。
- L3：未由报告生成器验证；迁移保留断言不等于原业务预期已全面证明，需独立语义审查。
- L4：原始日志、机器结果、版本及指纹可在下述证据核对；未上传的本地证据不冒充 CI 工件。

## 证据及后续事项

- 原始证据：`/private/tmp/nodus-transaction-resource-regression/test_vpn_transaction.json`、对应 `.log`（实际存在时）。
- CI 工件标识：`本地证据目录；未宣称已上传 CI`
- 最新报告被后续变更影响时必须重新执行；检查器会拒绝指纹不一致的旧结果。
- 未运行的 Swift/外部协议或夹具独立目标均保持 NOT RUN；本报告不代表系统 VPN、外部服务端及跨平台已经验收。

<!-- nodus-test-documentation:begin -->
## 测试目的与验收目标

目的：防止验证前运行、取消后脱离系统步骤、回滚失败误报恢复、旧事务控制新事务及终态持有活动资源。
目标：四阶段顺序固定；原故障和回滚故障分别保留；取消必须等待原步骤确定结束；全局拒绝冲突接管；成功清理撤销内核持有的步骤引用，失败保留可恢复对象。
方法：仅使用密封的本地测试步骤和同步门控，按规范给出独立顺序、状态与错误断言；通过 Weak 观察内核是否仍持有步骤，不用引用数裁决活动权限。不改变系统网络。性能测试不适用，本项没有吞吐、耗时或 RSS 验收目标。

## 本轮故障复现与修复依据

首次加入 terminal_cleanup_releases_steps_and_failure_retains_recovery 后，10项实际执行、9通过、1失败，进程退出101；断言“清理完成后仍持有步骤资源”复现全局终态保留步骤对象。原始证据保存在 /private/tmp/nodus-transaction-resource-regression/ 的首轮日志和机器结果；修复后重跑结果见自动报告，不能抹去此前失败。

## 本轮验证范围与独立判断

- L1：本轮 `cargo fmt --all --check`、`cargo clippy --locked --offline --all-targets -- -D warnings` 实际退出0；新模块没有 unsafe、文件写入或新依赖，事务槽固定一项，阶段执行与资源析构均受任务监管。
- L2：10个用例由显式模块和清单登记；本轮额外通过门控停在 Verify 并检查状态，防止仅检查调用列表而漏掉提前发布 Running。
- L3：依据生命周期关闭完成条款，成功清理后的摘要不得保留步骤资源；依据失败恢复条款，回滚失败不能释放唯一恢复对象或准许新启动。新回归分别检查这两条相反边界，修复前失败、修复后通过。未核实真实平台步骤、系统路由或实际 VPN，不作对应通过结论。
- L4：本轮原始日志、发现列表、退出状态及源码指纹均在自动报告指定目录；首轮失败固定为 test_vpn_transaction.attempt-1.log／json。不以本地测试替身作为生产系统证明。

## 历史驱动结果（不作为本轮验证）


- 最终联合回归18项，18通过、0失败、0跳过；进程退出0。新事务套件9项、既有候选套件9项，单独归属，不代表全量Rust/FFI或系统VPN验收。
- `cargo clippy --offline --locked --lib --tests --examples -- -D warnings`退出0，日志`/private/tmp/nodus-startup-transaction-clippy-verified.log`；三份本项Rust源文件rustfmt检查通过。只读示例实际运行退出0，输出无活动事务，不启动网络，日志`/private/tmp/nodus-startup-transaction-example.log`。
- 最初测试误用不存在的ConnectionFailed错误变体，编译失败、用例未执行；改为现有Network后8项通过。增加panic路径后9项通过；Clippy曾指出嵌套if和文档段落问题，修复后严格检查通过，没有禁用规则。
- 有一轮18项用例本身通过，但运行期间用户更新开发规范，指纹失效，执行器退出3并将报告标为STALE；本轮按新规范归档说明后重跑，退出0。此前失败/过期证据保存在同一证据目录各attempt文件，不抹去历史。
- 本模块目前只有密封测试步骤，没有生产步骤，未注册FFI、未构建或复制新发行内核到App；不打包、安装、启动或上传App。新源码版本2.9.0-dev仅标识兼容新增的Rust驱动，不代表系统VPN能力已交付。
- 语义范围：固定四阶段顺序、Verify失败不Running、取消等待原步骤停止、全局拒绝并发启动、旧ID隔离、前端对象消失后仍保留恢复、原错误与清理错误分离。真实出站/DNS/峰值预算、可信授权、平台逆向恢复、持久化进程恢复和最低macOS实机均未验证。
- 临时编译目录在本轮核验后清理，仅保留原始证据；用户修改保留。状态机测试不能代替真实网络和平台检查，也不意味着“完整四阶段启动事务”已经完成。

<!-- nodus-test-documentation:end -->
