# 测试报告：test_vpn_startup_budget

<!-- nodus-test-report:{"command": ["/Users/klee/Documents/Nodus Core/target/debug/deps/nodus_core-8baf1e4489cb936c", "::test_vpn_startup_budget::", "--test-threads=1"], "exit_code": 0, "duration_seconds": 0.00554116600000043, "status": "PASS", "discovered": ["vpn::startup_budget::test_vpn_startup_budget::cancelled_or_absent_observation_never_admits", "vpn::startup_budget::test_vpn_startup_budget::complete_plan_counts_coexisting_and_transient_resources", "vpn::startup_budget::test_vpn_startup_budget::each_missing_component_is_unknown_not_zero", "vpn::startup_budget::test_vpn_startup_budget::observations_are_transaction_bound_fresh_and_one_shot", "vpn::startup_budget::test_vpn_startup_budget::overflow_is_rejected_in_each_dimension", "vpn::startup_budget::test_vpn_startup_budget::process_and_system_limits_are_independent", "vpn::startup_budget::test_vpn_startup_budget::summary_is_diagnostic_only"], "results": {"vpn::startup_budget::test_vpn_startup_budget::cancelled_or_absent_observation_never_admits": "PASS", "vpn::startup_budget::test_vpn_startup_budget::complete_plan_counts_coexisting_and_transient_resources": "PASS", "vpn::startup_budget::test_vpn_startup_budget::each_missing_component_is_unknown_not_zero": "PASS", "vpn::startup_budget::test_vpn_startup_budget::observations_are_transaction_bound_fresh_and_one_shot": "PASS", "vpn::startup_budget::test_vpn_startup_budget::overflow_is_rejected_in_each_dimension": "PASS", "vpn::startup_budget::test_vpn_startup_budget::process_and_system_limits_are_independent": "PASS", "vpn::startup_budget::test_vpn_startup_budget::summary_is_diagnostic_only": "PASS"}, "fingerprint": "6fc3945c4a2c08bc43c7be8319cda31cdd490f12ed87c5e5681fca76efc2e568", "time": "2026-10-24T14:58:07.432509+08:00", "attempt": 2, "suite": "test_vpn_startup_budget", "file": "tests/test_vpn_startup_budget.rs", "environment": {"os": "macOS-27.2-arm64-arm-64bit", "architecture": "arm64", "rust": "rustc 1.98.1 (48a229cea 2026-09-01)", "cargo": "cargo 1.98.1 (797e8a9bc 2026-08-05)", "commit": "46e7b82bf1cb57f9e1fd11a0546e60194bb6423c", "dirty": true, "features": "Cargo 默认 features；未启用 --all-features", "optional_environments": {"NODUS_DEBUG_TEST_REQUIREMENT": false, "NODUS_DEBUG_TEST_SWIFT": false, "NODUS_PROTOCOL_PROFILE": false, "NODUS_PROTOCOL_FIXTURE": false}, "ci_run_id": null, "ci_run_attempt": null}} -->

## 基本信息

- 测试文件：`tests/test_vpn_startup_budget.rs`
- 测试对象：src/vpn/startup_budget.rs
- 执行机制：unit
- 测试范围：本文件登记且在当前平台实际发现的全部用例
- 实际命令：`/Users/klee/Documents/Nodus Core/target/debug/deps/nodus_core-8baf1e4489cb936c ::test_vpn_startup_budget:: --test-threads=1`
- 执行时间：2026-10-24T14:58:07.432509+08:00
- 实际耗时：0.00554116600000043 秒
- 运行环境：`{"os": "macOS-27.2-arm64-arm-64bit", "architecture": "arm64", "rust": "rustc 1.98.1 (48a229cea 2026-09-01)", "cargo": "cargo 1.98.1 (797e8a9bc 2026-08-05)", "commit": "46e7b82bf1cb57f9e1fd11a0546e60194bb6423c", "dirty": true, "features": "Cargo 默认 features；未启用 --all-features", "optional_environments": {"NODUS_DEBUG_TEST_REQUIREMENT": false, "NODUS_DEBUG_TEST_SWIFT": false, "NODUS_PROTOCOL_PROFILE": false, "NODUS_PROTOCOL_FIXTURE": false}, "ci_run_id": null, "ci_run_attempt": null}`
- 代码/测试/配置指纹：`6fc3945c4a2c08bc43c7be8319cda31cdd490f12ed87c5e5681fca76efc2e568`
- 总体状态：**PASS**
- 原始进程退出状态：0

## 实际用例与结果

| 实际发现的用例 | 独立目标 | 状态 |
| --- | --- | --- |
| `vpn::startup_budget::test_vpn_startup_budget::cancelled_or_absent_observation_never_admits` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `vpn::startup_budget::test_vpn_startup_budget::complete_plan_counts_coexisting_and_transient_resources` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `vpn::startup_budget::test_vpn_startup_budget::each_missing_component_is_unknown_not_zero` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `vpn::startup_budget::test_vpn_startup_budget::observations_are_transaction_bound_fresh_and_one_shot` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `vpn::startup_budget::test_vpn_startup_budget::overflow_is_rejected_in_each_dimension` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `vpn::startup_budget::test_vpn_startup_budget::process_and_system_limits_are_independent` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `vpn::startup_budget::test_vpn_startup_budget::summary_is_diagnostic_only` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |

## 执行汇总

- 实际发现：7
- 实际执行完成：7
- 通过：7
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

- 原始证据：`/private/tmp/nodus-startup-budget-final-evidence/test_vpn_startup_budget.json`、对应 `.log`（实际存在时）。
- CI 工件标识：`本地证据目录；未宣称已上传 CI`
- 最新报告被后续变更影响时必须重新执行；检查器会拒绝指纹不一致的旧结果。
- 未运行的 Swift/外部协议或夹具独立目标均保持 NOT RUN；本报告不代表系统 VPN、外部服务端及跨平台已经验收。

<!-- nodus-test-documentation:begin -->
## 测试目的与验收标准

防止启动资源核算漏掉仍存活的旧状态、缓冲和临时数据，或把未知需求、总容量和过期观测当成准入证据。独立预期来自VPN启动规范和会话生命周期规范，使用100+200+30+40+5=375字节、10+20+3+4+1=38FD的共存峰值；系统新增需求275字节/28FD，等于允许、少1拒绝。

七个真实测试分别核对共存/追加计算、四类未知与显式零、内存/FD及进程/系统各自超限、五部分两维溢出、事务错配/时间边界/单次消费、缺观测/取消及摘要无授权输入。不读取真实OS余量，不构造系统接管；不宣称实际RSS、FD峰值、资源预留、macOS13或真实VPN验收。性能测试不适用。

L3静态语义核对：旧快照未因逻辑关闭而扣除；系统空闲量仅比较新增需求，避免重复扣旧占用；未知不等于零，所有失败保持budget_satisfied=false；没有FFI/配置证据注入入口。计划完整性和可信采集器的生产实现尚未验证。
<!-- nodus-test-documentation:end -->
