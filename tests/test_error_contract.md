# 测试报告：test_error_contract

<!-- fluxa-test-report:{"command": ["/private/tmp/fluxa-tcp-latest/target/debug/deps/test_error_contract-03f6c006e50ca96d", "--test-threads=1"], "exit_code": 0, "duration_seconds": 0.15381499999999448, "status": "PASS", "discovered": ["all_abi_failures_use_segmented_codes_and_nonerrors_remain_states", "diagnostics_remain_fixed_size_and_do_not_hold_error_chains", "embedded_kernel_failure_survives_io_wrapper_without_sensitive_text", "every_internal_error_maps_to_the_independent_protocol_identity", "fixed_integer_protocol_has_no_string_or_heap_representation", "invalid_config_keeps_snapshot_and_machine_failure", "nested_and_cyclic_io_causes_preserve_classification_and_remain_bounded", "real_dns_deadline_and_closed_port_fail_without_policy_success", "registered_contract_and_initial_baseline_are_complete", "safety_filter_preserves_unknown_and_duplicate_classification", "segmented_codes_have_one_category_per_prefix", "serialization_failure_remains_a_failure", "strict_error_schema_rejects_unregistered_or_unbounded_data", "typed_context_is_required_bounded_and_specific_to_code"], "results": {"all_abi_failures_use_segmented_codes_and_nonerrors_remain_states": "PASS", "diagnostics_remain_fixed_size_and_do_not_hold_error_chains": "PASS", "embedded_kernel_failure_survives_io_wrapper_without_sensitive_text": "PASS", "every_internal_error_maps_to_the_independent_protocol_identity": "PASS", "fixed_integer_protocol_has_no_string_or_heap_representation": "PASS", "invalid_config_keeps_snapshot_and_machine_failure": "PASS", "nested_and_cyclic_io_causes_preserve_classification_and_remain_bounded": "PASS", "real_dns_deadline_and_closed_port_fail_without_policy_success": "PASS", "registered_contract_and_initial_baseline_are_complete": "PASS", "safety_filter_preserves_unknown_and_duplicate_classification": "PASS", "segmented_codes_have_one_category_per_prefix": "PASS", "serialization_failure_remains_a_failure": "PASS", "strict_error_schema_rejects_unregistered_or_unbounded_data": "PASS", "typed_context_is_required_bounded_and_specific_to_code": "PASS"}, "fingerprint": "0ab210900f6c1a572f9d17ed7b2b300142a30c1517e24f0c54af051832960502", "time": "2026-10-17T21:24:40.615935+08:00", "attempt": 1, "suite": "test_error_contract", "file": "tests/test_error_contract.rs", "environment": {"os": "macOS-27.2-arm64-arm-64bit", "architecture": "arm64", "rust": "rustc 1.98.1 (48a229cea 2026-09-01)", "cargo": "cargo 1.98.1 (797e8a9bc 2026-08-05)", "commit": "4cd3dfe7adfa6bec4c12cf8ba3abc4db7b75e575", "dirty": true, "features": "Cargo 默认 features；未启用 --all-features", "optional_environments": {"FLUXA_DEBUG_TEST_REQUIREMENT": false, "FLUXA_DEBUG_TEST_SWIFT": false, "FLUXA_PROTOCOL_PROFILE": false, "FLUXA_PROTOCOL_FIXTURE": false}, "ci_run_id": null, "ci_run_attempt": null}} -->

## 基本信息

- 测试文件：`tests/test_error_contract.rs`
- 测试对象：src/error.rs
- 执行机制：integration
- 测试范围：本文件登记且在当前平台实际发现的全部用例
- 实际命令：`/private/tmp/fluxa-tcp-latest/target/debug/deps/test_error_contract-03f6c006e50ca96d --test-threads=1`
- 执行时间：2026-10-17T21:24:40.615935+08:00
- 实际耗时：0.15381499999999448 秒
- 运行环境：`{"os": "macOS-27.2-arm64-arm-64bit", "architecture": "arm64", "rust": "rustc 1.98.1 (48a229cea 2026-09-01)", "cargo": "cargo 1.98.1 (797e8a9bc 2026-08-05)", "commit": "4cd3dfe7adfa6bec4c12cf8ba3abc4db7b75e575", "dirty": true, "features": "Cargo 默认 features；未启用 --all-features", "optional_environments": {"FLUXA_DEBUG_TEST_REQUIREMENT": false, "FLUXA_DEBUG_TEST_SWIFT": false, "FLUXA_PROTOCOL_PROFILE": false, "FLUXA_PROTOCOL_FIXTURE": false}, "ci_run_id": null, "ci_run_attempt": null}`
- 代码/测试/配置指纹：`0ab210900f6c1a572f9d17ed7b2b300142a30c1517e24f0c54af051832960502`
- 总体状态：**PASS**
- 原始进程退出状态：0

## 实际用例与结果

| 实际发现的用例 | 独立目标 | 状态 |
| --- | --- | --- |
| `all_abi_failures_use_segmented_codes_and_nonerrors_remain_states` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `diagnostics_remain_fixed_size_and_do_not_hold_error_chains` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `embedded_kernel_failure_survives_io_wrapper_without_sensitive_text` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `every_internal_error_maps_to_the_independent_protocol_identity` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `fixed_integer_protocol_has_no_string_or_heap_representation` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `invalid_config_keeps_snapshot_and_machine_failure` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `nested_and_cyclic_io_causes_preserve_classification_and_remain_bounded` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `real_dns_deadline_and_closed_port_fail_without_policy_success` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `registered_contract_and_initial_baseline_are_complete` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `safety_filter_preserves_unknown_and_duplicate_classification` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `segmented_codes_have_one_category_per_prefix` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `serialization_failure_remains_a_failure` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `strict_error_schema_rejects_unregistered_or_unbounded_data` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `typed_context_is_required_bounded_and_specific_to_code` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |

## 执行汇总

- 实际发现：14
- 实际执行完成：14
- 通过：14
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

- 原始证据：`/private/tmp/fluxa-pmtu-accepted-rust/test_error_contract.json`、对应 `.log`（实际存在时）。
- CI 工件标识：`本地证据目录；未宣称已上传 CI`
- 最新报告被后续变更影响时必须重新执行；检查器会拒绝指纹不一致的旧结果。
- 未运行的 Swift/外部协议或夹具独立目标均保持 NOT RUN；本报告不代表系统 VPN、外部服务端及跨平台已经验收。
