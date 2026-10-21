# 测试报告：test_test_reporting

<!-- fluxa-test-report:{"command": ["python3", "tests/test_test_reporting.py", "-v", "-b"], "exit_code": 0, "duration_seconds": 0.12561166699999998, "status": "PASS", "discovered": ["test_nonzero_is_never_pass", "test_missing_and_ignored_are_not_full_pass", "test_exact_case_ownership", "test_timeout_retains_child_status", "test_compile_failure_and_report_failure_keep_nonzero", "test_signaled_external_process_never_returns_success", "test_unrun_collection_cannot_overwrite_actual_external_result", "test_support_fixture_cannot_be_counted_as_passing_suite", "test_named_external_suites_require_actual_cases"], "results": {"test_nonzero_is_never_pass": "PASS", "test_missing_and_ignored_are_not_full_pass": "PASS", "test_exact_case_ownership": "PASS", "test_timeout_retains_child_status": "PASS", "test_compile_failure_and_report_failure_keep_nonzero": "PASS", "test_signaled_external_process_never_returns_success": "PASS", "test_unrun_collection_cannot_overwrite_actual_external_result": "PASS", "test_support_fixture_cannot_be_counted_as_passing_suite": "PASS", "test_named_external_suites_require_actual_cases": "PASS"}, "fingerprint": "195b9d9bf4f4f354ce9a8ad08fd3ffd134d1d1c9f76083014646048b21a9d89b", "time": "2026-10-21T13:02:28.494135+08:00", "scope": "逐项核对实际测试框架输出与本文件登记函数；零用例或缺少实际结果不计通过。", "note": "以真实外部进程退出状态验证程序整体；编译成功不代表测试已运行。内部断言数量未知，不与 Rust 函数数量合并。", "attempt": 1, "suite": "test_test_reporting", "file": "tests/test_test_reporting.py", "environment": {"os": "macOS-27.2-arm64-arm-64bit", "architecture": "arm64", "rust": "rustc 1.98.1 (48a229cea 2026-09-01)", "cargo": "cargo 1.98.1 (797e8a9bc 2026-08-05)", "commit": "8b17fa48772345e5ef38f042c1aa1b88d4555dbc", "dirty": true, "features": "Cargo 默认 features；未启用 --all-features", "optional_environments": {"FLUXA_DEBUG_TEST_REQUIREMENT": false, "FLUXA_DEBUG_TEST_SWIFT": false, "FLUXA_PROTOCOL_PROFILE": false, "FLUXA_PROTOCOL_FIXTURE": false}, "ci_run_id": null, "ci_run_attempt": null}} -->

## 基本信息

- 测试文件：`tests/test_test_reporting.py`
- 测试对象：Tools/test_reports.py：测试运行与报告失败保全契约
- 执行机制：python_unit
- 测试范围：逐项核对实际测试框架输出与本文件登记函数；零用例或缺少实际结果不计通过。
- 实际命令：`python3 tests/test_test_reporting.py -v -b`
- 执行时间：2026-10-21T13:02:28.494135+08:00
- 实际耗时：0.12561166699999998 秒
- 运行环境：`{"os": "macOS-27.2-arm64-arm-64bit", "architecture": "arm64", "rust": "rustc 1.98.1 (48a229cea 2026-09-01)", "cargo": "cargo 1.98.1 (797e8a9bc 2026-08-05)", "commit": "8b17fa48772345e5ef38f042c1aa1b88d4555dbc", "dirty": true, "features": "Cargo 默认 features；未启用 --all-features", "optional_environments": {"FLUXA_DEBUG_TEST_REQUIREMENT": false, "FLUXA_DEBUG_TEST_SWIFT": false, "FLUXA_PROTOCOL_PROFILE": false, "FLUXA_PROTOCOL_FIXTURE": false}, "ci_run_id": null, "ci_run_attempt": null}`
- 代码/测试/配置指纹：`195b9d9bf4f4f354ce9a8ad08fd3ffd134d1d1c9f76083014646048b21a9d89b`
- 总体状态：**PASS**
- 原始进程退出状态：0

## 实际用例与结果

| 实际发现的用例 | 独立目标 | 状态 |
| --- | --- | --- |
| `test_nonzero_is_never_pass` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `test_missing_and_ignored_are_not_full_pass` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `test_exact_case_ownership` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `test_timeout_retains_child_status` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `test_compile_failure_and_report_failure_keep_nonzero` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `test_signaled_external_process_never_returns_success` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `test_unrun_collection_cannot_overwrite_actual_external_result` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `test_support_fixture_cannot_be_counted_as_passing_suite` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `test_named_external_suites_require_actual_cases` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |

## 执行汇总

- 实际发现：9
- 实际执行完成：9
- 通过：9
- 失败：0
- 忽略/跳过：0
- 未取得完整执行结果：0

## 失败、异常与限制

以真实外部进程退出状态验证程序整体；编译成功不代表测试已运行。内部断言数量未知，不与 Rust 函数数量合并。
未覆盖范围以本文件实际断言和前置条件为界，不宣称全模块或全平台覆盖。
不自动重试；若手动重跑，旧报告及原始退出结果保存在证据目录。缺少执行结果时不推定成功。

## 独立 L1–L4 审查

- L1：本次测试构建结果见 build 日志；格式/Clippy 等需另行记录，不自动推断通过。
- L2：已登记用例按 Cargo 工件、发现列表与显式文件映射归属；实际结果见上表。
- L3：未由报告生成器验证；迁移保留断言不等于原业务预期已全面证明，需独立语义审查。
- L4：原始日志、机器结果、版本及指纹可在下述证据核对；未上传的本地证据不冒充 CI 工件。

## 证据及后续事项

- 原始证据：`/private/tmp/fluxa-core-platform-formatted-python-20261021/test_test_reporting.json`、对应 `.log`（实际存在时）。
- CI 工件标识：`本地证据目录；未宣称已上传 CI`
- 最新报告被后续变更影响时必须重新执行；检查器会拒绝指纹不一致的旧结果。
- 未运行的 Swift/外部协议或夹具独立目标均保持 NOT RUN；本报告不代表系统 VPN、外部服务端及跨平台已经验收。
