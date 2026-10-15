# 测试报告：test_service_pipeline

<!-- fluxa-test-report:{"command": ["/private/tmp/fluxa-h1-session-lease/target/debug/deps/test_service_pipeline-a7cb47f171b368f4", "--test-threads=1"], "exit_code": 0, "duration_seconds": 0.2144677080000008, "status": "PASS", "discovered": ["cancellation_interrupts_tls_and_plain_requests_and_releases_resources", "configured_rejection_never_dials_and_reconfiguration_recovers", "idle_expiry_also_interrupts_managed_tls", "service_requires_explicit_route_and_dns", "unsupported_proxy_is_not_direct"], "results": {"cancellation_interrupts_tls_and_plain_requests_and_releases_resources": "PASS", "configured_rejection_never_dials_and_reconfiguration_recovers": "PASS", "idle_expiry_also_interrupts_managed_tls": "PASS", "service_requires_explicit_route_and_dns": "PASS", "unsupported_proxy_is_not_direct": "PASS"}, "fingerprint": "0fb0813b0660fe47d55383cf227eea471e18280c288066ca437365f92b9c1931", "time": "2026-10-15T18:16:48.068667+08:00", "attempt": 1, "suite": "test_service_pipeline", "file": "tests/test_service_pipeline.rs", "environment": {"os": "macOS-27.2-arm64-arm-64bit", "architecture": "arm64", "rust": "rustc 1.98.1 (48a229cea 2026-09-01)", "cargo": "cargo 1.98.1 (797e8a9bc 2026-08-05)", "commit": "3d210fafdf3363f5c6ebf5c901178219ef9c33c5", "dirty": true, "features": "Cargo 默认 features；未启用 --all-features", "optional_environments": {"FLUXA_DEBUG_TEST_REQUIREMENT": false, "FLUXA_DEBUG_TEST_SWIFT": false, "FLUXA_PROTOCOL_PROFILE": false, "FLUXA_PROTOCOL_FIXTURE": false}, "ci_run_id": null, "ci_run_attempt": null}} -->

## 基本信息

- 测试文件：`tests/test_service_pipeline.rs`
- 测试对象：src/http.rs、src/https.rs
- 执行机制：integration
- 测试范围：本文件登记且在当前平台实际发现的全部用例
- 实际命令：`/private/tmp/fluxa-h1-session-lease/target/debug/deps/test_service_pipeline-a7cb47f171b368f4 --test-threads=1`
- 执行时间：2026-10-15T18:16:48.068667+08:00
- 实际耗时：0.2144677080000008 秒
- 运行环境：`{"os": "macOS-27.2-arm64-arm-64bit", "architecture": "arm64", "rust": "rustc 1.98.1 (48a229cea 2026-09-01)", "cargo": "cargo 1.98.1 (797e8a9bc 2026-08-05)", "commit": "3d210fafdf3363f5c6ebf5c901178219ef9c33c5", "dirty": true, "features": "Cargo 默认 features；未启用 --all-features", "optional_environments": {"FLUXA_DEBUG_TEST_REQUIREMENT": false, "FLUXA_DEBUG_TEST_SWIFT": false, "FLUXA_PROTOCOL_PROFILE": false, "FLUXA_PROTOCOL_FIXTURE": false}, "ci_run_id": null, "ci_run_attempt": null}`
- 代码/测试/配置指纹：`0fb0813b0660fe47d55383cf227eea471e18280c288066ca437365f92b9c1931`
- 总体状态：**PASS**
- 原始进程退出状态：0

## 实际用例与结果

| 实际发现的用例 | 独立目标 | 状态 |
| --- | --- | --- |
| `cancellation_interrupts_tls_and_plain_requests_and_releases_resources` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `configured_rejection_never_dials_and_reconfiguration_recovers` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `idle_expiry_also_interrupts_managed_tls` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `service_requires_explicit_route_and_dns` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `unsupported_proxy_is_not_direct` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |

## 执行汇总

- 实际发现：5
- 实际执行完成：5
- 通过：5
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

- 原始证据：`/private/tmp/fluxa-swift-split-core-evidence/test_service_pipeline.json`、对应 `.log`（实际存在时）。
- CI 工件标识：`本地证据目录；未宣称已上传 CI`
- 最新报告被后续变更影响时必须重新执行；检查器会拒绝指纹不一致的旧结果。
- 未运行的 Swift/外部协议或夹具独立目标均保持 NOT RUN；本报告不代表系统 VPN、外部服务端及跨平台已经验收。
