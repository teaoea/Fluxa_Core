# 测试报告：test_outbound_execution

<!-- nodus-test-report:{"command": ["/Users/klee/Documents/Nodus Core/target/mixed-policy/debug/deps/nodus_core-1cc847b6034014b7", "::test_outbound_execution::", "--test-threads=1"], "exit_code": 0, "duration_seconds": 0.9732974580000047, "status": "PASS", "discovered": ["outbound::execution::test_outbound_execution::cancel_during_node_connect_never_reaches_direct", "outbound::execution::test_outbound_execution::destination_connect_failure_does_not_try_next_node", "outbound::execution::test_outbound_execution::ordered_node_failures_use_only_explicit_direct", "outbound::execution::test_outbound_execution::process_proof_admits_new_flow_without_owning_live_connection", "outbound::execution::test_outbound_execution::scoped_quic_is_decided_before_real_udp_outbound", "outbound::execution::test_outbound_execution::successful_direct_stream_never_replays_on_failure", "outbound::execution::test_outbound_execution::udp_node_connect_failure_retries_before_datagram_send"], "results": {"outbound::execution::test_outbound_execution::cancel_during_node_connect_never_reaches_direct": "PASS", "outbound::execution::test_outbound_execution::destination_connect_failure_does_not_try_next_node": "PASS", "outbound::execution::test_outbound_execution::ordered_node_failures_use_only_explicit_direct": "PASS", "outbound::execution::test_outbound_execution::process_proof_admits_new_flow_without_owning_live_connection": "PASS", "outbound::execution::test_outbound_execution::scoped_quic_is_decided_before_real_udp_outbound": "PASS", "outbound::execution::test_outbound_execution::successful_direct_stream_never_replays_on_failure": "PASS", "outbound::execution::test_outbound_execution::udp_node_connect_failure_retries_before_datagram_send": "PASS"}, "fingerprint": "f0ca91027f3c767199f4c217fa27a1e6f22131f06863e5e6bc4dbb6c4f8d541e", "time": "2026-10-24T12:56:18.186920+08:00", "attempt": 3, "suite": "test_outbound_execution", "file": "tests/test_outbound_execution.rs", "environment": {"os": "macOS-27.2-arm64-arm-64bit", "architecture": "arm64", "rust": "rustc 1.98.1 (48a229cea 2026-09-01)", "cargo": "cargo 1.98.1 (797e8a9bc 2026-08-05)", "commit": "3a0e649264f7c48cd4d80dee20ca39035df9fcf4", "dirty": true, "features": "Cargo 默认 features；未启用 --all-features", "optional_environments": {"NODUS_DEBUG_TEST_REQUIREMENT": false, "NODUS_DEBUG_TEST_SWIFT": false, "NODUS_PROTOCOL_PROFILE": false, "NODUS_PROTOCOL_FIXTURE": false}, "ci_run_id": null, "ci_run_attempt": null}} -->

## 基本信息

- 测试文件：`tests/test_outbound_execution.rs`
- 测试对象：src/outbound/execution.rs
- 执行机制：unit
- 测试范围：本文件登记且在当前平台实际发现的全部用例
- 实际命令：`/Users/klee/Documents/Nodus Core/target/mixed-policy/debug/deps/nodus_core-1cc847b6034014b7 ::test_outbound_execution:: --test-threads=1`
- 执行时间：2026-10-24T12:56:18.186920+08:00
- 实际耗时：0.9732974580000047 秒
- 运行环境：`{"os": "macOS-27.2-arm64-arm-64bit", "architecture": "arm64", "rust": "rustc 1.98.1 (48a229cea 2026-09-01)", "cargo": "cargo 1.98.1 (797e8a9bc 2026-08-05)", "commit": "3a0e649264f7c48cd4d80dee20ca39035df9fcf4", "dirty": true, "features": "Cargo 默认 features；未启用 --all-features", "optional_environments": {"NODUS_DEBUG_TEST_REQUIREMENT": false, "NODUS_DEBUG_TEST_SWIFT": false, "NODUS_PROTOCOL_PROFILE": false, "NODUS_PROTOCOL_FIXTURE": false}, "ci_run_id": null, "ci_run_attempt": null}`
- 代码/测试/配置指纹：`f0ca91027f3c767199f4c217fa27a1e6f22131f06863e5e6bc4dbb6c4f8d541e`
- 总体状态：**PASS**
- 原始进程退出状态：0

## 实际用例与结果

| 实际发现的用例 | 独立目标 | 状态 |
| --- | --- | --- |
| `outbound::execution::test_outbound_execution::cancel_during_node_connect_never_reaches_direct` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `outbound::execution::test_outbound_execution::destination_connect_failure_does_not_try_next_node` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `outbound::execution::test_outbound_execution::ordered_node_failures_use_only_explicit_direct` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `outbound::execution::test_outbound_execution::process_proof_admits_new_flow_without_owning_live_connection` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `outbound::execution::test_outbound_execution::scoped_quic_is_decided_before_real_udp_outbound` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `outbound::execution::test_outbound_execution::successful_direct_stream_never_replays_on_failure` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `outbound::execution::test_outbound_execution::udp_node_connect_failure_retries_before_datagram_send` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |

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

- 原始证据：`/private/tmp/nodus-mixed-policy-evidence/test_outbound_execution.json`、对应 `.log`（实际存在时）。
- CI 工件标识：`本地证据目录；未宣称已上传 CI`
- 最新报告被后续变更影响时必须重新执行；检查器会拒绝指纹不一致的旧结果。
- 未运行的 Swift/外部协议或夹具独立目标均保持 NOT RUN；本报告不代表系统 VPN、外部服务端及跨平台已经验收。
