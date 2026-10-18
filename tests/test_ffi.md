# 测试报告：test_ffi

<!-- fluxa-test-report:{"command": ["/private/tmp/fluxa-tcp-latest/target/debug/deps/libfluxa_core-185a33e4556c7e57", "::test_ffi::", "--test-threads=1"], "exit_code": 0, "duration_seconds": 0.744523125, "status": "PASS", "discovered": ["ffi::test_ffi::tests::abi_rejects_invalid_and_destroyed_handles", "ffi::test_ffi::tests::admission_failure_keeps_tcp_bytes_and_returns_reservations", "ffi::test_ffi::tests::admission_failure_keeps_tun_return_packet", "ffi::test_ffi::tests::candidate_commands_are_safe_on_bounded_host_stack", "ffi::test_ffi::tests::concurrent_owners_release_their_own_handles", "ffi::test_ffi::tests::error_protocol_is_thread_local_and_success_clears_stale_error", "ffi::test_ffi::tests::error_query_remains_available_when_response_registry_is_full", "ffi::test_ffi::tests::failed_test_assertion_returns_owned_response_capacity", "ffi::test_ffi::tests::http_request_works_on_bounded_host_stack", "ffi::test_ffi::tests::invalid_internal_error_encoding_does_not_become_no_error", "ffi::test_ffi::tests::legacy_validation_order_preserves_handle_before_json", "ffi::test_ffi::tests::main_configuration_apply_is_atomic_and_status_is_read_only", "ffi::test_ffi::tests::main_configuration_validation_is_read_only_and_structured", "ffi::test_ffi::tests::main_dns_and_geoip_status_use_same_configuration", "ffi::test_ffi::tests::nested_runtime_does_not_detach_failed_http_calls", "ffi::test_ffi::tests::output_buffers_obey_budget_and_handle_type", "ffi::test_ffi::tests::provider_commands_use_real_tls_and_keep_process_rules_ignored"], "results": {"ffi::test_ffi::tests::abi_rejects_invalid_and_destroyed_handles": "PASS", "ffi::test_ffi::tests::admission_failure_keeps_tcp_bytes_and_returns_reservations": "PASS", "ffi::test_ffi::tests::admission_failure_keeps_tun_return_packet": "PASS", "ffi::test_ffi::tests::candidate_commands_are_safe_on_bounded_host_stack": "PASS", "ffi::test_ffi::tests::concurrent_owners_release_their_own_handles": "PASS", "ffi::test_ffi::tests::error_protocol_is_thread_local_and_success_clears_stale_error": "PASS", "ffi::test_ffi::tests::error_query_remains_available_when_response_registry_is_full": "PASS", "ffi::test_ffi::tests::failed_test_assertion_returns_owned_response_capacity": "PASS", "ffi::test_ffi::tests::http_request_works_on_bounded_host_stack": "PASS", "ffi::test_ffi::tests::invalid_internal_error_encoding_does_not_become_no_error": "PASS", "ffi::test_ffi::tests::legacy_validation_order_preserves_handle_before_json": "PASS", "ffi::test_ffi::tests::main_configuration_apply_is_atomic_and_status_is_read_only": "PASS", "ffi::test_ffi::tests::main_configuration_validation_is_read_only_and_structured": "PASS", "ffi::test_ffi::tests::main_dns_and_geoip_status_use_same_configuration": "PASS", "ffi::test_ffi::tests::nested_runtime_does_not_detach_failed_http_calls": "PASS", "ffi::test_ffi::tests::output_buffers_obey_budget_and_handle_type": "PASS", "ffi::test_ffi::tests::provider_commands_use_real_tls_and_keep_process_rules_ignored": "PASS"}, "fingerprint": "057cbe1b92439f32ccf6469b84b539fbd830dac34a9f1c7f81072ace804f370c", "time": "2026-10-19T01:06:06.563948+08:00", "attempt": 1, "suite": "test_ffi", "file": "tests/test_ffi.rs", "environment": {"os": "macOS-27.2-arm64-arm-64bit", "architecture": "arm64", "rust": "rustc 1.98.1 (48a229cea 2026-09-01)", "cargo": "cargo 1.98.1 (797e8a9bc 2026-08-05)", "commit": "e3381616e3625e0dd62ba1163ca1e1b228a00991", "dirty": true, "features": "Cargo 默认 features；未启用 --all-features", "optional_environments": {"FLUXA_DEBUG_TEST_REQUIREMENT": false, "FLUXA_DEBUG_TEST_SWIFT": false, "FLUXA_PROTOCOL_PROFILE": false, "FLUXA_PROTOCOL_FIXTURE": false}, "ci_run_id": null, "ci_run_attempt": null}} -->

## 基本信息

- 测试文件：`tests/test_ffi.rs`
- 测试对象：src/ffi.rs
- 执行机制：unit
- 测试范围：本文件登记且在当前平台实际发现的全部用例
- 实际命令：`/private/tmp/fluxa-tcp-latest/target/debug/deps/libfluxa_core-185a33e4556c7e57 ::test_ffi:: --test-threads=1`
- 执行时间：2026-10-19T01:06:06.563948+08:00
- 实际耗时：0.744523125 秒
- 运行环境：`{"os": "macOS-27.2-arm64-arm-64bit", "architecture": "arm64", "rust": "rustc 1.98.1 (48a229cea 2026-09-01)", "cargo": "cargo 1.98.1 (797e8a9bc 2026-08-05)", "commit": "e3381616e3625e0dd62ba1163ca1e1b228a00991", "dirty": true, "features": "Cargo 默认 features；未启用 --all-features", "optional_environments": {"FLUXA_DEBUG_TEST_REQUIREMENT": false, "FLUXA_DEBUG_TEST_SWIFT": false, "FLUXA_PROTOCOL_PROFILE": false, "FLUXA_PROTOCOL_FIXTURE": false}, "ci_run_id": null, "ci_run_attempt": null}`
- 代码/测试/配置指纹：`057cbe1b92439f32ccf6469b84b539fbd830dac34a9f1c7f81072ace804f370c`
- 总体状态：**PASS**
- 原始进程退出状态：0

## 实际用例与结果

| 实际发现的用例 | 独立目标 | 状态 |
| --- | --- | --- |
| `ffi::test_ffi::tests::abi_rejects_invalid_and_destroyed_handles` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `ffi::test_ffi::tests::admission_failure_keeps_tcp_bytes_and_returns_reservations` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `ffi::test_ffi::tests::admission_failure_keeps_tun_return_packet` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `ffi::test_ffi::tests::candidate_commands_are_safe_on_bounded_host_stack` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `ffi::test_ffi::tests::concurrent_owners_release_their_own_handles` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `ffi::test_ffi::tests::error_protocol_is_thread_local_and_success_clears_stale_error` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `ffi::test_ffi::tests::error_query_remains_available_when_response_registry_is_full` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `ffi::test_ffi::tests::failed_test_assertion_returns_owned_response_capacity` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `ffi::test_ffi::tests::http_request_works_on_bounded_host_stack` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `ffi::test_ffi::tests::invalid_internal_error_encoding_does_not_become_no_error` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `ffi::test_ffi::tests::legacy_validation_order_preserves_handle_before_json` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `ffi::test_ffi::tests::main_configuration_apply_is_atomic_and_status_is_read_only` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `ffi::test_ffi::tests::main_configuration_validation_is_read_only_and_structured` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `ffi::test_ffi::tests::main_dns_and_geoip_status_use_same_configuration` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `ffi::test_ffi::tests::nested_runtime_does_not_detach_failed_http_calls` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `ffi::test_ffi::tests::output_buffers_obey_budget_and_handle_type` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `ffi::test_ffi::tests::provider_commands_use_real_tls_and_keep_process_rules_ignored` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |

## 执行汇总

- 实际发现：17
- 实际执行完成：17
- 通过：17
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

- 原始证据：`/private/tmp/fluxa-vpn-stack-final/test_ffi.json`、对应 `.log`（实际存在时）。
- CI 工件标识：`本地证据目录；未宣称已上传 CI`
- 最新报告被后续变更影响时必须重新执行；检查器会拒绝指纹不一致的旧结果。
- 未运行的 Swift/外部协议或夹具独立目标均保持 NOT RUN；本报告不代表系统 VPN、外部服务端及跨平台已经验收。

## 2.6.1-dev 独立小栈复核

- 新增 candidate_commands_are_safe_on_bounded_host_stack，实际通过真实 ABI 而非直接调用内部 HTTPS helper，256 KiB 栈上验证候选开始、查询和关闭；本套件 17/17，全量 Rust 475 PASS、1 IGNORED/SKIPPED，退出 0。
- Swift 256 KiB 宿主首次在通用 ffi::command 入口 SIGBUS（-10），崩溃报告 /Users/klee/Library/Logs/DiagnosticReports/client-ffi-2026-10-19-005117.ips；仅隔离两个控制面分支仍失败，第二报告 client-ffi-2026-10-19-005443.ips。保留失败，不用普通栈成功覆盖它。
- 调试 ARM64 反汇编 /private/tmp/fluxa-command-only.txt：入口固定栈帧 521984 字节（约 510 KiB）；分支隔离后 /private/tmp/fluxa-command-final-assembly.txt：52368 字节（约 51 KiB）。这里只量该函数固定栈帧，不代表整条调用链、RSS 或性能指标。
- 分发仍在原准入和 BusyGuard 内同步完成，各变体匹配在独立 noinline 栈帧；没有新增业务线程、放宽配额、改变 API 错误/取消或移动二进制 TUN 热路径。只对生成分发中的未使用借用绑定局部允许 unused_variables，不关闭实质 lint。
- /private/tmp/fluxa-vpn-stack-swift-all.log：20 个真实 Swift FFI 程序全部 PASS、退出 0；候选程序在 256 KiB 线程完成 20 次回收。原始报告归档 /private/tmp/fluxa-vpn-stack-swift-source-evidence.zip，SHA-256 dbdcfc226c649e10f39e95330d69946d29f83c00e58a92da71c9e21f11bed47f。
- L3 按同一命令变体、准入、同步返回及结构化错误语义核查；完整源码 FFI 不能替代已发布远程二进制及真实系统 VPN 验收。
