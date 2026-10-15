# 测试报告：test_routing_flat

<!-- fluxa-test-report:{"command": ["/private/tmp/fluxa-h1-session-lease/target/debug/deps/libfluxa_core-84ce2e4fcc560243", "::test_routing_flat::", "--test-threads=1"], "exit_code": 101, "duration_seconds": 0.49293258400000184, "status": "FAIL", "discovered": ["routing::runtime::flat::test_routing_flat::public_contract::control_rewrite_with_recomputed_digest_still_loads", "routing::runtime::flat::test_routing_flat::public_contract::scenario_01_sixty_five_domains_in_one_slot", "routing::runtime::flat::test_routing_flat::public_contract::scenario_02_ipv4_zero_prefix_must_be_normalized", "routing::runtime::flat::test_routing_flat::public_contract::scenario_03_ipv6_zero_prefix_must_be_normalized", "routing::runtime::flat::test_routing_flat::public_contract::scenario_04_child_prefix_must_belong_to_parent", "routing::runtime::flat::test_routing_flat::public_contract::scenario_05_child_must_be_on_the_right_branch", "routing::runtime::flat::test_routing_flat::public_contract::scenario_06_keyword_failures_return_every_candidate", "routing::runtime::flat::test_routing_flat::public_contract::scenario_07_tampered_keyword_failure_transition_is_rejected", "routing::runtime::flat::test_routing_flat::public_contract::scenario_08_candidate_must_correspond_to_its_node", "routing::runtime::flat::test_routing_flat::public_contract::scenario_09_same_class_candidate_order_is_preserved", "routing::runtime::flat::test_routing_flat::public_contract::scenario_10_domain_and_ip_priority", "routing::runtime::flat::test_routing_flat::public_contract::scenario_11_build_peak_stays_within_budget", "routing::runtime::flat::test_routing_flat::public_contract::scenario_12_retained_snapshot_is_still_counted", "routing::runtime::flat::test_routing_flat::public_contract::scenario_13_incompatible_format_version_is_reported", "routing::runtime::flat::test_routing_flat::public_contract::scenario_14_domain_normalization_never_misses"], "results": {"routing::runtime::flat::test_routing_flat::public_contract::control_rewrite_with_recomputed_digest_still_loads": "PASS", "routing::runtime::flat::test_routing_flat::public_contract::scenario_01_sixty_five_domains_in_one_slot": "FAIL", "routing::runtime::flat::test_routing_flat::public_contract::scenario_02_ipv4_zero_prefix_must_be_normalized": "FAIL", "routing::runtime::flat::test_routing_flat::public_contract::scenario_03_ipv6_zero_prefix_must_be_normalized": "FAIL", "routing::runtime::flat::test_routing_flat::public_contract::scenario_04_child_prefix_must_belong_to_parent": "FAIL", "routing::runtime::flat::test_routing_flat::public_contract::scenario_05_child_must_be_on_the_right_branch": "FAIL", "routing::runtime::flat::test_routing_flat::public_contract::scenario_06_keyword_failures_return_every_candidate": "PASS", "routing::runtime::flat::test_routing_flat::public_contract::scenario_07_tampered_keyword_failure_transition_is_rejected": "PASS", "routing::runtime::flat::test_routing_flat::public_contract::scenario_08_candidate_must_correspond_to_its_node": "FAIL", "routing::runtime::flat::test_routing_flat::public_contract::scenario_09_same_class_candidate_order_is_preserved": "PASS", "routing::runtime::flat::test_routing_flat::public_contract::scenario_10_domain_and_ip_priority": "PASS", "routing::runtime::flat::test_routing_flat::public_contract::scenario_11_build_peak_stays_within_budget": "PASS", "routing::runtime::flat::test_routing_flat::public_contract::scenario_12_retained_snapshot_is_still_counted": "PASS", "routing::runtime::flat::test_routing_flat::public_contract::scenario_13_incompatible_format_version_is_reported": "PASS", "routing::runtime::flat::test_routing_flat::public_contract::scenario_14_domain_normalization_never_misses": "PASS"}, "fingerprint": "0fb0813b0660fe47d55383cf227eea471e18280c288066ca437365f92b9c1931", "time": "2026-10-15T18:16:48.068667+08:00", "attempt": 1, "suite": "test_routing_flat", "file": "tests/test_routing_flat.rs", "environment": {"os": "macOS-27.2-arm64-arm-64bit", "architecture": "arm64", "rust": "rustc 1.98.1 (48a229cea 2026-09-01)", "cargo": "cargo 1.98.1 (797e8a9bc 2026-08-05)", "commit": "3d210fafdf3363f5c6ebf5c901178219ef9c33c5", "dirty": true, "features": "Cargo 默认 features；未启用 --all-features", "optional_environments": {"FLUXA_DEBUG_TEST_REQUIREMENT": false, "FLUXA_DEBUG_TEST_SWIFT": false, "FLUXA_PROTOCOL_PROFILE": false, "FLUXA_PROTOCOL_FIXTURE": false}, "ci_run_id": null, "ci_run_attempt": null}} -->

## 基本信息

- 测试文件：`tests/test_routing_flat.rs`
- 测试对象：src/routing/flat.rs
- 执行机制：unit
- 测试范围：本文件登记且在当前平台实际发现的全部用例
- 实际命令：`/private/tmp/fluxa-h1-session-lease/target/debug/deps/libfluxa_core-84ce2e4fcc560243 ::test_routing_flat:: --test-threads=1`
- 执行时间：2026-10-15T18:16:48.068667+08:00
- 实际耗时：0.49293258400000184 秒
- 运行环境：`{"os": "macOS-27.2-arm64-arm-64bit", "architecture": "arm64", "rust": "rustc 1.98.1 (48a229cea 2026-09-01)", "cargo": "cargo 1.98.1 (797e8a9bc 2026-08-05)", "commit": "3d210fafdf3363f5c6ebf5c901178219ef9c33c5", "dirty": true, "features": "Cargo 默认 features；未启用 --all-features", "optional_environments": {"FLUXA_DEBUG_TEST_REQUIREMENT": false, "FLUXA_DEBUG_TEST_SWIFT": false, "FLUXA_PROTOCOL_PROFILE": false, "FLUXA_PROTOCOL_FIXTURE": false}, "ci_run_id": null, "ci_run_attempt": null}`
- 代码/测试/配置指纹：`0fb0813b0660fe47d55383cf227eea471e18280c288066ca437365f92b9c1931`
- 总体状态：**FAIL**
- 原始进程退出状态：101

## 实际用例与结果

| 实际发现的用例 | 独立目标 | 状态 |
| --- | --- | --- |
| `routing::runtime::flat::test_routing_flat::public_contract::control_rewrite_with_recomputed_digest_still_loads` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `routing::runtime::flat::test_routing_flat::public_contract::scenario_01_sixty_five_domains_in_one_slot` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | FAIL |
| `routing::runtime::flat::test_routing_flat::public_contract::scenario_02_ipv4_zero_prefix_must_be_normalized` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | FAIL |
| `routing::runtime::flat::test_routing_flat::public_contract::scenario_03_ipv6_zero_prefix_must_be_normalized` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | FAIL |
| `routing::runtime::flat::test_routing_flat::public_contract::scenario_04_child_prefix_must_belong_to_parent` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | FAIL |
| `routing::runtime::flat::test_routing_flat::public_contract::scenario_05_child_must_be_on_the_right_branch` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | FAIL |
| `routing::runtime::flat::test_routing_flat::public_contract::scenario_06_keyword_failures_return_every_candidate` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `routing::runtime::flat::test_routing_flat::public_contract::scenario_07_tampered_keyword_failure_transition_is_rejected` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `routing::runtime::flat::test_routing_flat::public_contract::scenario_08_candidate_must_correspond_to_its_node` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | FAIL |
| `routing::runtime::flat::test_routing_flat::public_contract::scenario_09_same_class_candidate_order_is_preserved` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `routing::runtime::flat::test_routing_flat::public_contract::scenario_10_domain_and_ip_priority` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `routing::runtime::flat::test_routing_flat::public_contract::scenario_11_build_peak_stays_within_budget` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `routing::runtime::flat::test_routing_flat::public_contract::scenario_12_retained_snapshot_is_still_counted` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `routing::runtime::flat::test_routing_flat::public_contract::scenario_13_incompatible_format_version_is_reported` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `routing::runtime::flat::test_routing_flat::public_contract::scenario_14_domain_normalization_never_misses` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |

## 执行汇总

- 实际发现：15
- 实际执行完成：15
- 通过：9
- 失败：6
- 忽略/跳过：0
- 未取得完整执行结果：0

## 失败、异常与限制

本次观察到断言失败；实际错误及期望/实际输出见下方和原始日志。
未覆盖范围以本文件实际断言和前置条件为界，不宣称全模块或全平台覆盖。
不自动重试；若手动重跑，旧报告及原始退出结果保存在证据目录。缺少执行结果时不推定成功。

## 独立 L1–L4 审查

- L1：本次测试构建结果见 build 日志；格式/Clippy 等需另行记录，不自动推断通过。
- L2：已登记用例按 Cargo 工件、发现列表与显式文件映射归属；实际结果见上表。
- L3：未由报告生成器验证；迁移保留断言不等于原业务预期已全面证明，需独立语义审查。
- L4：原始日志、机器结果、版本及指纹可在下述证据核对；未上传的本地证据不冒充 CI 工件。

## 证据及后续事项

- 原始证据：`/private/tmp/fluxa-swift-split-core-evidence/test_routing_flat.json`、对应 `.log`（实际存在时）。
- CI 工件标识：`本地证据目录；未宣称已上传 CI`
- 最新报告被后续变更影响时必须重新执行；检查器会拒绝指纹不一致的旧结果。
- 未运行的 Swift/外部协议或夹具独立目标均保持 NOT RUN；本报告不代表系统 VPN、外部服务端及跨平台已经验收。

## 异常原始输出节选

最小重现：`python3 Tools/test_reports.py --run --suite test_routing_flat`（Rust 套件）；外部套件使用上方真实命令。

```text

running 15 tests
test routing::runtime::flat::test_routing_flat::public_contract::control_rewrite_with_recomputed_digest_still_loads ... ok
test routing::runtime::flat::test_routing_flat::public_contract::scenario_01_sixty_five_domains_in_one_slot ... FAILED
test routing::runtime::flat::test_routing_flat::public_contract::scenario_02_ipv4_zero_prefix_must_be_normalized ... FAILED
test routing::runtime::flat::test_routing_flat::public_contract::scenario_03_ipv6_zero_prefix_must_be_normalized ... FAILED
test routing::runtime::flat::test_routing_flat::public_contract::scenario_04_child_prefix_must_belong_to_parent ... FAILED
test routing::runtime::flat::test_routing_flat::public_contract::scenario_05_child_must_be_on_the_right_branch ... FAILED
test routing::runtime::flat::test_routing_flat::public_contract::scenario_06_keyword_failures_return_every_candidate ... ok
test routing::runtime::flat::test_routing_flat::public_contract::scenario_07_tampered_keyword_failure_transition_is_rejected ... ok
test routing::runtime::flat::test_routing_flat::public_contract::scenario_08_candidate_must_correspond_to_its_node ... FAILED
test routing::runtime::flat::test_routing_flat::public_contract::scenario_09_same_class_candidate_order_is_preserved ... ok
test routing::runtime::flat::test_routing_flat::public_contract::scenario_10_domain_and_ip_priority ... ok
test routing::runtime::flat::test_routing_flat::public_contract::scenario_11_build_peak_stays_within_budget ... ok
test routing::runtime::flat::test_routing_flat::public_contract::scenario_12_retained_snapshot_is_still_counted ... ok
test routing::runtime::flat::test_routing_flat::public_contract::scenario_13_incompatible_format_version_is_reported ... ok
test routing::runtime::flat::test_routing_flat::public_contract::scenario_14_domain_normalization_never_misses ... ok

failures:

---- routing::runtime::flat::test_routing_flat::public_contract::scenario_01_sixty_five_domains_in_one_slot stdout ----

thread 'routing::runtime::flat::test_routing_flat::public_contract::scenario_01_sixty_five_domains_in_one_slot' (4153423) panicked at src/routing/../../tests/test_routing_flat.rs:360:9:
65 个同槽合法域名被误报为资源耗尽（探测链上限，非真实内存不足）
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

---- routing::runtime::flat::test_routing_flat::public_contract::scenario_02_ipv4_zero_prefix_must_be_normalized stdout ----

thread 'routing::runtime::flat::test_routing_flat::public_contract::scenario_02_ipv4_zero_prefix_must_be_normalized' (4153424) panicked at src/routing/../../tests/test_routing_flat.rs:390:9:
非规范化的 /0 前缀必须被拒绝，实际：Ok(())

---- routing::runtime::flat::test_routing_flat::public_contract::scenario_03_ipv6_zero_prefix_must_be_normalized stdout ----

thread 'routing::runtime::flat::test_routing_flat::public_contract::scenario_03_ipv6_zero_prefix_must_be_normalized' (4153435) panicked at src/routing/../../tests/test_routing_flat.rs:390:9:
非规范化的 /0 前缀必须被拒绝，实际：Ok(())

---- routing::runtime::flat::test_routing_flat::public_contract::scenario_04_child_prefix_must_belong_to_parent stdout ----

thread 'routing::runtime::flat::test_routing_flat::public_contract::scenario_04_child_prefix_must_belong_to_parent' (4153446) panicked at src/routing/../../tests/test_routing_flat.rs:448:9:
子节点前缀不属于父节点时必须被拒绝，实际：Ok(())

---- routing::runtime::flat::test_routing_flat::public_contract::scenario_05_child_must_be_on_the_right_branch stdout ----

thread 'routing::runtime::flat::test_routing_flat::public_contract::scenario_05_child_must_be_on_the_right_branch' (4153457) panicked at src/routing/../../tests/test_routing_flat.rs:477:9:
子节点挂载到错误分支时必须被拒绝，实际：Ok(())

---- routing::runtime::flat::test_routing_flat::public_contract::scenario_08_candidate_must_correspond_to_its_node stdout ----

thread 'routing::runtime::flat::test_routing_flat::public_contract::scenario_08_candidate_must_correspond_to_its_node' (4153480) panicked at src/routing/../../tests/test_routing_flat.rs:609:9:
候选编号与匹配条件不对应时必须被拒绝，实际：Ok(())


failures:
    routing::runtime::flat::test_routing_flat::public_contract::scenario_01_sixty_five_domains_in_one_slot
    routing::runtime::flat::test_routing_flat::public_contract::scenario_02_ipv4_zero_prefix_must_be_normalized
    routing::runtime::flat::test_routing_flat::public_contract::scenario_03_ipv6_zero_prefix_must_be_normalized
    routing::runtime::flat::test_routing_flat::public_contract::scenario_04_child_prefix_must_belong_to_parent
    routing::runtime::flat::test_routing_flat::public_contract::scenario_05_child_must_be_on_the_right_branch
    routing::runtime::flat::test_routing_flat::public_contract::scenario_08_candidate_must_correspond_to_its_node

test result: FAILED. 9 passed; 6 failed; 0 ignored; 0 measured; 168 filtered out; finished in 0.48s


```

节选不替代完整日志；输入及独立预期可追踪到上表同名函数。

## 独立基线核验

同一组六项失败已在迁移前提交 `3d210fafdf3363f5c6ebf5c901178219ef9c33c5` 的独立副本复现；实际命令 `cargo test --offline --manifest-path /private/tmp/fluxa-tests-baseline/Cargo.toml --test test_routing_flat -- --test-threads=1`，退出 101，15 个用例中 9 通过、6 失败。原始日志 `/private/tmp/fluxa-tests-baseline-flat.log`。迁移没有降低断言或把失败计为成功；失败原因仍需逐项独立语义审查。
