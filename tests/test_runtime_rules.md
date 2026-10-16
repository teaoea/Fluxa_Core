# 测试报告：test_runtime_rules

<!-- fluxa-test-report:{"command": ["/private/tmp/fluxa-tcp-latest/target/debug/deps/test_runtime_rules-813a956430c55a88", "--test-threads=1"], "exit_code": 0, "duration_seconds": 2.327476374999989, "status": "PASS", "discovered": ["audit_dns_before_after_matrix", "audit_document_trust_requires_original_sources", "audit_global_transport_only_allows_reject_and_quic_is_not_udp", "audit_invalid_documents_are_rejected_with_low_sensitive_locations", "audit_primary_domain_and_ip_precede_local_port", "audit_resource_limits_reject_before_index_expansion", "audit_seeded_random_unpruned_differential_and_round_trip", "binary_cache_fault_injection_never_panics_with_recomputed_checksum", "binary_cache_matches_all_matcher_shapes_before_and_after_reopen", "binary_cache_rejects_corruption_offsets_truncation_and_unknown_version", "binary_cache_rejects_legacy_sqlite_without_silent_yaml_fallback", "binary_cache_reopens_merged_rules_and_migrates_only_internal_legacy_yaml", "both_complete_yaml_examples_strictly_compile_formal_rules", "clash_payload_is_strict_uppercase_and_supports_both_address_families", "domain_branch_skips_ip_and_respects_domain_type_and_source_priority", "domain_hit_decides_before_dns_and_domain_miss_waits", "domain_hit_skips_ip_reject_but_domain_miss_checks_ip_reject", "domain_match_skips_ip_for_domain_target", "duplicate_vec_capacity_is_accounted_without_false_rejection", "duplicates_record_origins_and_conflicts_choose_first", "empty_clash_payload_still_validates_outbound_metadata", "exact_domain_precedes_earlier_suffix_across_source_layers", "failed_update_preserves_disk_and_snapshot_and_close_releases_owner", "formal_domain_transport_matrix_and_roundtrip", "formal_reject_and_same_type_local_precedence_survive_scoped_conditions", "generation_input_accepts_structured_and_clash_sources", "indexes_equal_reference_for_mixed_inputs_and_overlapping_ranges", "keyword_reject_overrides_domain_actions_and_domain_hit_skips_ip", "mismatched_scoped_domain_does_not_skip_ip_or_early_dns", "only_complete_nonreject_coverage_is_removed", "port_partial_overlap_stays_and_full_coverage_is_pruned", "pruned_index_agrees_with_unpruned_reference_including_original_origin", "regeneration_restores_pruned_and_disabled_sources_stay_remote", "rejects_survive_coverage_and_every_mode", "scoped_flat_cache_retains_both_protocols_and_rejects_previous_layout", "scoped_rule_validation_is_strict_and_disabled_sources_cannot_hide_illegal_rules", "strict_yaml_round_trip_unknown_duplicate_alias_and_depth", "suffix_boundaries_cover_all_depths_and_rejects_are_not_pruned"], "results": {"audit_dns_before_after_matrix": "PASS", "audit_document_trust_requires_original_sources": "PASS", "audit_global_transport_only_allows_reject_and_quic_is_not_udp": "PASS", "audit_invalid_documents_are_rejected_with_low_sensitive_locations": "PASS", "audit_primary_domain_and_ip_precede_local_port": "PASS", "audit_resource_limits_reject_before_index_expansion": "PASS", "audit_seeded_random_unpruned_differential_and_round_trip": "PASS", "binary_cache_fault_injection_never_panics_with_recomputed_checksum": "PASS", "binary_cache_matches_all_matcher_shapes_before_and_after_reopen": "PASS", "binary_cache_rejects_corruption_offsets_truncation_and_unknown_version": "PASS", "binary_cache_rejects_legacy_sqlite_without_silent_yaml_fallback": "PASS", "binary_cache_reopens_merged_rules_and_migrates_only_internal_legacy_yaml": "PASS", "both_complete_yaml_examples_strictly_compile_formal_rules": "PASS", "clash_payload_is_strict_uppercase_and_supports_both_address_families": "PASS", "domain_branch_skips_ip_and_respects_domain_type_and_source_priority": "PASS", "domain_hit_decides_before_dns_and_domain_miss_waits": "PASS", "domain_hit_skips_ip_reject_but_domain_miss_checks_ip_reject": "PASS", "domain_match_skips_ip_for_domain_target": "PASS", "duplicate_vec_capacity_is_accounted_without_false_rejection": "PASS", "duplicates_record_origins_and_conflicts_choose_first": "PASS", "empty_clash_payload_still_validates_outbound_metadata": "PASS", "exact_domain_precedes_earlier_suffix_across_source_layers": "PASS", "failed_update_preserves_disk_and_snapshot_and_close_releases_owner": "PASS", "formal_domain_transport_matrix_and_roundtrip": "PASS", "formal_reject_and_same_type_local_precedence_survive_scoped_conditions": "PASS", "generation_input_accepts_structured_and_clash_sources": "PASS", "indexes_equal_reference_for_mixed_inputs_and_overlapping_ranges": "PASS", "keyword_reject_overrides_domain_actions_and_domain_hit_skips_ip": "PASS", "mismatched_scoped_domain_does_not_skip_ip_or_early_dns": "PASS", "only_complete_nonreject_coverage_is_removed": "PASS", "port_partial_overlap_stays_and_full_coverage_is_pruned": "PASS", "pruned_index_agrees_with_unpruned_reference_including_original_origin": "PASS", "regeneration_restores_pruned_and_disabled_sources_stay_remote": "PASS", "rejects_survive_coverage_and_every_mode": "PASS", "scoped_flat_cache_retains_both_protocols_and_rejects_previous_layout": "PASS", "scoped_rule_validation_is_strict_and_disabled_sources_cannot_hide_illegal_rules": "PASS", "strict_yaml_round_trip_unknown_duplicate_alias_and_depth": "PASS", "suffix_boundaries_cover_all_depths_and_rejects_are_not_pruned": "PASS"}, "fingerprint": "5347bf6b24a14449f71d68e85b9c355d4a8211693f7ab52851f026f1910f557c", "time": "2026-10-16T10:06:37.444762+08:00", "attempt": 1, "suite": "test_runtime_rules", "file": "tests/test_runtime_rules.rs", "environment": {"os": "macOS-27.2-arm64-arm-64bit", "architecture": "arm64", "rust": "rustc 1.98.1 (48a229cea 2026-09-01)", "cargo": "cargo 1.98.1 (797e8a9bc 2026-08-05)", "commit": "769a8afd53ac269e6545233c8ec5515de2c4745f", "dirty": true, "features": "Cargo 默认 features；未启用 --all-features", "optional_environments": {"FLUXA_DEBUG_TEST_REQUIREMENT": false, "FLUXA_DEBUG_TEST_SWIFT": false, "FLUXA_PROTOCOL_PROFILE": false, "FLUXA_PROTOCOL_FIXTURE": false}, "ci_run_id": null, "ci_run_attempt": null}} -->

## 基本信息

- 测试文件：`tests/test_runtime_rules.rs`
- 测试对象：src/routing/runtime.rs、src/routing/store.rs、src/routing/background.rs
- 执行机制：integration
- 测试范围：本文件登记且在当前平台实际发现的全部用例
- 实际命令：`/private/tmp/fluxa-tcp-latest/target/debug/deps/test_runtime_rules-813a956430c55a88 --test-threads=1`
- 执行时间：2026-10-16T10:06:37.444762+08:00
- 实际耗时：2.327476374999989 秒
- 运行环境：`{"os": "macOS-27.2-arm64-arm-64bit", "architecture": "arm64", "rust": "rustc 1.98.1 (48a229cea 2026-09-01)", "cargo": "cargo 1.98.1 (797e8a9bc 2026-08-05)", "commit": "769a8afd53ac269e6545233c8ec5515de2c4745f", "dirty": true, "features": "Cargo 默认 features；未启用 --all-features", "optional_environments": {"FLUXA_DEBUG_TEST_REQUIREMENT": false, "FLUXA_DEBUG_TEST_SWIFT": false, "FLUXA_PROTOCOL_PROFILE": false, "FLUXA_PROTOCOL_FIXTURE": false}, "ci_run_id": null, "ci_run_attempt": null}`
- 代码/测试/配置指纹：`5347bf6b24a14449f71d68e85b9c355d4a8211693f7ab52851f026f1910f557c`
- 总体状态：**PASS**
- 原始进程退出状态：0

## 实际用例与结果

| 实际发现的用例 | 独立目标 | 状态 |
| --- | --- | --- |
| `audit_dns_before_after_matrix` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `audit_document_trust_requires_original_sources` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `audit_global_transport_only_allows_reject_and_quic_is_not_udp` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `audit_invalid_documents_are_rejected_with_low_sensitive_locations` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `audit_primary_domain_and_ip_precede_local_port` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `audit_resource_limits_reject_before_index_expansion` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `audit_seeded_random_unpruned_differential_and_round_trip` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `binary_cache_fault_injection_never_panics_with_recomputed_checksum` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `binary_cache_matches_all_matcher_shapes_before_and_after_reopen` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `binary_cache_rejects_corruption_offsets_truncation_and_unknown_version` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `binary_cache_rejects_legacy_sqlite_without_silent_yaml_fallback` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `binary_cache_reopens_merged_rules_and_migrates_only_internal_legacy_yaml` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `both_complete_yaml_examples_strictly_compile_formal_rules` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `clash_payload_is_strict_uppercase_and_supports_both_address_families` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `domain_branch_skips_ip_and_respects_domain_type_and_source_priority` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `domain_hit_decides_before_dns_and_domain_miss_waits` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `domain_hit_skips_ip_reject_but_domain_miss_checks_ip_reject` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `domain_match_skips_ip_for_domain_target` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `duplicate_vec_capacity_is_accounted_without_false_rejection` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `duplicates_record_origins_and_conflicts_choose_first` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `empty_clash_payload_still_validates_outbound_metadata` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `exact_domain_precedes_earlier_suffix_across_source_layers` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `failed_update_preserves_disk_and_snapshot_and_close_releases_owner` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `formal_domain_transport_matrix_and_roundtrip` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `formal_reject_and_same_type_local_precedence_survive_scoped_conditions` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `generation_input_accepts_structured_and_clash_sources` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `indexes_equal_reference_for_mixed_inputs_and_overlapping_ranges` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `keyword_reject_overrides_domain_actions_and_domain_hit_skips_ip` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `mismatched_scoped_domain_does_not_skip_ip_or_early_dns` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `only_complete_nonreject_coverage_is_removed` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `port_partial_overlap_stays_and_full_coverage_is_pruned` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `pruned_index_agrees_with_unpruned_reference_including_original_origin` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `regeneration_restores_pruned_and_disabled_sources_stay_remote` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `rejects_survive_coverage_and_every_mode` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `scoped_flat_cache_retains_both_protocols_and_rejects_previous_layout` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `scoped_rule_validation_is_strict_and_disabled_sources_cannot_hide_illegal_rules` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `strict_yaml_round_trip_unknown_duplicate_alias_and_depth` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `suffix_boundaries_cover_all_depths_and_rejects_are_not_pruned` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |

## 执行汇总

- 实际发现：38
- 实际执行完成：38
- 通过：38
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

- 原始证据：`/private/tmp/fluxa-sack-rust-verified/test_runtime_rules.json`、对应 `.log`（实际存在时）。
- CI 工件标识：`本地证据目录；未宣称已上传 CI`
- 最新报告被后续变更影响时必须重新执行；检查器会拒绝指纹不一致的旧结果。
- 未运行的 Swift/外部协议或夹具独立目标均保持 NOT RUN；本报告不代表系统 VPN、外部服务端及跨平台已经验收。
