# 测试报告：test_tun_udp

<!-- fluxa-test-report:{"command": ["/private/tmp/fluxa-tcp-latest/target/debug/deps/libfluxa_core-4d1b10b7d1f04a59", "::test_tun_udp::", "--test-threads=1"], "exit_code": 0, "duration_seconds": 3.196802749999989, "status": "PASS", "discovered": ["tun::udp::test_tun_udp::fragmented_returns_preserve_datagrams_and_close_discards_pending", "tun::udp::test_tun_udp::pmtu_udp_feedback_fragments_next_datagram_and_close_revokes_control", "tun::udp::test_tun_udp::public_contract::bounded_flows_idle_reuse_backpressure_and_close_release_resources", "tun::udp::test_tun_udp::public_contract::fake_ip_requires_live_resolver_generation_and_never_dials_synthetic_address", "tun::udp::test_tun_udp::public_contract::new_flows_use_new_rules_existing_flows_keep_snapshot", "tun::udp::test_tun_udp::public_contract::options_and_return_packet_limits_are_strict", "tun::udp::test_tun_udp::public_contract::oversize_response_is_an_error_and_drop_cancels_blocked_output", "tun::udp::test_tun_udp::public_contract::real_ipv4_ipv6_multiple_datagrams_and_empty_payload", "tun::udp::test_tun_udp::public_contract::reject_missing_configuration_proxy_and_invalid_packets_do_not_send", "tun::udp::test_tun_udp::task_registration_contract::completed_worker_panic_is_observed_once", "tun::udp::test_tun_udp::task_registration_contract::completed_workers_are_reaped_without_aborting_active_tasks", "tun::udp::test_tun_udp::task_registration_contract::registered_task_limit_includes_pending_cancellation_and_recovers", "tun::udp::test_tun_udp::task_registration_contract::udp_flow_churn_reaps_before_registering_new_worker"], "results": {"tun::udp::test_tun_udp::fragmented_returns_preserve_datagrams_and_close_discards_pending": "PASS", "tun::udp::test_tun_udp::pmtu_udp_feedback_fragments_next_datagram_and_close_revokes_control": "PASS", "tun::udp::test_tun_udp::public_contract::bounded_flows_idle_reuse_backpressure_and_close_release_resources": "PASS", "tun::udp::test_tun_udp::public_contract::fake_ip_requires_live_resolver_generation_and_never_dials_synthetic_address": "PASS", "tun::udp::test_tun_udp::public_contract::new_flows_use_new_rules_existing_flows_keep_snapshot": "PASS", "tun::udp::test_tun_udp::public_contract::options_and_return_packet_limits_are_strict": "PASS", "tun::udp::test_tun_udp::public_contract::oversize_response_is_an_error_and_drop_cancels_blocked_output": "PASS", "tun::udp::test_tun_udp::public_contract::real_ipv4_ipv6_multiple_datagrams_and_empty_payload": "PASS", "tun::udp::test_tun_udp::public_contract::reject_missing_configuration_proxy_and_invalid_packets_do_not_send": "PASS", "tun::udp::test_tun_udp::task_registration_contract::completed_worker_panic_is_observed_once": "PASS", "tun::udp::test_tun_udp::task_registration_contract::completed_workers_are_reaped_without_aborting_active_tasks": "PASS", "tun::udp::test_tun_udp::task_registration_contract::registered_task_limit_includes_pending_cancellation_and_recovers": "PASS", "tun::udp::test_tun_udp::task_registration_contract::udp_flow_churn_reaps_before_registering_new_worker": "PASS"}, "fingerprint": "5d233b7c28b170b92dabcb432d2e529720e6996268ba902e3d218eccea829d8b", "time": "2026-10-18T08:14:09.983512+08:00", "attempt": 1, "suite": "test_tun_udp", "file": "tests/test_tun_udp.rs", "environment": {"os": "macOS-27.2-arm64-arm-64bit", "architecture": "arm64", "rust": "rustc 1.98.1 (48a229cea 2026-09-01)", "cargo": "cargo 1.98.1 (797e8a9bc 2026-08-05)", "commit": "58426e230f4dfeff8af828a319f33d0dcf268122", "dirty": true, "features": "Cargo 默认 features；未启用 --all-features", "optional_environments": {"FLUXA_DEBUG_TEST_REQUIREMENT": false, "FLUXA_DEBUG_TEST_SWIFT": false, "FLUXA_PROTOCOL_PROFILE": false, "FLUXA_PROTOCOL_FIXTURE": false}, "ci_run_id": null, "ci_run_attempt": null}} -->

## 基本信息

- 测试文件：`tests/test_tun_udp.rs`
- 测试对象：src/tun/udp.rs
- 执行机制：unit
- 测试范围：本文件登记且在当前平台实际发现的全部用例
- 实际命令：`/private/tmp/fluxa-tcp-latest/target/debug/deps/libfluxa_core-4d1b10b7d1f04a59 ::test_tun_udp:: --test-threads=1`
- 执行时间：2026-10-18T08:14:09.983512+08:00
- 实际耗时：3.196802749999989 秒
- 运行环境：`{"os": "macOS-27.2-arm64-arm-64bit", "architecture": "arm64", "rust": "rustc 1.98.1 (48a229cea 2026-09-01)", "cargo": "cargo 1.98.1 (797e8a9bc 2026-08-05)", "commit": "58426e230f4dfeff8af828a319f33d0dcf268122", "dirty": true, "features": "Cargo 默认 features；未启用 --all-features", "optional_environments": {"FLUXA_DEBUG_TEST_REQUIREMENT": false, "FLUXA_DEBUG_TEST_SWIFT": false, "FLUXA_PROTOCOL_PROFILE": false, "FLUXA_PROTOCOL_FIXTURE": false}, "ci_run_id": null, "ci_run_attempt": null}`
- 代码/测试/配置指纹：`5d233b7c28b170b92dabcb432d2e529720e6996268ba902e3d218eccea829d8b`
- 总体状态：**PASS**
- 原始进程退出状态：0

## 实际用例与结果

| 实际发现的用例 | 独立目标 | 状态 |
| --- | --- | --- |
| `tun::udp::test_tun_udp::fragmented_returns_preserve_datagrams_and_close_discards_pending` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `tun::udp::test_tun_udp::pmtu_udp_feedback_fragments_next_datagram_and_close_revokes_control` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `tun::udp::test_tun_udp::public_contract::bounded_flows_idle_reuse_backpressure_and_close_release_resources` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `tun::udp::test_tun_udp::public_contract::fake_ip_requires_live_resolver_generation_and_never_dials_synthetic_address` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `tun::udp::test_tun_udp::public_contract::new_flows_use_new_rules_existing_flows_keep_snapshot` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `tun::udp::test_tun_udp::public_contract::options_and_return_packet_limits_are_strict` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `tun::udp::test_tun_udp::public_contract::oversize_response_is_an_error_and_drop_cancels_blocked_output` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `tun::udp::test_tun_udp::public_contract::real_ipv4_ipv6_multiple_datagrams_and_empty_payload` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `tun::udp::test_tun_udp::public_contract::reject_missing_configuration_proxy_and_invalid_packets_do_not_send` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `tun::udp::test_tun_udp::task_registration_contract::completed_worker_panic_is_observed_once` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `tun::udp::test_tun_udp::task_registration_contract::completed_workers_are_reaped_without_aborting_active_tasks` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `tun::udp::test_tun_udp::task_registration_contract::registered_task_limit_includes_pending_cancellation_and_recovers` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `tun::udp::test_tun_udp::task_registration_contract::udp_flow_churn_reaps_before_registering_new_worker` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |

## 执行汇总

- 实际发现：13
- 实际执行完成：13
- 通过：13
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

- 原始证据：`/private/tmp/fluxa-pmtu-release-final-rust/test_tun_udp.json`、对应 `.log`（实际存在时）。
- CI 工件标识：`本地证据目录；未宣称已上传 CI`
- 最新报告被后续变更影响时必须重新执行；检查器会拒绝指纹不一致的旧结果。
- 未运行的 Swift/外部协议或夹具独立目标均保持 NOT RUN；本报告不代表系统 VPN、外部服务端及跨平台已经验收。
