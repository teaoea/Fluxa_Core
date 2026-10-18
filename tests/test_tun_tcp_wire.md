# 测试报告：test_tun_tcp_wire

<!-- fluxa-test-report:{"command": ["/private/tmp/fluxa-tcp-latest/target/debug/deps/libfluxa_core-4d1b10b7d1f04a59", "::test_tun_tcp_wire::", "--test-threads=1"], "exit_code": 0, "duration_seconds": 0.008559541000010995, "status": "PASS", "discovered": ["tun::tcp::wire::test_tun_tcp_wire::deterministic_truncated_and_mutated_packets_never_panic", "tun::tcp::wire::test_tun_tcp_wire::exact_fields_roundtrip_for_both_address_families", "tun::tcp::wire::test_tun_tcp_wire::invalid_window_mss_and_checksum_are_rejected", "tun::tcp::wire::test_tun_tcp_wire::malformed_options_and_urgent_data_never_enter_state_machine", "tun::tcp::wire::test_tun_tcp_wire::sack_malformed_options_rejected_after_valid_checksum", "tun::tcp::wire::test_tun_tcp_wire::sack_options_have_independent_wire_layout_and_strict_bounds", "tun::tcp::wire::test_tun_tcp_wire::window_scale_wire_negotiation_clamp_and_invalid_lengths"], "results": {"tun::tcp::wire::test_tun_tcp_wire::deterministic_truncated_and_mutated_packets_never_panic": "PASS", "tun::tcp::wire::test_tun_tcp_wire::exact_fields_roundtrip_for_both_address_families": "PASS", "tun::tcp::wire::test_tun_tcp_wire::invalid_window_mss_and_checksum_are_rejected": "PASS", "tun::tcp::wire::test_tun_tcp_wire::malformed_options_and_urgent_data_never_enter_state_machine": "PASS", "tun::tcp::wire::test_tun_tcp_wire::sack_malformed_options_rejected_after_valid_checksum": "PASS", "tun::tcp::wire::test_tun_tcp_wire::sack_options_have_independent_wire_layout_and_strict_bounds": "PASS", "tun::tcp::wire::test_tun_tcp_wire::window_scale_wire_negotiation_clamp_and_invalid_lengths": "PASS"}, "fingerprint": "0ab210900f6c1a572f9d17ed7b2b300142a30c1517e24f0c54af051832960502", "time": "2026-10-17T21:24:40.615935+08:00", "attempt": 1, "suite": "test_tun_tcp_wire", "file": "tests/test_tun_tcp_wire.rs", "environment": {"os": "macOS-27.2-arm64-arm-64bit", "architecture": "arm64", "rust": "rustc 1.98.1 (48a229cea 2026-09-01)", "cargo": "cargo 1.98.1 (797e8a9bc 2026-08-05)", "commit": "4cd3dfe7adfa6bec4c12cf8ba3abc4db7b75e575", "dirty": true, "features": "Cargo 默认 features；未启用 --all-features", "optional_environments": {"FLUXA_DEBUG_TEST_REQUIREMENT": false, "FLUXA_DEBUG_TEST_SWIFT": false, "FLUXA_PROTOCOL_PROFILE": false, "FLUXA_PROTOCOL_FIXTURE": false}, "ci_run_id": null, "ci_run_attempt": null}} -->

## 基本信息

- 测试文件：`tests/test_tun_tcp_wire.rs`
- 测试对象：src/tun/tcp/wire.rs
- 执行机制：unit
- 测试范围：本文件登记且在当前平台实际发现的全部用例
- 实际命令：`/private/tmp/fluxa-tcp-latest/target/debug/deps/libfluxa_core-4d1b10b7d1f04a59 ::test_tun_tcp_wire:: --test-threads=1`
- 执行时间：2026-10-17T21:24:40.615935+08:00
- 实际耗时：0.008559541000010995 秒
- 运行环境：`{"os": "macOS-27.2-arm64-arm-64bit", "architecture": "arm64", "rust": "rustc 1.98.1 (48a229cea 2026-09-01)", "cargo": "cargo 1.98.1 (797e8a9bc 2026-08-05)", "commit": "4cd3dfe7adfa6bec4c12cf8ba3abc4db7b75e575", "dirty": true, "features": "Cargo 默认 features；未启用 --all-features", "optional_environments": {"FLUXA_DEBUG_TEST_REQUIREMENT": false, "FLUXA_DEBUG_TEST_SWIFT": false, "FLUXA_PROTOCOL_PROFILE": false, "FLUXA_PROTOCOL_FIXTURE": false}, "ci_run_id": null, "ci_run_attempt": null}`
- 代码/测试/配置指纹：`0ab210900f6c1a572f9d17ed7b2b300142a30c1517e24f0c54af051832960502`
- 总体状态：**PASS**
- 原始进程退出状态：0

## 实际用例与结果

| 实际发现的用例 | 独立目标 | 状态 |
| --- | --- | --- |
| `tun::tcp::wire::test_tun_tcp_wire::deterministic_truncated_and_mutated_packets_never_panic` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `tun::tcp::wire::test_tun_tcp_wire::exact_fields_roundtrip_for_both_address_families` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `tun::tcp::wire::test_tun_tcp_wire::invalid_window_mss_and_checksum_are_rejected` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `tun::tcp::wire::test_tun_tcp_wire::malformed_options_and_urgent_data_never_enter_state_machine` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `tun::tcp::wire::test_tun_tcp_wire::sack_malformed_options_rejected_after_valid_checksum` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `tun::tcp::wire::test_tun_tcp_wire::sack_options_have_independent_wire_layout_and_strict_bounds` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `tun::tcp::wire::test_tun_tcp_wire::window_scale_wire_negotiation_clamp_and_invalid_lengths` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |

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

- 原始证据：`/private/tmp/fluxa-pmtu-accepted-rust/test_tun_tcp_wire.json`、对应 `.log`（实际存在时）。
- CI 工件标识：`本地证据目录；未宣称已上传 CI`
- 最新报告被后续变更影响时必须重新执行；检查器会拒绝指纹不一致的旧结果。
- 未运行的 Swift/外部协议或夹具独立目标均保持 NOT RUN；本报告不代表系统 VPN、外部服务端及跨平台已经验收。
