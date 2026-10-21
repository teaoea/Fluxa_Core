# 测试报告：test_error

<!-- fluxa-test-report:{"command": ["/Users/klee/Documents/FLuxa_Core/target/debug/deps/fluxa_core-87939ed5fdf85bf0", "::test_error::", "--test-threads=1"], "exit_code": 0, "duration_seconds": 0.007342999999999877, "status": "PASS", "discovered": ["error::test_error::tls_mapping_tests::tls_keeps_budget_state_protocol_and_cancellation_causes"], "results": {"error::test_error::tls_mapping_tests::tls_keeps_budget_state_protocol_and_cancellation_causes": "PASS"}, "fingerprint": "23e34507a613c48170087b90501299e7bca9fe06332f55ac09e5a298857e3243", "time": "2026-10-21T19:59:51.359405+08:00", "attempt": 1, "suite": "test_error", "file": "tests/test_error.rs", "environment": {"os": "macOS-27.2-arm64-arm-64bit", "architecture": "arm64", "rust": "rustc 1.98.1 (48a229cea 2026-09-01)", "cargo": "cargo 1.98.1 (797e8a9bc 2026-08-05)", "commit": "1a798addfabd84db3e26bdccf2794318eba7e1ce", "dirty": true, "features": "Cargo 默认 features；未启用 --all-features", "optional_environments": {"FLUXA_DEBUG_TEST_REQUIREMENT": false, "FLUXA_DEBUG_TEST_SWIFT": false, "FLUXA_PROTOCOL_PROFILE": false, "FLUXA_PROTOCOL_FIXTURE": false}, "ci_run_id": null, "ci_run_attempt": null}} -->

## 基本信息

- 测试文件：`tests/test_error.rs`
- 测试对象：src/error.rs
- 执行机制：unit
- 测试范围：本文件登记且在当前平台实际发现的全部用例
- 实际命令：`/Users/klee/Documents/FLuxa_Core/target/debug/deps/fluxa_core-87939ed5fdf85bf0 ::test_error:: --test-threads=1`
- 执行时间：2026-10-21T19:59:51.359405+08:00
- 实际耗时：0.007342999999999877 秒
- 运行环境：`{"os": "macOS-27.2-arm64-arm-64bit", "architecture": "arm64", "rust": "rustc 1.98.1 (48a229cea 2026-09-01)", "cargo": "cargo 1.98.1 (797e8a9bc 2026-08-05)", "commit": "1a798addfabd84db3e26bdccf2794318eba7e1ce", "dirty": true, "features": "Cargo 默认 features；未启用 --all-features", "optional_environments": {"FLUXA_DEBUG_TEST_REQUIREMENT": false, "FLUXA_DEBUG_TEST_SWIFT": false, "FLUXA_PROTOCOL_PROFILE": false, "FLUXA_PROTOCOL_FIXTURE": false}, "ci_run_id": null, "ci_run_attempt": null}`
- 代码/测试/配置指纹：`23e34507a613c48170087b90501299e7bca9fe06332f55ac09e5a298857e3243`
- 总体状态：**PASS**
- 原始进程退出状态：0

## 实际用例与结果

| 实际发现的用例 | 独立目标 | 状态 |
| --- | --- | --- |
| `error::test_error::tls_mapping_tests::tls_keeps_budget_state_protocol_and_cancellation_causes` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |

## 执行汇总

- 实际发现：1
- 实际执行完成：1
- 通过：1
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

- 原始证据：`/private/tmp/fluxa-readonly-main-policy-final/test_error.json`、对应 `.log`（实际存在时）。
- CI 工件标识：`本地证据目录；未宣称已上传 CI`
- 最新报告被后续变更影响时必须重新执行；检查器会拒绝指纹不一致的旧结果。
- 未运行的 Swift/外部协议或夹具独立目标均保持 NOT RUN；本报告不代表系统 VPN、外部服务端及跨平台已经验收。
