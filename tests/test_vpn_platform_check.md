# 测试报告：test_vpn_platform_check

<!-- nodus-test-report:{"command": ["/Users/klee/Documents/Nodus Core/target/debug/deps/nodus_core-915003f7fa5ba4b5", "::test_vpn_platform_check::", "--test-threads=1"], "exit_code": 0, "duration_seconds": 0.012483541000008813, "status": "PASS", "discovered": ["vpn::platform_check::test_vpn_platform_check::absent_adapter_never_passes", "vpn::platform_check::test_vpn_platform_check::consumed_cancelled_expired_requests_fail_closed", "vpn::platform_check::test_vpn_platform_check::egress_coverage_requires_exact_protected_routes", "vpn::platform_check::test_vpn_platform_check::foreign_request_platform_and_time_are_rejected", "vpn::platform_check::test_vpn_platform_check::inputs_are_bounded_before_copy_and_challenges_differ", "vpn::platform_check::test_vpn_platform_check::matching_observations_only_validate_platform_subset", "vpn::platform_check::test_vpn_platform_check::permission_and_required_capabilities_are_distinct", "vpn::platform_check::test_vpn_platform_check::revoked_or_wrong_source_identity_cannot_authorize", "vpn::platform_check::test_vpn_platform_check::serialized_assessment_cannot_leak_or_restore_proof", "vpn::platform_check::test_vpn_platform_check::unresolved_and_empty_egress_are_not_equivalent"], "results": {"vpn::platform_check::test_vpn_platform_check::absent_adapter_never_passes": "PASS", "vpn::platform_check::test_vpn_platform_check::consumed_cancelled_expired_requests_fail_closed": "PASS", "vpn::platform_check::test_vpn_platform_check::egress_coverage_requires_exact_protected_routes": "PASS", "vpn::platform_check::test_vpn_platform_check::foreign_request_platform_and_time_are_rejected": "PASS", "vpn::platform_check::test_vpn_platform_check::inputs_are_bounded_before_copy_and_challenges_differ": "PASS", "vpn::platform_check::test_vpn_platform_check::matching_observations_only_validate_platform_subset": "PASS", "vpn::platform_check::test_vpn_platform_check::permission_and_required_capabilities_are_distinct": "PASS", "vpn::platform_check::test_vpn_platform_check::revoked_or_wrong_source_identity_cannot_authorize": "PASS", "vpn::platform_check::test_vpn_platform_check::serialized_assessment_cannot_leak_or_restore_proof": "PASS", "vpn::platform_check::test_vpn_platform_check::unresolved_and_empty_egress_are_not_equivalent": "PASS"}, "fingerprint": "6570db01cac088692214174f3c064ba92f94de504c7ddbad674b6c9a5028d243", "time": "2026-10-24T13:41:42.348500+08:00", "attempt": 1, "suite": "test_vpn_platform_check", "file": "tests/test_vpn_platform_check.rs", "environment": {"os": "macOS-27.2-arm64-arm-64bit", "architecture": "arm64", "rust": "rustc 1.98.1 (48a229cea 2026-09-01)", "cargo": "cargo 1.98.1 (797e8a9bc 2026-08-05)", "commit": "2f6cd1e624851636840713246da2f51fa05c86fb", "dirty": true, "features": "Cargo 默认 features；未启用 --all-features", "optional_environments": {"NODUS_DEBUG_TEST_REQUIREMENT": false, "NODUS_DEBUG_TEST_SWIFT": false, "NODUS_PROTOCOL_PROFILE": false, "NODUS_PROTOCOL_FIXTURE": false}, "ci_run_id": null, "ci_run_attempt": null}} -->

## 基本信息

- 测试文件：`tests/test_vpn_platform_check.rs`
- 测试对象：src/vpn/platform_check.rs
- 执行机制：unit
- 测试范围：本文件登记且在当前平台实际发现的全部用例
- 实际命令：`/Users/klee/Documents/Nodus Core/target/debug/deps/nodus_core-915003f7fa5ba4b5 ::test_vpn_platform_check:: --test-threads=1`
- 执行时间：2026-10-24T13:41:42.348500+08:00
- 实际耗时：0.012483541000008813 秒
- 运行环境：`{"os": "macOS-27.2-arm64-arm-64bit", "architecture": "arm64", "rust": "rustc 1.98.1 (48a229cea 2026-09-01)", "cargo": "cargo 1.98.1 (797e8a9bc 2026-08-05)", "commit": "2f6cd1e624851636840713246da2f51fa05c86fb", "dirty": true, "features": "Cargo 默认 features；未启用 --all-features", "optional_environments": {"NODUS_DEBUG_TEST_REQUIREMENT": false, "NODUS_DEBUG_TEST_SWIFT": false, "NODUS_PROTOCOL_PROFILE": false, "NODUS_PROTOCOL_FIXTURE": false}, "ci_run_id": null, "ci_run_attempt": null}`
- 代码/测试/配置指纹：`6570db01cac088692214174f3c064ba92f94de504c7ddbad674b6c9a5028d243`
- 总体状态：**PASS**
- 原始进程退出状态：0

## 实际用例与结果

| 实际发现的用例 | 独立目标 | 状态 |
| --- | --- | --- |
| `vpn::platform_check::test_vpn_platform_check::absent_adapter_never_passes` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `vpn::platform_check::test_vpn_platform_check::consumed_cancelled_expired_requests_fail_closed` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `vpn::platform_check::test_vpn_platform_check::egress_coverage_requires_exact_protected_routes` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `vpn::platform_check::test_vpn_platform_check::foreign_request_platform_and_time_are_rejected` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `vpn::platform_check::test_vpn_platform_check::inputs_are_bounded_before_copy_and_challenges_differ` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `vpn::platform_check::test_vpn_platform_check::matching_observations_only_validate_platform_subset` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `vpn::platform_check::test_vpn_platform_check::permission_and_required_capabilities_are_distinct` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `vpn::platform_check::test_vpn_platform_check::revoked_or_wrong_source_identity_cannot_authorize` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `vpn::platform_check::test_vpn_platform_check::serialized_assessment_cannot_leak_or_restore_proof` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `vpn::platform_check::test_vpn_platform_check::unresolved_and_empty_egress_are_not_equivalent` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |

## 执行汇总

- 实际发现：10
- 实际执行完成：10
- 通过：10
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

- 原始证据：`/private/tmp/claude-501/-Users-klee-Documents-Fluxa/dd1504c4-f781-4692-b482-4e9c216c95e1/scratchpad/core-evidence/test_vpn_platform_check.json`、对应 `.log`（实际存在时）。
- CI 工件标识：`本地证据目录；未宣称已上传 CI`
- 最新报告被后续变更影响时必须重新执行；检查器会拒绝指纹不一致的旧结果。
- 未运行的 Swift/外部协议或夹具独立目标均保持 NOT RUN；本报告不代表系统 VPN、外部服务端及跨平台已经验收。

<!-- nodus-test-documentation:begin -->
## 测试目的

防止网络成功、外部成功标志或陈旧/不匹配的平台观察被当成启动资格；验证纯Core契约，明确与真实OS接线的边界。

## 验收目标与方法

在固定单调时刻，以仅子模块可构造的证据替身调用真实Request校验。已知有效输入三项通过；逐项改变事务、挑战、平台、观测时间、来源代次/撤销、权限、IP包与地址族/DNS能力、出站完整性时，必须得到预先指定的拒绝原因，且后续未执行项不伪造Passed。30秒边界、取消和单次消费明确拒绝。未知出站集合不等于已知空集合；已知空集合仍需受信保护范围。序列化严格只有6个诊断字段且无地址、挑战、身份或激活标志。

## 不涵盖范围与性能

不验证真实OS权限、签名/能力通道身份、路由、系统接管、进程恢复、最低系统或其他平台；替身成功不是生产证据。性能测试不适用，未测RSS、FD、吞吐或完整启动峰值。通用证据与配置/网络变化失效仍待后续任务。

## 本轮补充核对

L1：Rust2024格式、差异空白检查及严格Clippy通过。L2：最终47项回归通过，错误生成检查44项一致。L3：依照启动规范核对缺证据拒绝、取消、消费、未知与空出站集合差异，未把Authority生命周期验证或替身成功当作OS认证。L4：最终日志及历史证据见本报告记录的证据目录；Clippy日志为 /private/tmp/nodus-platform-contract-20261024-clippy-classified.log，错误生成最终日志为 /private/tmp/nodus-platform-contract-20261024-api-check-final.log。工件arm64/minos13.0，不代表最低系统验收；35个API相关本地链接无失效。最初格式命令遗漏edition及模板旧链接检查失败已分别纠正，最终重新通过。真实平台边界及性能未验收。
<!-- nodus-test-documentation:end -->
