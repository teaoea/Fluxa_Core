# 测试报告：test_tun_tcp_buffer

<!-- fluxa-test-report:{"command": ["/private/tmp/fluxa-macos-team-storage/target/debug/deps/libfluxa_core-318794320ad432f5", "::test_tun_tcp_buffer::", "--test-threads=1"], "exit_code": 0, "duration_seconds": 0.046851999999987015, "status": "PASS", "discovered": ["tun::tcp::buffer::test_tun_tcp_buffer::fixed_window_zero_and_open_after_consume", "tun::tcp::buffer::test_tun_tcp_buffer::reordered_duplicate_overlap_wrap_and_finish", "tun::tcp::buffer::test_tun_tcp_buffer::sack_bitmap_matches_independent_presence_model", "tun::tcp::buffer::test_tun_tcp_buffer::sack_bitmap_word_and_ring_boundaries_after_consume", "tun::tcp::buffer::test_tun_tcp_buffer::sack_blocks_merge_recent_first_and_survive_wrap", "tun::tcp::buffer::test_tun_tcp_buffer::sack_never_reports_bytes_beyond_known_fin", "tun::tcp::buffer::test_tun_tcp_buffer::scaled_receive_ring_is_bounded_and_reopens_after_consumption", "tun::tcp::buffer::test_tun_tcp_buffer::scaled_sack_blocks_cross_old_capacity_and_ring_wrap", "tun::tcp::buffer::test_tun_tcp_buffer::transmit_preserves_unacked_bytes_and_wraps"], "results": {"tun::tcp::buffer::test_tun_tcp_buffer::fixed_window_zero_and_open_after_consume": "PASS", "tun::tcp::buffer::test_tun_tcp_buffer::reordered_duplicate_overlap_wrap_and_finish": "PASS", "tun::tcp::buffer::test_tun_tcp_buffer::sack_bitmap_matches_independent_presence_model": "PASS", "tun::tcp::buffer::test_tun_tcp_buffer::sack_bitmap_word_and_ring_boundaries_after_consume": "PASS", "tun::tcp::buffer::test_tun_tcp_buffer::sack_blocks_merge_recent_first_and_survive_wrap": "PASS", "tun::tcp::buffer::test_tun_tcp_buffer::sack_never_reports_bytes_beyond_known_fin": "PASS", "tun::tcp::buffer::test_tun_tcp_buffer::scaled_receive_ring_is_bounded_and_reopens_after_consumption": "PASS", "tun::tcp::buffer::test_tun_tcp_buffer::scaled_sack_blocks_cross_old_capacity_and_ring_wrap": "PASS", "tun::tcp::buffer::test_tun_tcp_buffer::transmit_preserves_unacked_bytes_and_wraps": "PASS"}, "fingerprint": "95a77d0e822027d3d8d91dc72bd23cd1b197af94788923845e5c4d07153812fe", "time": "2026-10-21T05:14:07.529593+08:00", "attempt": 1, "suite": "test_tun_tcp_buffer", "file": "tests/test_tun_tcp_buffer.rs", "environment": {"os": "macOS-27.2-arm64-arm-64bit", "architecture": "arm64", "rust": "rustc 1.98.1 (48a229cea 2026-09-01)", "cargo": "cargo 1.98.1 (797e8a9bc 2026-08-05)", "commit": "ca03dbe91d30381600e401e82a2cffa3a5d38c87", "dirty": true, "features": "Cargo 默认 features；未启用 --all-features", "optional_environments": {"FLUXA_DEBUG_TEST_REQUIREMENT": false, "FLUXA_DEBUG_TEST_SWIFT": false, "FLUXA_PROTOCOL_PROFILE": false, "FLUXA_PROTOCOL_FIXTURE": false}, "ci_run_id": null, "ci_run_attempt": null}} -->

## 基本信息

- 测试文件：`tests/test_tun_tcp_buffer.rs`
- 测试对象：src/tun/tcp/buffer.rs
- 执行机制：unit
- 测试范围：本文件登记且在当前平台实际发现的全部用例
- 实际命令：`/private/tmp/fluxa-macos-team-storage/target/debug/deps/libfluxa_core-318794320ad432f5 ::test_tun_tcp_buffer:: --test-threads=1`
- 执行时间：2026-10-21T05:14:07.529593+08:00
- 实际耗时：0.046851999999987015 秒
- 运行环境：`{"os": "macOS-27.2-arm64-arm-64bit", "architecture": "arm64", "rust": "rustc 1.98.1 (48a229cea 2026-09-01)", "cargo": "cargo 1.98.1 (797e8a9bc 2026-08-05)", "commit": "ca03dbe91d30381600e401e82a2cffa3a5d38c87", "dirty": true, "features": "Cargo 默认 features；未启用 --all-features", "optional_environments": {"FLUXA_DEBUG_TEST_REQUIREMENT": false, "FLUXA_DEBUG_TEST_SWIFT": false, "FLUXA_PROTOCOL_PROFILE": false, "FLUXA_PROTOCOL_FIXTURE": false}, "ci_run_id": null, "ci_run_attempt": null}`
- 代码/测试/配置指纹：`95a77d0e822027d3d8d91dc72bd23cd1b197af94788923845e5c4d07153812fe`
- 总体状态：**PASS**
- 原始进程退出状态：0

## 实际用例与结果

| 实际发现的用例 | 独立目标 | 状态 |
| --- | --- | --- |
| `tun::tcp::buffer::test_tun_tcp_buffer::fixed_window_zero_and_open_after_consume` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `tun::tcp::buffer::test_tun_tcp_buffer::reordered_duplicate_overlap_wrap_and_finish` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `tun::tcp::buffer::test_tun_tcp_buffer::sack_bitmap_matches_independent_presence_model` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `tun::tcp::buffer::test_tun_tcp_buffer::sack_bitmap_word_and_ring_boundaries_after_consume` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `tun::tcp::buffer::test_tun_tcp_buffer::sack_blocks_merge_recent_first_and_survive_wrap` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `tun::tcp::buffer::test_tun_tcp_buffer::sack_never_reports_bytes_beyond_known_fin` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `tun::tcp::buffer::test_tun_tcp_buffer::scaled_receive_ring_is_bounded_and_reopens_after_consumption` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `tun::tcp::buffer::test_tun_tcp_buffer::scaled_sack_blocks_cross_old_capacity_and_ring_wrap` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `tun::tcp::buffer::test_tun_tcp_buffer::transmit_preserves_unacked_bytes_and_wraps` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |

## 执行汇总

- 实际发现：9
- 实际执行完成：9
- 通过：9
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

- 原始证据：`/private/tmp/fluxa-team-final-evidence/test_tun_tcp_buffer.json`、对应 `.log`（实际存在时）。
- CI 工件标识：`本地证据目录；未宣称已上传 CI`
- 最新报告被后续变更影响时必须重新执行；检查器会拒绝指纹不一致的旧结果。
- 未运行的 Swift/外部协议或夹具独立目标均保持 NOT RUN；本报告不代表系统 VPN、外部服务端及跨平台已经验收。
