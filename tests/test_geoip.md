# 测试报告：test_geoip

<!-- fluxa-test-report:{"command": ["/private/tmp/fluxa-tcp-latest/target/debug/deps/libfluxa_core-1494eed5db643679", "::test_geoip::", "--test-threads=1"], "exit_code": 0, "duration_seconds": 60.221158792000004, "status": "PASS", "discovered": ["geoip::test_geoip::classification_tests::bounded_diagnostic_retains_download_reason_and_correct_levels", "geoip::test_geoip::classification_tests::complete_examples_have_strict_yaml_and_disabled_geoip", "geoip::test_geoip::public_contract::all_user_assigned_geoip_failure_codes_are_exact", "geoip::test_geoip::public_contract::configured_https_download_integrity_budget_and_no_match", "geoip::test_geoip::public_contract::configured_url_is_strict_and_never_defaults", "geoip::test_geoip::public_contract::offline_ipv4_ipv6_missing_invalid_and_file_limits", "geoip::test_geoip::public_contract::reachable_corruption_is_rejected_and_query_fault_is_not_no_match", "geoip::test_geoip::tests::invalid_database_is_rejected"], "results": {"geoip::test_geoip::classification_tests::bounded_diagnostic_retains_download_reason_and_correct_levels": "PASS", "geoip::test_geoip::classification_tests::complete_examples_have_strict_yaml_and_disabled_geoip": "PASS", "geoip::test_geoip::public_contract::all_user_assigned_geoip_failure_codes_are_exact": "PASS", "geoip::test_geoip::public_contract::configured_https_download_integrity_budget_and_no_match": "PASS", "geoip::test_geoip::public_contract::configured_url_is_strict_and_never_defaults": "PASS", "geoip::test_geoip::public_contract::offline_ipv4_ipv6_missing_invalid_and_file_limits": "PASS", "geoip::test_geoip::public_contract::reachable_corruption_is_rejected_and_query_fault_is_not_no_match": "PASS", "geoip::test_geoip::tests::invalid_database_is_rejected": "PASS"}, "fingerprint": "2987524312908b9f93ce2c36a4472dfe6abf7664f6caf89640788a0bc0dcd293", "time": "2026-10-15T23:50:40.763384+08:00", "attempt": 1, "suite": "test_geoip", "file": "tests/test_geoip.rs", "environment": {"os": "macOS-27.2-arm64-arm-64bit", "architecture": "arm64", "rust": "rustc 1.98.1 (48a229cea 2026-09-01)", "cargo": "cargo 1.98.1 (797e8a9bc 2026-08-05)", "commit": "85f8a4ea1141c68ce5a931bde09859ae722d88b2", "dirty": true, "features": "Cargo 默认 features；未启用 --all-features", "optional_environments": {"FLUXA_DEBUG_TEST_REQUIREMENT": false, "FLUXA_DEBUG_TEST_SWIFT": false, "FLUXA_PROTOCOL_PROFILE": false, "FLUXA_PROTOCOL_FIXTURE": false}, "ci_run_id": null, "ci_run_attempt": null}} -->

## 基本信息

- 测试文件：`tests/test_geoip.rs`
- 测试对象：src/geoip.rs
- 执行机制：unit
- 测试范围：本文件登记且在当前平台实际发现的全部用例
- 实际命令：`/private/tmp/fluxa-tcp-latest/target/debug/deps/libfluxa_core-1494eed5db643679 ::test_geoip:: --test-threads=1`
- 执行时间：2026-10-15T23:50:40.763384+08:00
- 实际耗时：60.221158792000004 秒
- 运行环境：`{"os": "macOS-27.2-arm64-arm-64bit", "architecture": "arm64", "rust": "rustc 1.98.1 (48a229cea 2026-09-01)", "cargo": "cargo 1.98.1 (797e8a9bc 2026-08-05)", "commit": "85f8a4ea1141c68ce5a931bde09859ae722d88b2", "dirty": true, "features": "Cargo 默认 features；未启用 --all-features", "optional_environments": {"FLUXA_DEBUG_TEST_REQUIREMENT": false, "FLUXA_DEBUG_TEST_SWIFT": false, "FLUXA_PROTOCOL_PROFILE": false, "FLUXA_PROTOCOL_FIXTURE": false}, "ci_run_id": null, "ci_run_attempt": null}`
- 代码/测试/配置指纹：`2987524312908b9f93ce2c36a4472dfe6abf7664f6caf89640788a0bc0dcd293`
- 总体状态：**PASS**
- 原始进程退出状态：0

## 实际用例与结果

| 实际发现的用例 | 独立目标 | 状态 |
| --- | --- | --- |
| `geoip::test_geoip::classification_tests::bounded_diagnostic_retains_download_reason_and_correct_levels` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `geoip::test_geoip::classification_tests::complete_examples_have_strict_yaml_and_disabled_geoip` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `geoip::test_geoip::public_contract::all_user_assigned_geoip_failure_codes_are_exact` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `geoip::test_geoip::public_contract::configured_https_download_integrity_budget_and_no_match` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `geoip::test_geoip::public_contract::configured_url_is_strict_and_never_defaults` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `geoip::test_geoip::public_contract::offline_ipv4_ipv6_missing_invalid_and_file_limits` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `geoip::test_geoip::public_contract::reachable_corruption_is_rejected_and_query_fault_is_not_no_match` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `geoip::test_geoip::tests::invalid_database_is_rejected` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |

## 执行汇总

- 实际发现：8
- 实际执行完成：8
- 通过：8
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

- 原始证据：`/private/tmp/fluxa-tcp-ci-fix-rust/test_geoip.json`、对应 `.log`（实际存在时）。
- CI 工件标识：`本地证据目录；未宣称已上传 CI`
- 最新报告被后续变更影响时必须重新执行；检查器会拒绝指纹不一致的旧结果。
- 未运行的 Swift/外部协议或夹具独立目标均保持 NOT RUN；本报告不代表系统 VPN、外部服务端及跨平台已经验收。
