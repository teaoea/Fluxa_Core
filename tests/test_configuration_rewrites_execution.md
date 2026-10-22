# 测试报告：test_configuration_rewrites_execution

<!-- nodus-test-report:{"command": ["/Users/klee/Documents/Nodus Core/target/debug/deps/nodus_core-221a120c6527c41c", "::test_configuration_rewrites_execution::", "--test-threads=1"], "exit_code": 0, "duration_seconds": 0.01003066699999522, "status": "PASS", "discovered": ["configuration::rewrites::execution::test_configuration_rewrites_execution::body_conditions_are_explicit", "configuration::rewrites::execution::test_configuration_rewrites_execution::direct_preserves_debug_precedence", "configuration::rewrites::execution::test_configuration_rewrites_execution::disabled_rules_and_unbound_proxy_are_skipped", "configuration::rewrites::execution::test_configuration_rewrites_execution::first_match_has_priority", "configuration::rewrites::execution::test_configuration_rewrites_execution::headers_and_query_have_distinct_case_rules", "configuration::rewrites::execution::test_configuration_rewrites_execution::length_uses_actual_body", "configuration::rewrites::execution::test_configuration_rewrites_execution::media_presets_are_real_bytes", "configuration::rewrites::execution::test_configuration_rewrites_execution::mp4_declares_zero_milliseconds_without_audio", "configuration::rewrites::execution::test_configuration_rewrites_execution::phase_and_unknown_fields_are_rejected", "configuration::rewrites::execution::test_configuration_rewrites_execution::proxy_reference_is_checked", "configuration::rewrites::execution::test_configuration_rewrites_execution::redirect_and_abort_are_distinct", "configuration::rewrites::execution::test_configuration_rewrites_execution::request_conditions_are_and", "configuration::rewrites::execution::test_configuration_rewrites_execution::request_modification_preserves_origin", "configuration::rewrites::execution::test_configuration_rewrites_execution::response_modification_clears_stale_metadata", "configuration::rewrites::execution::test_configuration_rewrites_execution::response_waits_for_original_status_and_mime", "configuration::rewrites::execution::test_configuration_rewrites_execution::script_condition_does_not_fake_execution"], "results": {"configuration::rewrites::execution::test_configuration_rewrites_execution::body_conditions_are_explicit": "PASS", "configuration::rewrites::execution::test_configuration_rewrites_execution::direct_preserves_debug_precedence": "PASS", "configuration::rewrites::execution::test_configuration_rewrites_execution::disabled_rules_and_unbound_proxy_are_skipped": "PASS", "configuration::rewrites::execution::test_configuration_rewrites_execution::first_match_has_priority": "PASS", "configuration::rewrites::execution::test_configuration_rewrites_execution::headers_and_query_have_distinct_case_rules": "PASS", "configuration::rewrites::execution::test_configuration_rewrites_execution::length_uses_actual_body": "PASS", "configuration::rewrites::execution::test_configuration_rewrites_execution::media_presets_are_real_bytes": "PASS", "configuration::rewrites::execution::test_configuration_rewrites_execution::mp4_declares_zero_milliseconds_without_audio": "PASS", "configuration::rewrites::execution::test_configuration_rewrites_execution::phase_and_unknown_fields_are_rejected": "PASS", "configuration::rewrites::execution::test_configuration_rewrites_execution::proxy_reference_is_checked": "PASS", "configuration::rewrites::execution::test_configuration_rewrites_execution::redirect_and_abort_are_distinct": "PASS", "configuration::rewrites::execution::test_configuration_rewrites_execution::request_conditions_are_and": "PASS", "configuration::rewrites::execution::test_configuration_rewrites_execution::request_modification_preserves_origin": "PASS", "configuration::rewrites::execution::test_configuration_rewrites_execution::response_modification_clears_stale_metadata": "PASS", "configuration::rewrites::execution::test_configuration_rewrites_execution::response_waits_for_original_status_and_mime": "PASS", "configuration::rewrites::execution::test_configuration_rewrites_execution::script_condition_does_not_fake_execution": "PASS"}, "fingerprint": "e7dc7bb22d839384bd6a5a9665770e06100a46e88d35ddbf533eb904d38875e5", "time": "2026-10-23T02:28:19.213729+08:00", "attempt": 1, "suite": "test_configuration_rewrites_execution", "file": "tests/test_configuration_rewrites_execution.rs", "environment": {"os": "macOS-27.2-arm64-arm-64bit", "architecture": "arm64", "rust": "rustc 1.98.1 (48a229cea 2026-09-01)", "cargo": "cargo 1.98.1 (797e8a9bc 2026-08-05)", "commit": "69e48b634fd25eff4d80577996322b751f6a1e17", "dirty": true, "features": "Cargo 默认 features；未启用 --all-features", "optional_environments": {"NODUS_DEBUG_TEST_REQUIREMENT": false, "NODUS_DEBUG_TEST_SWIFT": false, "NODUS_PROTOCOL_PROFILE": false, "NODUS_PROTOCOL_FIXTURE": false}, "ci_run_id": null, "ci_run_attempt": null}} -->

## 基本信息

- 测试文件：`tests/test_configuration_rewrites_execution.rs`
- 测试对象：src/configuration/rewrites/execution.rs、src/configuration/rewrites/conditions.rs、src/configuration/rewrites/presets.rs、src/configuration/rewrites.rs、src/https.rs、src/routing.rs
- 执行机制：unit
- 测试范围：本文件登记且在当前平台实际发现的全部用例
- 实际命令：`/Users/klee/Documents/Nodus Core/target/debug/deps/nodus_core-221a120c6527c41c ::test_configuration_rewrites_execution:: --test-threads=1`
- 执行时间：2026-10-23T02:28:19.213729+08:00
- 实际耗时：0.01003066699999522 秒
- 运行环境：`{"os": "macOS-27.2-arm64-arm-64bit", "architecture": "arm64", "rust": "rustc 1.98.1 (48a229cea 2026-09-01)", "cargo": "cargo 1.98.1 (797e8a9bc 2026-08-05)", "commit": "69e48b634fd25eff4d80577996322b751f6a1e17", "dirty": true, "features": "Cargo 默认 features；未启用 --all-features", "optional_environments": {"NODUS_DEBUG_TEST_REQUIREMENT": false, "NODUS_DEBUG_TEST_SWIFT": false, "NODUS_PROTOCOL_PROFILE": false, "NODUS_PROTOCOL_FIXTURE": false}, "ci_run_id": null, "ci_run_attempt": null}`
- 代码/测试/配置指纹：`e7dc7bb22d839384bd6a5a9665770e06100a46e88d35ddbf533eb904d38875e5`
- 总体状态：**PASS**
- 原始进程退出状态：0

## 实际用例与结果

| 实际发现的用例 | 独立目标 | 状态 |
| --- | --- | --- |
| `configuration::rewrites::execution::test_configuration_rewrites_execution::body_conditions_are_explicit` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `configuration::rewrites::execution::test_configuration_rewrites_execution::direct_preserves_debug_precedence` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `configuration::rewrites::execution::test_configuration_rewrites_execution::disabled_rules_and_unbound_proxy_are_skipped` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `configuration::rewrites::execution::test_configuration_rewrites_execution::first_match_has_priority` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `configuration::rewrites::execution::test_configuration_rewrites_execution::headers_and_query_have_distinct_case_rules` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `configuration::rewrites::execution::test_configuration_rewrites_execution::length_uses_actual_body` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `configuration::rewrites::execution::test_configuration_rewrites_execution::media_presets_are_real_bytes` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `configuration::rewrites::execution::test_configuration_rewrites_execution::mp4_declares_zero_milliseconds_without_audio` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `configuration::rewrites::execution::test_configuration_rewrites_execution::phase_and_unknown_fields_are_rejected` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `configuration::rewrites::execution::test_configuration_rewrites_execution::proxy_reference_is_checked` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `configuration::rewrites::execution::test_configuration_rewrites_execution::redirect_and_abort_are_distinct` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `configuration::rewrites::execution::test_configuration_rewrites_execution::request_conditions_are_and` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `configuration::rewrites::execution::test_configuration_rewrites_execution::request_modification_preserves_origin` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `configuration::rewrites::execution::test_configuration_rewrites_execution::response_modification_clears_stale_metadata` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `configuration::rewrites::execution::test_configuration_rewrites_execution::response_waits_for_original_status_and_mime` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `configuration::rewrites::execution::test_configuration_rewrites_execution::script_condition_does_not_fake_execution` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |

## 执行汇总

- 实际发现：16
- 实际执行完成：16
- 通过：16
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

- 原始证据：`/private/tmp/nodus-rename-final/test_configuration_rewrites_execution.json`、对应 `.log`（实际存在时）。
- CI 工件标识：`本地证据目录；未宣称已上传 CI`
- 最新报告被后续变更影响时必须重新执行；检查器会拒绝指纹不一致的旧结果。
- 未运行的 Swift/外部协议或夹具独立目标均保持 NOT RUN；本报告不代表系统 VPN、外部服务端及跨平台已经验收。
