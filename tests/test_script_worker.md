# 测试报告：test_script_worker

<!-- nodus-test-report:{"command": ["/Users/klee/Documents/Nodus Core/target/debug/deps/nodus_core-915003f7fa5ba4b5", "::test_script_worker::", "--test-threads=1"], "exit_code": 0, "duration_seconds": 2.2793715419999927, "status": "PASS", "discovered": ["script::worker::test_script_worker::automatic_expiry_rejects_new_jobs_but_does_not_interrupt_running_js", "script::worker::test_script_worker::clear_interrupts_active_script_and_joins_before_returning", "script::worker::test_script_worker::exact_name_without_extension_is_bound_and_source_is_loaded_lazily", "script::worker::test_script_worker::missing_or_unregistered_resource_is_excluded_without_reinterpreting_url", "script::worker::test_script_worker::one_resource_uses_one_binding_with_shortest_configured_ttl", "script::worker::test_script_worker::repeated_compilation_evicts_only_idle_threads_instead_of_exhausting_cache", "script::worker::test_script_worker::ttl_comes_from_configuration_and_expiry_releases_the_thread"], "results": {"script::worker::test_script_worker::automatic_expiry_rejects_new_jobs_but_does_not_interrupt_running_js": "PASS", "script::worker::test_script_worker::clear_interrupts_active_script_and_joins_before_returning": "PASS", "script::worker::test_script_worker::exact_name_without_extension_is_bound_and_source_is_loaded_lazily": "PASS", "script::worker::test_script_worker::missing_or_unregistered_resource_is_excluded_without_reinterpreting_url": "PASS", "script::worker::test_script_worker::one_resource_uses_one_binding_with_shortest_configured_ttl": "PASS", "script::worker::test_script_worker::repeated_compilation_evicts_only_idle_threads_instead_of_exhausting_cache": "PASS", "script::worker::test_script_worker::ttl_comes_from_configuration_and_expiry_releases_the_thread": "PASS"}, "fingerprint": "6570db01cac088692214174f3c064ba92f94de504c7ddbad674b6c9a5028d243", "time": "2026-10-24T13:41:42.348500+08:00", "attempt": 1, "suite": "test_script_worker", "file": "tests/test_script_worker.rs", "environment": {"os": "macOS-27.2-arm64-arm-64bit", "architecture": "arm64", "rust": "rustc 1.98.1 (48a229cea 2026-09-01)", "cargo": "cargo 1.98.1 (797e8a9bc 2026-08-05)", "commit": "2f6cd1e624851636840713246da2f51fa05c86fb", "dirty": true, "features": "Cargo 默认 features；未启用 --all-features", "optional_environments": {"NODUS_DEBUG_TEST_REQUIREMENT": false, "NODUS_DEBUG_TEST_SWIFT": false, "NODUS_PROTOCOL_PROFILE": false, "NODUS_PROTOCOL_FIXTURE": false}, "ci_run_id": null, "ci_run_attempt": null}} -->

## 基本信息

- 测试文件：`tests/test_script_worker.rs`
- 测试对象：src/script/worker.rs、src/script.rs
- 执行机制：unit
- 测试范围：本文件登记且在当前平台实际发现的全部用例
- 实际命令：`/Users/klee/Documents/Nodus Core/target/debug/deps/nodus_core-915003f7fa5ba4b5 ::test_script_worker:: --test-threads=1`
- 执行时间：2026-10-24T13:41:42.348500+08:00
- 实际耗时：2.2793715419999927 秒
- 运行环境：`{"os": "macOS-27.2-arm64-arm-64bit", "architecture": "arm64", "rust": "rustc 1.98.1 (48a229cea 2026-09-01)", "cargo": "cargo 1.98.1 (797e8a9bc 2026-08-05)", "commit": "2f6cd1e624851636840713246da2f51fa05c86fb", "dirty": true, "features": "Cargo 默认 features；未启用 --all-features", "optional_environments": {"NODUS_DEBUG_TEST_REQUIREMENT": false, "NODUS_DEBUG_TEST_SWIFT": false, "NODUS_PROTOCOL_PROFILE": false, "NODUS_PROTOCOL_FIXTURE": false}, "ci_run_id": null, "ci_run_attempt": null}`
- 代码/测试/配置指纹：`6570db01cac088692214174f3c064ba92f94de504c7ddbad674b6c9a5028d243`
- 总体状态：**PASS**
- 原始进程退出状态：0

## 实际用例与结果

| 实际发现的用例 | 独立目标 | 状态 |
| --- | --- | --- |
| `script::worker::test_script_worker::automatic_expiry_rejects_new_jobs_but_does_not_interrupt_running_js` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `script::worker::test_script_worker::clear_interrupts_active_script_and_joins_before_returning` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `script::worker::test_script_worker::exact_name_without_extension_is_bound_and_source_is_loaded_lazily` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `script::worker::test_script_worker::missing_or_unregistered_resource_is_excluded_without_reinterpreting_url` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `script::worker::test_script_worker::one_resource_uses_one_binding_with_shortest_configured_ttl` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `script::worker::test_script_worker::repeated_compilation_evicts_only_idle_threads_instead_of_exhausting_cache` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `script::worker::test_script_worker::ttl_comes_from_configuration_and_expiry_releases_the_thread` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |

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

- 原始证据：`/private/tmp/claude-501/-Users-klee-Documents-Fluxa/dd1504c4-f781-4692-b482-4e9c216c95e1/scratchpad/core-evidence/test_script_worker.json`、对应 `.log`（实际存在时）。
- CI 工件标识：`本地证据目录；未宣称已上传 CI`
- 最新报告被后续变更影响时必须重新执行；检查器会拒绝指纹不一致的旧结果。
- 未运行的 Swift/外部协议或夹具独立目标均保持 NOT RUN；本报告不代表系统 VPN、外部服务端及跨平台已经验收。
