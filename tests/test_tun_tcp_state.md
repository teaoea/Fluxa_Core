# 测试报告：test_tun_tcp_state

<!-- fluxa-test-report:{"command": ["/private/tmp/fluxa-tcp-latest/target/debug/deps/libfluxa_core-185a33e4556c7e57", "::test_tun_tcp_state::", "--test-threads=1"], "exit_code": 0, "duration_seconds": 0.04257183400000031, "status": "PASS", "discovered": ["tun::tcp::state::test_tun_tcp_state::active_half_close_accepts_final_return_bytes_and_fin", "tun::tcp::state::test_tun_tcp_state::duplicate_ack_triggers_fast_retransmit_without_waiting_for_timer", "tun::tcp::state::test_tun_tcp_state::duplicate_syn_consumes_retry_budget_and_handshake_has_absolute_deadline", "tun::tcp::state::test_tun_tcp_state::exponential_retry_budget_exhausts_with_explicit_timeout", "tun::tcp::state::test_tun_tcp_state::finish_waits_for_bounded_flight_ledger_space", "tun::tcp::state::test_tun_tcp_state::half_close_keeps_return_data_then_fin_and_time_wait", "tun::tcp::state::test_tun_tcp_state::pmtu_resegments_outstanding_data_without_losing_tail_or_retry_budget", "tun::tcp::state::test_tun_tcp_state::receive_reordering_duplicate_and_window_update", "tun::tcp::state::test_tun_tcp_state::reset_requires_expected_sequence_and_far_packet_cannot_release_tx", "tun::tcp::state::test_tun_tcp_state::round_trip_estimate_adapts_and_retransmission_is_not_a_sample", "tun::tcp::state::test_tun_tcp_state::sack_negotiated_duplicate_ack_fallback_and_mtu_budget", "tun::tcp::state::test_tun_tcp_state::sack_reneging_rto_and_unsent_ranges_are_safe", "tun::tcp::state::test_tun_tcp_state::sack_reports_out_of_order_and_requires_negotiation", "tun::tcp::state::test_tun_tcp_state::sack_selects_multiple_holes_without_releasing_unacked_bytes", "tun::tcp::state::test_tun_tcp_state::scaled_rounding_never_accepts_bytes_outside_advertised_window", "tun::tcp::state::test_tun_tcp_state::scaled_windows_only_apply_after_syn_and_both_sides_negotiate", "tun::tcp::state::test_tun_tcp_state::segmentation_partial_ack_and_retransmit_preserve_payload", "tun::tcp::state::test_tun_tcp_state::syn_retransmission_and_acknowledgement_validation", "tun::tcp::state::test_tun_tcp_state::zero_window_probe_and_reopen_does_not_consume_data"], "results": {"tun::tcp::state::test_tun_tcp_state::active_half_close_accepts_final_return_bytes_and_fin": "PASS", "tun::tcp::state::test_tun_tcp_state::duplicate_ack_triggers_fast_retransmit_without_waiting_for_timer": "PASS", "tun::tcp::state::test_tun_tcp_state::duplicate_syn_consumes_retry_budget_and_handshake_has_absolute_deadline": "PASS", "tun::tcp::state::test_tun_tcp_state::exponential_retry_budget_exhausts_with_explicit_timeout": "PASS", "tun::tcp::state::test_tun_tcp_state::finish_waits_for_bounded_flight_ledger_space": "PASS", "tun::tcp::state::test_tun_tcp_state::half_close_keeps_return_data_then_fin_and_time_wait": "PASS", "tun::tcp::state::test_tun_tcp_state::pmtu_resegments_outstanding_data_without_losing_tail_or_retry_budget": "PASS", "tun::tcp::state::test_tun_tcp_state::receive_reordering_duplicate_and_window_update": "PASS", "tun::tcp::state::test_tun_tcp_state::reset_requires_expected_sequence_and_far_packet_cannot_release_tx": "PASS", "tun::tcp::state::test_tun_tcp_state::round_trip_estimate_adapts_and_retransmission_is_not_a_sample": "PASS", "tun::tcp::state::test_tun_tcp_state::sack_negotiated_duplicate_ack_fallback_and_mtu_budget": "PASS", "tun::tcp::state::test_tun_tcp_state::sack_reneging_rto_and_unsent_ranges_are_safe": "PASS", "tun::tcp::state::test_tun_tcp_state::sack_reports_out_of_order_and_requires_negotiation": "PASS", "tun::tcp::state::test_tun_tcp_state::sack_selects_multiple_holes_without_releasing_unacked_bytes": "PASS", "tun::tcp::state::test_tun_tcp_state::scaled_rounding_never_accepts_bytes_outside_advertised_window": "PASS", "tun::tcp::state::test_tun_tcp_state::scaled_windows_only_apply_after_syn_and_both_sides_negotiate": "PASS", "tun::tcp::state::test_tun_tcp_state::segmentation_partial_ack_and_retransmit_preserve_payload": "PASS", "tun::tcp::state::test_tun_tcp_state::syn_retransmission_and_acknowledgement_validation": "PASS", "tun::tcp::state::test_tun_tcp_state::zero_window_probe_and_reopen_does_not_consume_data": "PASS"}, "fingerprint": "057cbe1b92439f32ccf6469b84b539fbd830dac34a9f1c7f81072ace804f370c", "time": "2026-10-19T01:06:06.563948+08:00", "attempt": 1, "suite": "test_tun_tcp_state", "file": "tests/test_tun_tcp_state.rs", "environment": {"os": "macOS-27.2-arm64-arm-64bit", "architecture": "arm64", "rust": "rustc 1.98.1 (48a229cea 2026-09-01)", "cargo": "cargo 1.98.1 (797e8a9bc 2026-08-05)", "commit": "e3381616e3625e0dd62ba1163ca1e1b228a00991", "dirty": true, "features": "Cargo 默认 features；未启用 --all-features", "optional_environments": {"FLUXA_DEBUG_TEST_REQUIREMENT": false, "FLUXA_DEBUG_TEST_SWIFT": false, "FLUXA_PROTOCOL_PROFILE": false, "FLUXA_PROTOCOL_FIXTURE": false}, "ci_run_id": null, "ci_run_attempt": null}} -->

## 基本信息

- 测试文件：`tests/test_tun_tcp_state.rs`
- 测试对象：src/tun/tcp/state.rs
- 执行机制：unit
- 测试范围：本文件登记且在当前平台实际发现的全部用例
- 实际命令：`/private/tmp/fluxa-tcp-latest/target/debug/deps/libfluxa_core-185a33e4556c7e57 ::test_tun_tcp_state:: --test-threads=1`
- 执行时间：2026-10-19T01:06:06.563948+08:00
- 实际耗时：0.04257183400000031 秒
- 运行环境：`{"os": "macOS-27.2-arm64-arm-64bit", "architecture": "arm64", "rust": "rustc 1.98.1 (48a229cea 2026-09-01)", "cargo": "cargo 1.98.1 (797e8a9bc 2026-08-05)", "commit": "e3381616e3625e0dd62ba1163ca1e1b228a00991", "dirty": true, "features": "Cargo 默认 features；未启用 --all-features", "optional_environments": {"FLUXA_DEBUG_TEST_REQUIREMENT": false, "FLUXA_DEBUG_TEST_SWIFT": false, "FLUXA_PROTOCOL_PROFILE": false, "FLUXA_PROTOCOL_FIXTURE": false}, "ci_run_id": null, "ci_run_attempt": null}`
- 代码/测试/配置指纹：`057cbe1b92439f32ccf6469b84b539fbd830dac34a9f1c7f81072ace804f370c`
- 总体状态：**PASS**
- 原始进程退出状态：0

## 实际用例与结果

| 实际发现的用例 | 独立目标 | 状态 |
| --- | --- | --- |
| `tun::tcp::state::test_tun_tcp_state::active_half_close_accepts_final_return_bytes_and_fin` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `tun::tcp::state::test_tun_tcp_state::duplicate_ack_triggers_fast_retransmit_without_waiting_for_timer` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `tun::tcp::state::test_tun_tcp_state::duplicate_syn_consumes_retry_budget_and_handshake_has_absolute_deadline` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `tun::tcp::state::test_tun_tcp_state::exponential_retry_budget_exhausts_with_explicit_timeout` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `tun::tcp::state::test_tun_tcp_state::finish_waits_for_bounded_flight_ledger_space` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `tun::tcp::state::test_tun_tcp_state::half_close_keeps_return_data_then_fin_and_time_wait` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `tun::tcp::state::test_tun_tcp_state::pmtu_resegments_outstanding_data_without_losing_tail_or_retry_budget` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `tun::tcp::state::test_tun_tcp_state::receive_reordering_duplicate_and_window_update` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `tun::tcp::state::test_tun_tcp_state::reset_requires_expected_sequence_and_far_packet_cannot_release_tx` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `tun::tcp::state::test_tun_tcp_state::round_trip_estimate_adapts_and_retransmission_is_not_a_sample` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `tun::tcp::state::test_tun_tcp_state::sack_negotiated_duplicate_ack_fallback_and_mtu_budget` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `tun::tcp::state::test_tun_tcp_state::sack_reneging_rto_and_unsent_ranges_are_safe` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `tun::tcp::state::test_tun_tcp_state::sack_reports_out_of_order_and_requires_negotiation` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `tun::tcp::state::test_tun_tcp_state::sack_selects_multiple_holes_without_releasing_unacked_bytes` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `tun::tcp::state::test_tun_tcp_state::scaled_rounding_never_accepts_bytes_outside_advertised_window` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `tun::tcp::state::test_tun_tcp_state::scaled_windows_only_apply_after_syn_and_both_sides_negotiate` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `tun::tcp::state::test_tun_tcp_state::segmentation_partial_ack_and_retransmit_preserve_payload` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `tun::tcp::state::test_tun_tcp_state::syn_retransmission_and_acknowledgement_validation` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `tun::tcp::state::test_tun_tcp_state::zero_window_probe_and_reopen_does_not_consume_data` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |

## 执行汇总

- 实际发现：19
- 实际执行完成：19
- 通过：19
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

- 原始证据：`/private/tmp/fluxa-vpn-stack-final/test_tun_tcp_state.json`、对应 `.log`（实际存在时）。
- CI 工件标识：`本地证据目录；未宣称已上传 CI`
- 最新报告被后续变更影响时必须重新执行；检查器会拒绝指纹不一致的旧结果。
- 未运行的 Swift/外部协议或夹具独立目标均保持 NOT RUN；本报告不代表系统 VPN、外部服务端及跨平台已经验收。
