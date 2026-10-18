# 测试报告：test_tun_tcp_session

<!-- fluxa-test-report:{"command": ["/private/tmp/fluxa-tcp-latest/target/debug/deps/libfluxa_core-4d1b10b7d1f04a59", "::test_tun_tcp_session::", "--test-threads=1"], "exit_code": 0, "duration_seconds": 0.5234440419999942, "status": "PASS", "discovered": ["tun::tcp::session::test_tun_tcp_session::absolute_handshake_deadline_cancels_pending_real_dns_before_dial", "tun::tcp::session::test_tun_tcp_session::automatic_fake_ip_uses_original_return_address", "tun::tcp::session::test_tun_tcp_session::close_cancels_pending_tcp_dns_and_revokes_retained_resolver", "tun::tcp::session::test_tun_tcp_session::completed_tasks_reaping_preserves_live_tcp_data", "tun::tcp::session::test_tun_tcp_session::concurrent_close_with_full_output_releases_actual_resources_and_quotas", "tun::tcp::session::test_tun_tcp_session::dns_tcp_length_framing_and_real_resolver", "tun::tcp::session::test_tun_tcp_session::invalid_handshake_packets_do_not_refresh_idle_deadline", "tun::tcp::session::test_tun_tcp_session::network_update_preserves_active_stream_and_rejects_new_flow", "tun::tcp::session::test_tun_tcp_session::real_ipv4_ipv6_data_and_half_close", "tun::tcp::session::test_tun_tcp_session::rejection_invalid_input_and_close_never_dial", "tun::tcp::session::test_tun_tcp_session::scaled_flow_budget_failure_is_atomic_and_close_releases_ring"], "results": {"tun::tcp::session::test_tun_tcp_session::absolute_handshake_deadline_cancels_pending_real_dns_before_dial": "PASS", "tun::tcp::session::test_tun_tcp_session::automatic_fake_ip_uses_original_return_address": "PASS", "tun::tcp::session::test_tun_tcp_session::close_cancels_pending_tcp_dns_and_revokes_retained_resolver": "PASS", "tun::tcp::session::test_tun_tcp_session::completed_tasks_reaping_preserves_live_tcp_data": "PASS", "tun::tcp::session::test_tun_tcp_session::concurrent_close_with_full_output_releases_actual_resources_and_quotas": "PASS", "tun::tcp::session::test_tun_tcp_session::dns_tcp_length_framing_and_real_resolver": "PASS", "tun::tcp::session::test_tun_tcp_session::invalid_handshake_packets_do_not_refresh_idle_deadline": "PASS", "tun::tcp::session::test_tun_tcp_session::network_update_preserves_active_stream_and_rejects_new_flow": "PASS", "tun::tcp::session::test_tun_tcp_session::real_ipv4_ipv6_data_and_half_close": "PASS", "tun::tcp::session::test_tun_tcp_session::rejection_invalid_input_and_close_never_dial": "PASS", "tun::tcp::session::test_tun_tcp_session::scaled_flow_budget_failure_is_atomic_and_close_releases_ring": "PASS"}, "fingerprint": "0ab210900f6c1a572f9d17ed7b2b300142a30c1517e24f0c54af051832960502", "time": "2026-10-17T21:24:40.615935+08:00", "attempt": 1, "suite": "test_tun_tcp_session", "file": "tests/test_tun_tcp_session.rs", "environment": {"os": "macOS-27.2-arm64-arm-64bit", "architecture": "arm64", "rust": "rustc 1.98.1 (48a229cea 2026-09-01)", "cargo": "cargo 1.98.1 (797e8a9bc 2026-08-05)", "commit": "4cd3dfe7adfa6bec4c12cf8ba3abc4db7b75e575", "dirty": true, "features": "Cargo 默认 features；未启用 --all-features", "optional_environments": {"FLUXA_DEBUG_TEST_REQUIREMENT": false, "FLUXA_DEBUG_TEST_SWIFT": false, "FLUXA_PROTOCOL_PROFILE": false, "FLUXA_PROTOCOL_FIXTURE": false}, "ci_run_id": null, "ci_run_attempt": null}} -->

## 基本信息

- 测试文件：`tests/test_tun_tcp_session.rs`
- 测试对象：src/tun/tcp/session.rs
- 执行机制：unit
- 测试范围：本文件登记且在当前平台实际发现的全部用例
- 实际命令：`/private/tmp/fluxa-tcp-latest/target/debug/deps/libfluxa_core-4d1b10b7d1f04a59 ::test_tun_tcp_session:: --test-threads=1`
- 执行时间：2026-10-17T21:24:40.615935+08:00
- 实际耗时：0.5234440419999942 秒
- 运行环境：`{"os": "macOS-27.2-arm64-arm-64bit", "architecture": "arm64", "rust": "rustc 1.98.1 (48a229cea 2026-09-01)", "cargo": "cargo 1.98.1 (797e8a9bc 2026-08-05)", "commit": "4cd3dfe7adfa6bec4c12cf8ba3abc4db7b75e575", "dirty": true, "features": "Cargo 默认 features；未启用 --all-features", "optional_environments": {"FLUXA_DEBUG_TEST_REQUIREMENT": false, "FLUXA_DEBUG_TEST_SWIFT": false, "FLUXA_PROTOCOL_PROFILE": false, "FLUXA_PROTOCOL_FIXTURE": false}, "ci_run_id": null, "ci_run_attempt": null}`
- 代码/测试/配置指纹：`0ab210900f6c1a572f9d17ed7b2b300142a30c1517e24f0c54af051832960502`
- 总体状态：**PASS**
- 原始进程退出状态：0

## 实际用例与结果

| 实际发现的用例 | 独立目标 | 状态 |
| --- | --- | --- |
| `tun::tcp::session::test_tun_tcp_session::absolute_handshake_deadline_cancels_pending_real_dns_before_dial` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `tun::tcp::session::test_tun_tcp_session::automatic_fake_ip_uses_original_return_address` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `tun::tcp::session::test_tun_tcp_session::close_cancels_pending_tcp_dns_and_revokes_retained_resolver` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `tun::tcp::session::test_tun_tcp_session::completed_tasks_reaping_preserves_live_tcp_data` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `tun::tcp::session::test_tun_tcp_session::concurrent_close_with_full_output_releases_actual_resources_and_quotas` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `tun::tcp::session::test_tun_tcp_session::dns_tcp_length_framing_and_real_resolver` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `tun::tcp::session::test_tun_tcp_session::invalid_handshake_packets_do_not_refresh_idle_deadline` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `tun::tcp::session::test_tun_tcp_session::network_update_preserves_active_stream_and_rejects_new_flow` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `tun::tcp::session::test_tun_tcp_session::real_ipv4_ipv6_data_and_half_close` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `tun::tcp::session::test_tun_tcp_session::rejection_invalid_input_and_close_never_dial` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `tun::tcp::session::test_tun_tcp_session::scaled_flow_budget_failure_is_atomic_and_close_releases_ring` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |

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

- 原始证据：`/private/tmp/fluxa-pmtu-accepted-rust/test_tun_tcp_session.json`、对应 `.log`（实际存在时）。
- CI 工件标识：`本地证据目录；未宣称已上传 CI`
- 最新报告被后续变更影响时必须重新执行；检查器会拒绝指纹不一致的旧结果。
- 未运行的 Swift/外部协议或夹具独立目标均保持 NOT RUN；本报告不代表系统 VPN、外部服务端及跨平台已经验收。
