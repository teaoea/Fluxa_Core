# 测试报告：test_outbound

<!-- fluxa-test-report:{"command": ["/private/tmp/fluxa-tcp-latest/target/debug/deps/libfluxa_core-1494eed5db643679", "::test_outbound::", "--test-threads=1"], "exit_code": 0, "duration_seconds": 0.10793650000000099, "status": "PASS", "discovered": ["outbound::test_outbound::public_contract::ipv6_tcp_and_udp_reach_real_loopback_peers", "outbound::test_outbound::public_contract::latency_probe_closes_connection_without_application_data", "outbound::test_outbound::public_contract::policy_and_invalid_targets_do_not_connect", "outbound::test_outbound::public_contract::read_timeout_recovers_and_close_cancels_blocked_io", "outbound::test_outbound::public_contract::session_limit_reclaims_resources_and_never_reuses_handles", "outbound::test_outbound::public_contract::tcp_duplex_half_close_and_eof_use_real_peer", "outbound::test_outbound::public_contract::tcp_write_timeout_invalidates_connection_to_prevent_duplicate_retry", "outbound::test_outbound::public_contract::udp_keeps_datagram_boundaries_zero_length_and_rejects_truncation"], "results": {"outbound::test_outbound::public_contract::ipv6_tcp_and_udp_reach_real_loopback_peers": "PASS", "outbound::test_outbound::public_contract::latency_probe_closes_connection_without_application_data": "PASS", "outbound::test_outbound::public_contract::policy_and_invalid_targets_do_not_connect": "PASS", "outbound::test_outbound::public_contract::read_timeout_recovers_and_close_cancels_blocked_io": "PASS", "outbound::test_outbound::public_contract::session_limit_reclaims_resources_and_never_reuses_handles": "PASS", "outbound::test_outbound::public_contract::tcp_duplex_half_close_and_eof_use_real_peer": "PASS", "outbound::test_outbound::public_contract::tcp_write_timeout_invalidates_connection_to_prevent_duplicate_retry": "PASS", "outbound::test_outbound::public_contract::udp_keeps_datagram_boundaries_zero_length_and_rejects_truncation": "PASS"}, "fingerprint": "2987524312908b9f93ce2c36a4472dfe6abf7664f6caf89640788a0bc0dcd293", "time": "2026-10-15T23:50:40.763384+08:00", "attempt": 1, "suite": "test_outbound", "file": "tests/test_outbound.rs", "environment": {"os": "macOS-27.2-arm64-arm-64bit", "architecture": "arm64", "rust": "rustc 1.98.1 (48a229cea 2026-09-01)", "cargo": "cargo 1.98.1 (797e8a9bc 2026-08-05)", "commit": "85f8a4ea1141c68ce5a931bde09859ae722d88b2", "dirty": true, "features": "Cargo 默认 features；未启用 --all-features", "optional_environments": {"FLUXA_DEBUG_TEST_REQUIREMENT": false, "FLUXA_DEBUG_TEST_SWIFT": false, "FLUXA_PROTOCOL_PROFILE": false, "FLUXA_PROTOCOL_FIXTURE": false}, "ci_run_id": null, "ci_run_attempt": null}} -->

## 基本信息

- 测试文件：`tests/test_outbound.rs`
- 测试对象：src/outbound.rs
- 执行机制：unit
- 测试范围：本文件登记且在当前平台实际发现的全部用例
- 实际命令：`/private/tmp/fluxa-tcp-latest/target/debug/deps/libfluxa_core-1494eed5db643679 ::test_outbound:: --test-threads=1`
- 执行时间：2026-10-15T23:50:40.763384+08:00
- 实际耗时：0.10793650000000099 秒
- 运行环境：`{"os": "macOS-27.2-arm64-arm-64bit", "architecture": "arm64", "rust": "rustc 1.98.1 (48a229cea 2026-09-01)", "cargo": "cargo 1.98.1 (797e8a9bc 2026-08-05)", "commit": "85f8a4ea1141c68ce5a931bde09859ae722d88b2", "dirty": true, "features": "Cargo 默认 features；未启用 --all-features", "optional_environments": {"FLUXA_DEBUG_TEST_REQUIREMENT": false, "FLUXA_DEBUG_TEST_SWIFT": false, "FLUXA_PROTOCOL_PROFILE": false, "FLUXA_PROTOCOL_FIXTURE": false}, "ci_run_id": null, "ci_run_attempt": null}`
- 代码/测试/配置指纹：`2987524312908b9f93ce2c36a4472dfe6abf7664f6caf89640788a0bc0dcd293`
- 总体状态：**PASS**
- 原始进程退出状态：0

## 实际用例与结果

| 实际发现的用例 | 独立目标 | 状态 |
| --- | --- | --- |
| `outbound::test_outbound::public_contract::ipv6_tcp_and_udp_reach_real_loopback_peers` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `outbound::test_outbound::public_contract::latency_probe_closes_connection_without_application_data` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `outbound::test_outbound::public_contract::policy_and_invalid_targets_do_not_connect` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `outbound::test_outbound::public_contract::read_timeout_recovers_and_close_cancels_blocked_io` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `outbound::test_outbound::public_contract::session_limit_reclaims_resources_and_never_reuses_handles` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `outbound::test_outbound::public_contract::tcp_duplex_half_close_and_eof_use_real_peer` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `outbound::test_outbound::public_contract::tcp_write_timeout_invalidates_connection_to_prevent_duplicate_retry` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `outbound::test_outbound::public_contract::udp_keeps_datagram_boundaries_zero_length_and_rejects_truncation` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |

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

- 原始证据：`/private/tmp/fluxa-tcp-ci-fix-rust/test_outbound.json`、对应 `.log`（实际存在时）。
- CI 工件标识：`本地证据目录；未宣称已上传 CI`
- 最新报告被后续变更影响时必须重新执行；检查器会拒绝指纹不一致的旧结果。
- 未运行的 Swift/外部协议或夹具独立目标均保持 NOT RUN；本报告不代表系统 VPN、外部服务端及跨平台已经验收。
