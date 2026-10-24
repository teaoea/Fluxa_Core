# 测试报告：test_routing_strategy

<!-- nodus-test-report:{"command": ["/Users/klee/Documents/Nodus Core/target/debug/deps/nodus_core-915003f7fa5ba4b5", "::test_routing_strategy::", "--test-threads=1"], "exit_code": 0, "duration_seconds": 0.009370708000005834, "status": "PASS", "discovered": ["routing::strategy::test_routing_strategy::invalid_or_unbounded_lists_fail", "routing::strategy::test_routing_strategy::ordered_unicode_and_explicit_terminals", "routing::strategy::test_routing_strategy::single_proxy_name_is_preserved"], "results": {"routing::strategy::test_routing_strategy::invalid_or_unbounded_lists_fail": "PASS", "routing::strategy::test_routing_strategy::ordered_unicode_and_explicit_terminals": "PASS", "routing::strategy::test_routing_strategy::single_proxy_name_is_preserved": "PASS"}, "fingerprint": "6570db01cac088692214174f3c064ba92f94de504c7ddbad674b6c9a5028d243", "time": "2026-10-24T13:41:42.348500+08:00", "attempt": 1, "suite": "test_routing_strategy", "file": "tests/test_routing_strategy.rs", "environment": {"os": "macOS-27.2-arm64-arm-64bit", "architecture": "arm64", "rust": "rustc 1.98.1 (48a229cea 2026-09-01)", "cargo": "cargo 1.98.1 (797e8a9bc 2026-08-05)", "commit": "2f6cd1e624851636840713246da2f51fa05c86fb", "dirty": true, "features": "Cargo 默认 features；未启用 --all-features", "optional_environments": {"NODUS_DEBUG_TEST_REQUIREMENT": false, "NODUS_DEBUG_TEST_SWIFT": false, "NODUS_PROTOCOL_PROFILE": false, "NODUS_PROTOCOL_FIXTURE": false}, "ci_run_id": null, "ci_run_attempt": null}} -->

## 基本信息

- 测试文件：`tests/test_routing_strategy.rs`
- 测试对象：src/routing/strategy.rs
- 执行机制：unit
- 测试范围：本文件登记且在当前平台实际发现的全部用例
- 实际命令：`/Users/klee/Documents/Nodus Core/target/debug/deps/nodus_core-915003f7fa5ba4b5 ::test_routing_strategy:: --test-threads=1`
- 执行时间：2026-10-24T13:41:42.348500+08:00
- 实际耗时：0.009370708000005834 秒
- 运行环境：`{"os": "macOS-27.2-arm64-arm-64bit", "architecture": "arm64", "rust": "rustc 1.98.1 (48a229cea 2026-09-01)", "cargo": "cargo 1.98.1 (797e8a9bc 2026-08-05)", "commit": "2f6cd1e624851636840713246da2f51fa05c86fb", "dirty": true, "features": "Cargo 默认 features；未启用 --all-features", "optional_environments": {"NODUS_DEBUG_TEST_REQUIREMENT": false, "NODUS_DEBUG_TEST_SWIFT": false, "NODUS_PROTOCOL_PROFILE": false, "NODUS_PROTOCOL_FIXTURE": false}, "ci_run_id": null, "ci_run_attempt": null}`
- 代码/测试/配置指纹：`6570db01cac088692214174f3c064ba92f94de504c7ddbad674b6c9a5028d243`
- 总体状态：**PASS**
- 原始进程退出状态：0

## 实际用例与结果

| 实际发现的用例 | 独立目标 | 状态 |
| --- | --- | --- |
| `routing::strategy::test_routing_strategy::invalid_or_unbounded_lists_fail` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `routing::strategy::test_routing_strategy::ordered_unicode_and_explicit_terminals` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `routing::strategy::test_routing_strategy::single_proxy_name_is_preserved` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |

## 执行汇总

- 实际发现：3
- 实际执行完成：3
- 通过：3
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

- 原始证据：`/private/tmp/claude-501/-Users-klee-Documents-Fluxa/dd1504c4-f781-4692-b482-4e9c216c95e1/scratchpad/core-evidence/test_routing_strategy.json`、对应 `.log`（实际存在时）。
- CI 工件标识：`本地证据目录；未宣称已上传 CI`
- 最新报告被后续变更影响时必须重新执行；检查器会拒绝指纹不一致的旧结果。
- 未运行的 Swift/外部协议或夹具独立目标均保持 NOT RUN；本报告不代表系统 VPN、外部服务端及跨平台已经验收。

<!-- nodus-test-documentation:begin -->
## 目的、目标与方法

防止有序策略被当成单一节点、隐式直连或无界列表。以用户的节点顺序及明确direct/reject语义为依据，直接解析固定输入并核对顺序、规范表达式和必要节点集合；空项、空白、重复、括号嵌套、reject后的不可达项和8项/128字节边界必须明确拒绝。新模块仅借用字符串、固定数组解析，不进行网络访问。性能测试不适用。

## 证据边界

最终15套129项定向测试及严格Clippy通过。实际TCP/UDP回环节点建连失败、超时、显式直连、拒绝、取消与不重放验证见test_outbound_execution同名报告。真实外部节点到节点成功切换、最低系统及系统VPN未测。首轮编译的借用/断言类型错误与后续注释格式警告已修正；最终原始日志为/private/tmp/nodus-mixed-policy-delivery-tests.log与/private/tmp/nodus-mixed-policy-delivery-clippy.log，历史尝试由同一证据目录保留。
<!-- nodus-test-documentation:end -->
