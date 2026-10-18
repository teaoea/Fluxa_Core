# 测试报告：test_routing_flat

<!-- fluxa-test-report:{"command": ["/private/tmp/fluxa-icmp-pmtu-core/target/debug/deps/libfluxa_core-4d1b10b7d1f04a59", "::test_routing_flat::", "--test-threads=1"], "exit_code": 0, "duration_seconds": 0.9086072500000029, "status": "PASS", "discovered": ["routing::runtime::flat::test_routing_flat::public_contract::collision_fallback_preserves_other_sections", "routing::runtime::flat::test_routing_flat::public_contract::control_rewrite_with_recomputed_digest_still_loads", "routing::runtime::flat::test_routing_flat::public_contract::normalized_child_outside_parent_is_rejected_for_both_families", "routing::runtime::flat::test_routing_flat::public_contract::prefix_address_bits_and_leaf_boundaries_are_validated", "routing::runtime::flat::test_routing_flat::public_contract::previous_cache_layout_requires_explicit_rebuild", "routing::runtime::flat::test_routing_flat::public_contract::scenario_01_sixty_five_domains_in_one_slot", "routing::runtime::flat::test_routing_flat::public_contract::scenario_02_ipv4_zero_prefix_must_be_normalized", "routing::runtime::flat::test_routing_flat::public_contract::scenario_03_ipv6_zero_prefix_must_be_normalized", "routing::runtime::flat::test_routing_flat::public_contract::scenario_04_child_prefix_must_belong_to_parent", "routing::runtime::flat::test_routing_flat::public_contract::scenario_05_child_must_be_on_the_right_branch", "routing::runtime::flat::test_routing_flat::public_contract::scenario_06_keyword_failures_return_every_candidate", "routing::runtime::flat::test_routing_flat::public_contract::scenario_07_tampered_keyword_failure_transition_is_rejected", "routing::runtime::flat::test_routing_flat::public_contract::scenario_08_candidate_must_correspond_to_its_node", "routing::runtime::flat::test_routing_flat::public_contract::scenario_09_same_class_candidate_order_is_preserved", "routing::runtime::flat::test_routing_flat::public_contract::scenario_10_domain_and_ip_priority", "routing::runtime::flat::test_routing_flat::public_contract::scenario_11_build_peak_stays_within_budget", "routing::runtime::flat::test_routing_flat::public_contract::scenario_12_retained_snapshot_is_still_counted", "routing::runtime::flat::test_routing_flat::public_contract::scenario_13_incompatible_format_version_is_reported", "routing::runtime::flat::test_routing_flat::public_contract::scenario_14_domain_normalization_never_misses", "routing::runtime::flat::test_routing_flat::public_contract::scoped_port_forest_roundtrip_preserves_conditions", "routing::runtime::flat::test_routing_flat::public_contract::scoped_port_roots_reject_semantic_tampering"], "results": {"routing::runtime::flat::test_routing_flat::public_contract::collision_fallback_preserves_other_sections": "PASS", "routing::runtime::flat::test_routing_flat::public_contract::control_rewrite_with_recomputed_digest_still_loads": "PASS", "routing::runtime::flat::test_routing_flat::public_contract::normalized_child_outside_parent_is_rejected_for_both_families": "PASS", "routing::runtime::flat::test_routing_flat::public_contract::prefix_address_bits_and_leaf_boundaries_are_validated": "PASS", "routing::runtime::flat::test_routing_flat::public_contract::previous_cache_layout_requires_explicit_rebuild": "PASS", "routing::runtime::flat::test_routing_flat::public_contract::scenario_01_sixty_five_domains_in_one_slot": "PASS", "routing::runtime::flat::test_routing_flat::public_contract::scenario_02_ipv4_zero_prefix_must_be_normalized": "PASS", "routing::runtime::flat::test_routing_flat::public_contract::scenario_03_ipv6_zero_prefix_must_be_normalized": "PASS", "routing::runtime::flat::test_routing_flat::public_contract::scenario_04_child_prefix_must_belong_to_parent": "PASS", "routing::runtime::flat::test_routing_flat::public_contract::scenario_05_child_must_be_on_the_right_branch": "PASS", "routing::runtime::flat::test_routing_flat::public_contract::scenario_06_keyword_failures_return_every_candidate": "PASS", "routing::runtime::flat::test_routing_flat::public_contract::scenario_07_tampered_keyword_failure_transition_is_rejected": "PASS", "routing::runtime::flat::test_routing_flat::public_contract::scenario_08_candidate_must_correspond_to_its_node": "PASS", "routing::runtime::flat::test_routing_flat::public_contract::scenario_09_same_class_candidate_order_is_preserved": "PASS", "routing::runtime::flat::test_routing_flat::public_contract::scenario_10_domain_and_ip_priority": "PASS", "routing::runtime::flat::test_routing_flat::public_contract::scenario_11_build_peak_stays_within_budget": "PASS", "routing::runtime::flat::test_routing_flat::public_contract::scenario_12_retained_snapshot_is_still_counted": "PASS", "routing::runtime::flat::test_routing_flat::public_contract::scenario_13_incompatible_format_version_is_reported": "PASS", "routing::runtime::flat::test_routing_flat::public_contract::scenario_14_domain_normalization_never_misses": "PASS", "routing::runtime::flat::test_routing_flat::public_contract::scoped_port_forest_roundtrip_preserves_conditions": "PASS", "routing::runtime::flat::test_routing_flat::public_contract::scoped_port_roots_reject_semantic_tampering": "PASS"}, "fingerprint": "5a328403bc10d073c7ae9162e549dc746a2787322605811603ed3867089142fd", "time": "2026-10-18T08:34:03.381658+08:00", "attempt": 1, "suite": "test_routing_flat", "file": "tests/test_routing_flat.rs", "environment": {"os": "macOS-27.2-arm64-arm-64bit", "architecture": "arm64", "rust": "rustc 1.98.1 (48a229cea 2026-09-01)", "cargo": "cargo 1.98.1 (797e8a9bc 2026-08-05)", "commit": "f266b25488d41dfa10b5412fbb4c2c1a9da3aaf1", "dirty": true, "features": "Cargo 默认 features；未启用 --all-features", "optional_environments": {"FLUXA_DEBUG_TEST_REQUIREMENT": false, "FLUXA_DEBUG_TEST_SWIFT": false, "FLUXA_PROTOCOL_PROFILE": false, "FLUXA_PROTOCOL_FIXTURE": false}, "ci_run_id": null, "ci_run_attempt": null}} -->

## 基本信息

- 测试文件：`tests/test_routing_flat.rs`
- 测试对象：src/routing/flat.rs
- 执行机制：unit
- 测试范围：本文件登记且在当前平台实际发现的全部用例
- 实际命令：`/private/tmp/fluxa-icmp-pmtu-core/target/debug/deps/libfluxa_core-4d1b10b7d1f04a59 ::test_routing_flat:: --test-threads=1`
- 执行时间：2026-10-18T08:34:03.381658+08:00
- 实际耗时：0.9086072500000029 秒
- 运行环境：`{"os": "macOS-27.2-arm64-arm-64bit", "architecture": "arm64", "rust": "rustc 1.98.1 (48a229cea 2026-09-01)", "cargo": "cargo 1.98.1 (797e8a9bc 2026-08-05)", "commit": "f266b25488d41dfa10b5412fbb4c2c1a9da3aaf1", "dirty": true, "features": "Cargo 默认 features；未启用 --all-features", "optional_environments": {"FLUXA_DEBUG_TEST_REQUIREMENT": false, "FLUXA_DEBUG_TEST_SWIFT": false, "FLUXA_PROTOCOL_PROFILE": false, "FLUXA_PROTOCOL_FIXTURE": false}, "ci_run_id": null, "ci_run_attempt": null}`
- 代码/测试/配置指纹：`5a328403bc10d073c7ae9162e549dc746a2787322605811603ed3867089142fd`
- 总体状态：**PASS**
- 原始进程退出状态：0

## 实际用例与结果

| 实际发现的用例 | 独立目标 | 状态 |
| --- | --- | --- |
| `routing::runtime::flat::test_routing_flat::public_contract::collision_fallback_preserves_other_sections` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `routing::runtime::flat::test_routing_flat::public_contract::control_rewrite_with_recomputed_digest_still_loads` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `routing::runtime::flat::test_routing_flat::public_contract::normalized_child_outside_parent_is_rejected_for_both_families` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `routing::runtime::flat::test_routing_flat::public_contract::prefix_address_bits_and_leaf_boundaries_are_validated` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `routing::runtime::flat::test_routing_flat::public_contract::previous_cache_layout_requires_explicit_rebuild` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `routing::runtime::flat::test_routing_flat::public_contract::scenario_01_sixty_five_domains_in_one_slot` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `routing::runtime::flat::test_routing_flat::public_contract::scenario_02_ipv4_zero_prefix_must_be_normalized` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `routing::runtime::flat::test_routing_flat::public_contract::scenario_03_ipv6_zero_prefix_must_be_normalized` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `routing::runtime::flat::test_routing_flat::public_contract::scenario_04_child_prefix_must_belong_to_parent` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `routing::runtime::flat::test_routing_flat::public_contract::scenario_05_child_must_be_on_the_right_branch` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `routing::runtime::flat::test_routing_flat::public_contract::scenario_06_keyword_failures_return_every_candidate` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `routing::runtime::flat::test_routing_flat::public_contract::scenario_07_tampered_keyword_failure_transition_is_rejected` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `routing::runtime::flat::test_routing_flat::public_contract::scenario_08_candidate_must_correspond_to_its_node` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `routing::runtime::flat::test_routing_flat::public_contract::scenario_09_same_class_candidate_order_is_preserved` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `routing::runtime::flat::test_routing_flat::public_contract::scenario_10_domain_and_ip_priority` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `routing::runtime::flat::test_routing_flat::public_contract::scenario_11_build_peak_stays_within_budget` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `routing::runtime::flat::test_routing_flat::public_contract::scenario_12_retained_snapshot_is_still_counted` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `routing::runtime::flat::test_routing_flat::public_contract::scenario_13_incompatible_format_version_is_reported` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `routing::runtime::flat::test_routing_flat::public_contract::scenario_14_domain_normalization_never_misses` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `routing::runtime::flat::test_routing_flat::public_contract::scoped_port_forest_roundtrip_preserves_conditions` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `routing::runtime::flat::test_routing_flat::public_contract::scoped_port_roots_reject_semantic_tampering` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |

## 执行汇总

- 实际发现：21
- 实际执行完成：21
- 通过：21
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

- 原始证据：`/private/tmp/fluxa-pmtu-license-rust-authorized/test_routing_flat.json`、对应 `.log`（实际存在时）。
- CI 工件标识：`本地证据目录；未宣称已上传 CI`
- 最新报告被后续变更影响时必须重新执行；检查器会拒绝指纹不一致的旧结果。
- 未运行的 Swift/外部协议或夹具独立目标均保持 NOT RUN；本报告不代表系统 VPN、外部服务端及跨平台已经验收。
