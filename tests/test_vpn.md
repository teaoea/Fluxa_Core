# 测试报告：test_vpn

<!-- nodus-test-report:{"command": ["/Users/klee/Documents/Nodus Core/target/debug/deps/nodus_core-041096b04c02ab5d", "::test_vpn::", "--test-threads=1"], "exit_code": 0, "duration_seconds": 0.3385254999999958, "status": "PASS", "discovered": ["vpn::test_vpn::close_releases_slot_with_old_reference_alive", "vpn::test_vpn::failure_of_second_source_never_publishes_partial_candidate", "vpn::test_vpn::geoip_failure_is_error_not_disabled_success", "vpn::test_vpn::host_sources_are_read_only_and_keep_policy", "vpn::test_vpn::invalid_input_and_unsupported_modes_fail_without_partial_candidate", "vpn::test_vpn::local_candidate_is_not_activation_and_does_not_replace_live_rules", "vpn::test_vpn::nonexistent_default_node_fails_without_candidate_or_direct_fallback", "vpn::test_vpn::slots_are_bounded_before_background_work", "vpn::test_vpn::total_deadline_rejects_invalid_duration_before_work"], "results": {"vpn::test_vpn::close_releases_slot_with_old_reference_alive": "PASS", "vpn::test_vpn::failure_of_second_source_never_publishes_partial_candidate": "PASS", "vpn::test_vpn::geoip_failure_is_error_not_disabled_success": "PASS", "vpn::test_vpn::host_sources_are_read_only_and_keep_policy": "PASS", "vpn::test_vpn::invalid_input_and_unsupported_modes_fail_without_partial_candidate": "PASS", "vpn::test_vpn::local_candidate_is_not_activation_and_does_not_replace_live_rules": "PASS", "vpn::test_vpn::nonexistent_default_node_fails_without_candidate_or_direct_fallback": "PASS", "vpn::test_vpn::slots_are_bounded_before_background_work": "PASS", "vpn::test_vpn::total_deadline_rejects_invalid_duration_before_work": "PASS"}, "fingerprint": "58ff0b1f3f50eff7c22be04bcf8a416d3011f789935ef2364ba1d069e2eb3746", "time": "2026-10-23T22:13:42.019403+08:00", "attempt": 2, "suite": "test_vpn", "file": "tests/test_vpn.rs", "environment": {"os": "macOS-27.2-arm64-arm-64bit", "architecture": "arm64", "rust": "rustc 1.98.1 (48a229cea 2026-09-01)", "cargo": "cargo 1.98.1 (797e8a9bc 2026-08-05)", "commit": "d773a32585e840c5ea3e78183d08d1a99d20059a", "dirty": true, "features": "Cargo 默认 features；未启用 --all-features", "optional_environments": {"NODUS_DEBUG_TEST_REQUIREMENT": false, "NODUS_DEBUG_TEST_SWIFT": false, "NODUS_PROTOCOL_PROFILE": false, "NODUS_PROTOCOL_FIXTURE": false}, "ci_run_id": null, "ci_run_attempt": null}} -->

## 基本信息

- 测试文件：`tests/test_vpn.rs`
- 测试对象：src/vpn.rs、src/providers.rs、src/configuration.rs、src/outbound/configuration.rs
- 执行机制：unit
- 测试范围：本文件登记且在当前平台实际发现的全部用例
- 实际命令：`/Users/klee/Documents/Nodus Core/target/debug/deps/nodus_core-041096b04c02ab5d ::test_vpn:: --test-threads=1`
- 执行时间：2026-10-23T22:13:42.019403+08:00
- 实际耗时：0.3385254999999958 秒
- 运行环境：`{"os": "macOS-27.2-arm64-arm-64bit", "architecture": "arm64", "rust": "rustc 1.98.1 (48a229cea 2026-09-01)", "cargo": "cargo 1.98.1 (797e8a9bc 2026-08-05)", "commit": "d773a32585e840c5ea3e78183d08d1a99d20059a", "dirty": true, "features": "Cargo 默认 features；未启用 --all-features", "optional_environments": {"NODUS_DEBUG_TEST_REQUIREMENT": false, "NODUS_DEBUG_TEST_SWIFT": false, "NODUS_PROTOCOL_PROFILE": false, "NODUS_PROTOCOL_FIXTURE": false}, "ci_run_id": null, "ci_run_attempt": null}`
- 代码/测试/配置指纹：`58ff0b1f3f50eff7c22be04bcf8a416d3011f789935ef2364ba1d069e2eb3746`
- 总体状态：**PASS**
- 原始进程退出状态：0

## 实际用例与结果

| 实际发现的用例 | 独立目标 | 状态 |
| --- | --- | --- |
| `vpn::test_vpn::close_releases_slot_with_old_reference_alive` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `vpn::test_vpn::failure_of_second_source_never_publishes_partial_candidate` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `vpn::test_vpn::geoip_failure_is_error_not_disabled_success` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `vpn::test_vpn::host_sources_are_read_only_and_keep_policy` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `vpn::test_vpn::invalid_input_and_unsupported_modes_fail_without_partial_candidate` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `vpn::test_vpn::local_candidate_is_not_activation_and_does_not_replace_live_rules` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `vpn::test_vpn::nonexistent_default_node_fails_without_candidate_or_direct_fallback` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `vpn::test_vpn::slots_are_bounded_before_background_work` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `vpn::test_vpn::total_deadline_rejects_invalid_duration_before_work` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |

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

- 原始证据：`/private/tmp/nodus-real-preflight-final/test_vpn.json`、对应 `.log`（实际存在时）。
- CI 工件标识：`本地证据目录；未宣称已上传 CI`
- 最新报告被后续变更影响时必须重新执行；检查器会拒绝指纹不一致的旧结果。
- 未运行的 Swift/外部协议或夹具独立目标均保持 NOT RUN；本报告不代表系统 VPN、外部服务端及跨平台已经验收。

<!-- nodus-test-documentation:begin -->
## 测试目的、目标与本轮范围

目的：确保新增四阶段事务驱动不破坏既有隔离候选准备及关闭契约。
目标：候选不发布活动配置或授予接管权；非法配置、只读资源错误和准备失败保留真实错误；取消及重复关闭确定释放候选槽，旧对象不恢复活动权限。
方法：本套件原有9项断言完整执行，与新事务的10项分开归属；原始命令、结果、版本和指纹见上方实际报告。只做隔离候选及本地夹具验证，不接管系统 VPN。性能测试不适用，不以普通执行耗时作为性能验收。
<!-- nodus-test-documentation:end -->
