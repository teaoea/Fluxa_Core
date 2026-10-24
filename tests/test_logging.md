# 测试报告：test_logging

<!-- nodus-test-report:{"command": ["/Users/klee/Documents/Nodus Core/target/debug/deps/nodus_core-81ae8f5f888050a0", "::test_logging::", "--test-threads=1"], "exit_code": 0, "duration_seconds": 0.2609981250000004, "status": "PASS", "discovered": ["logging::test_logging::configuration_failure_keeps_previous_level_and_revision", "logging::test_logging::connection_events_share_id_and_close_once", "logging::test_logging::encrypted_unix_transport_round_trip_and_rotation", "logging::test_logging::encryption_authenticates_and_survives_missing_packets", "logging::test_logging::encryption_bounds_and_exhaustion_never_emit_plaintext", "logging::test_logging::full_receiver_and_busy_output_never_wait", "logging::test_logging::levels_keep_exact_existing_semantics", "logging::test_logging::log_schema_defaults_and_strict_public_key_validation", "logging::test_logging::no_consumer_or_disconnection_never_accumulates_history", "logging::test_logging::structured_fields_are_transferred_without_redaction", "logging::test_logging::unix_output_is_live_datagrams_only"], "results": {"logging::test_logging::configuration_failure_keeps_previous_level_and_revision": "PASS", "logging::test_logging::connection_events_share_id_and_close_once": "PASS", "logging::test_logging::encrypted_unix_transport_round_trip_and_rotation": "PASS", "logging::test_logging::encryption_authenticates_and_survives_missing_packets": "PASS", "logging::test_logging::encryption_bounds_and_exhaustion_never_emit_plaintext": "PASS", "logging::test_logging::full_receiver_and_busy_output_never_wait": "PASS", "logging::test_logging::levels_keep_exact_existing_semantics": "PASS", "logging::test_logging::log_schema_defaults_and_strict_public_key_validation": "PASS", "logging::test_logging::no_consumer_or_disconnection_never_accumulates_history": "PASS", "logging::test_logging::structured_fields_are_transferred_without_redaction": "PASS", "logging::test_logging::unix_output_is_live_datagrams_only": "PASS"}, "fingerprint": "b7bc57f342b032d047f7e777f6d2b411c9dd7a9095658d68f7e07cb7d80d2396", "time": "2026-10-24T19:15:31.886720+08:00", "attempt": 1, "suite": "test_logging", "file": "tests/test_logging.rs", "environment": {"os": "macOS-27.2-arm64-arm-64bit", "architecture": "arm64", "rust": "rustc 1.98.1 (48a229cea 2026-09-01)", "cargo": "cargo 1.98.1 (797e8a9bc 2026-08-05)", "commit": "867077280c32f31b53a10ad45776eb44da3cc243", "dirty": true, "features": "Cargo 默认 features；未启用 --all-features", "optional_environments": {"NODUS_DEBUG_TEST_REQUIREMENT": false, "NODUS_DEBUG_TEST_SWIFT": false, "NODUS_PROTOCOL_PROFILE": false, "NODUS_PROTOCOL_FIXTURE": false}, "ci_run_id": null, "ci_run_attempt": null}} -->

## 基本信息

- 测试文件：`tests/test_logging.rs`
- 测试对象：src/logging.rs、src/outbound.rs、src/outbound/configuration.rs、src/configuration.rs、src/configuration/models.rs
- 执行机制：unit
- 测试范围：本文件登记且在当前平台实际发现的全部用例
- 实际命令：`/Users/klee/Documents/Nodus Core/target/debug/deps/nodus_core-81ae8f5f888050a0 ::test_logging:: --test-threads=1`
- 执行时间：2026-10-24T19:15:31.886720+08:00
- 实际耗时：0.2609981250000004 秒
- 运行环境：`{"os": "macOS-27.2-arm64-arm-64bit", "architecture": "arm64", "rust": "rustc 1.98.1 (48a229cea 2026-09-01)", "cargo": "cargo 1.98.1 (797e8a9bc 2026-08-05)", "commit": "867077280c32f31b53a10ad45776eb44da3cc243", "dirty": true, "features": "Cargo 默认 features；未启用 --all-features", "optional_environments": {"NODUS_DEBUG_TEST_REQUIREMENT": false, "NODUS_DEBUG_TEST_SWIFT": false, "NODUS_PROTOCOL_PROFILE": false, "NODUS_PROTOCOL_FIXTURE": false}, "ci_run_id": null, "ci_run_attempt": null}`
- 代码/测试/配置指纹：`b7bc57f342b032d047f7e777f6d2b411c9dd7a9095658d68f7e07cb7d80d2396`
- 总体状态：**PASS**
- 原始进程退出状态：0

## 实际用例与结果

| 实际发现的用例 | 独立目标 | 状态 |
| --- | --- | --- |
| `logging::test_logging::configuration_failure_keeps_previous_level_and_revision` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `logging::test_logging::connection_events_share_id_and_close_once` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `logging::test_logging::encrypted_unix_transport_round_trip_and_rotation` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `logging::test_logging::encryption_authenticates_and_survives_missing_packets` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `logging::test_logging::encryption_bounds_and_exhaustion_never_emit_plaintext` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `logging::test_logging::full_receiver_and_busy_output_never_wait` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `logging::test_logging::levels_keep_exact_existing_semantics` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `logging::test_logging::log_schema_defaults_and_strict_public_key_validation` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `logging::test_logging::no_consumer_or_disconnection_never_accumulates_history` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `logging::test_logging::structured_fields_are_transferred_without_redaction` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `logging::test_logging::unix_output_is_live_datagrams_only` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |

## 执行汇总

- 实际发现：11
- 实际执行完成：11
- 通过：11
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

- 原始证据：`/private/tmp/nodus-logging-final-evidence/test_logging.json`、对应 `.log`（实际存在时）。
- CI 工件标识：`本地证据目录；未宣称已上传 CI`
- 最新报告被后续变更影响时必须重新执行；检查器会拒绝指纹不一致的旧结果。
- 未运行的 Swift/外部协议或夹具独立目标均保持 NOT RUN；本报告不代表系统 VPN、外部服务端及跨平台已经验收。
