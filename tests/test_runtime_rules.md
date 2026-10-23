# 测试报告：test_runtime_rules

<!-- nodus-test-report:{"command": ["/Users/klee/Documents/Nodus Core/target/debug/deps/test_runtime_rules-560b48bbdabbb515", "--test-threads=1"], "exit_code": 0, "duration_seconds": 2.234085915999998, "status": "PASS", "discovered": ["audit_dns_before_after_matrix", "audit_document_trust_requires_original_sources", "audit_global_transport_only_allows_reject_and_quic_is_not_udp", "audit_invalid_documents_are_rejected_with_low_sensitive_locations", "audit_resource_limits_reject_before_index_expansion", "audit_seeded_random_unpruned_differential_and_round_trip", "audit_source_and_port_priority_matrix", "binary_cache_fault_injection_never_panics_with_recomputed_checksum", "binary_cache_matches_all_matcher_shapes_before_and_after_reopen", "binary_cache_rejects_corruption_offsets_truncation_and_unknown_version", "binary_cache_rejects_legacy_sqlite_without_silent_yaml_fallback", "binary_cache_reopens_host_export_without_migrating_legacy_yaml", "both_complete_yaml_examples_strictly_compile_formal_rules", "clash_payload_is_strict_uppercase_and_supports_both_address_families", "domain_branch_skips_ip_and_respects_domain_type_and_source_priority", "domain_hit_decides_before_dns_and_domain_miss_waits", "domain_hit_skips_ip_reject_but_domain_miss_checks_ip_reject", "domain_match_skips_ip_for_domain_target", "duplicate_vec_capacity_is_accounted_without_false_rejection", "duplicates_record_origins_and_conflicts_choose_first", "empty_clash_payload_still_validates_outbound_metadata", "failed_update_preserves_disk_and_snapshot_and_close_releases_owner", "formal_domain_transport_matrix_and_roundtrip", "formal_reject_and_same_type_local_precedence_survive_scoped_conditions", "generation_input_accepts_structured_and_clash_sources", "ignored_domain_rules_in_global_direct_do_not_hide_ip_reject", "indexes_equal_reference_for_mixed_inputs_and_overlapping_ranges", "keyword_reject_is_ordinary_and_domain_hit_skips_ip", "local_plugin_remote_precedence_is_independent_of_slot_and_input_order", "mismatched_scoped_domain_does_not_skip_ip_or_early_dns", "only_complete_nonreject_coverage_is_removed", "plugin_layer_round_trip_keeps_identity_and_shared_candidates", "port_partial_overlap_stays_and_full_coverage_is_pruned", "pruned_index_agrees_with_unpruned_reference_including_original_origin", "regeneration_restores_pruned_and_disabled_sources_stay_remote", "rejects_survive_coverage_and_every_mode", "scoped_flat_cache_retains_both_protocols_and_rejects_previous_layout", "scoped_rule_validation_is_strict_and_disabled_sources_cannot_hide_illegal_rules", "source_precedes_type_but_exact_precedes_suffix_within_source", "special_reject_is_source_independent_but_suffix_and_keyword_are_ordinary", "strict_yaml_round_trip_unknown_duplicate_alias_and_depth", "suffix_boundaries_cover_all_depths_and_rejects_are_not_pruned"], "results": {"audit_dns_before_after_matrix": "PASS", "audit_document_trust_requires_original_sources": "PASS", "audit_global_transport_only_allows_reject_and_quic_is_not_udp": "PASS", "audit_invalid_documents_are_rejected_with_low_sensitive_locations": "PASS", "audit_resource_limits_reject_before_index_expansion": "PASS", "audit_seeded_random_unpruned_differential_and_round_trip": "PASS", "audit_source_and_port_priority_matrix": "PASS", "binary_cache_fault_injection_never_panics_with_recomputed_checksum": "PASS", "binary_cache_matches_all_matcher_shapes_before_and_after_reopen": "PASS", "binary_cache_rejects_corruption_offsets_truncation_and_unknown_version": "PASS", "binary_cache_rejects_legacy_sqlite_without_silent_yaml_fallback": "PASS", "binary_cache_reopens_host_export_without_migrating_legacy_yaml": "PASS", "both_complete_yaml_examples_strictly_compile_formal_rules": "PASS", "clash_payload_is_strict_uppercase_and_supports_both_address_families": "PASS", "domain_branch_skips_ip_and_respects_domain_type_and_source_priority": "PASS", "domain_hit_decides_before_dns_and_domain_miss_waits": "PASS", "domain_hit_skips_ip_reject_but_domain_miss_checks_ip_reject": "PASS", "domain_match_skips_ip_for_domain_target": "PASS", "duplicate_vec_capacity_is_accounted_without_false_rejection": "PASS", "duplicates_record_origins_and_conflicts_choose_first": "PASS", "empty_clash_payload_still_validates_outbound_metadata": "PASS", "failed_update_preserves_disk_and_snapshot_and_close_releases_owner": "PASS", "formal_domain_transport_matrix_and_roundtrip": "PASS", "formal_reject_and_same_type_local_precedence_survive_scoped_conditions": "PASS", "generation_input_accepts_structured_and_clash_sources": "PASS", "ignored_domain_rules_in_global_direct_do_not_hide_ip_reject": "PASS", "indexes_equal_reference_for_mixed_inputs_and_overlapping_ranges": "PASS", "keyword_reject_is_ordinary_and_domain_hit_skips_ip": "PASS", "local_plugin_remote_precedence_is_independent_of_slot_and_input_order": "PASS", "mismatched_scoped_domain_does_not_skip_ip_or_early_dns": "PASS", "only_complete_nonreject_coverage_is_removed": "PASS", "plugin_layer_round_trip_keeps_identity_and_shared_candidates": "PASS", "port_partial_overlap_stays_and_full_coverage_is_pruned": "PASS", "pruned_index_agrees_with_unpruned_reference_including_original_origin": "PASS", "regeneration_restores_pruned_and_disabled_sources_stay_remote": "PASS", "rejects_survive_coverage_and_every_mode": "PASS", "scoped_flat_cache_retains_both_protocols_and_rejects_previous_layout": "PASS", "scoped_rule_validation_is_strict_and_disabled_sources_cannot_hide_illegal_rules": "PASS", "source_precedes_type_but_exact_precedes_suffix_within_source": "PASS", "special_reject_is_source_independent_but_suffix_and_keyword_are_ordinary": "PASS", "strict_yaml_round_trip_unknown_duplicate_alias_and_depth": "PASS", "suffix_boundaries_cover_all_depths_and_rejects_are_not_pruned": "PASS"}, "fingerprint": "d9670c964cdab34c1d34b131936f46956cf02167ea86ba2450a3dc6fe7423345", "time": "2026-10-23T16:26:16.867832+08:00", "attempt": 1, "suite": "test_runtime_rules", "file": "tests/test_runtime_rules.rs", "environment": {"os": "macOS-27.2-arm64-arm-64bit", "architecture": "arm64", "rust": "rustc 1.98.1 (48a229cea 2026-09-01)", "cargo": "cargo 1.98.1 (797e8a9bc 2026-08-05)", "commit": "e3fa7f51836b41fd34ad202bf6c738a3a4d2c1e6", "dirty": true, "features": "Cargo 默认 features；未启用 --all-features", "optional_environments": {"NODUS_DEBUG_TEST_REQUIREMENT": false, "NODUS_DEBUG_TEST_SWIFT": false, "NODUS_PROTOCOL_PROFILE": false, "NODUS_PROTOCOL_FIXTURE": false}, "ci_run_id": null, "ci_run_attempt": null}} -->

## 基本信息

- 测试文件：`tests/test_runtime_rules.rs`
- 测试对象：src/routing/runtime.rs、src/routing/store.rs、src/routing/background.rs
- 执行机制：integration
- 测试范围：本文件登记且在当前平台实际发现的全部用例
- 实际命令：`/Users/klee/Documents/Nodus Core/target/debug/deps/test_runtime_rules-560b48bbdabbb515 --test-threads=1`
- 执行时间：2026-10-23T16:26:16.867832+08:00
- 实际耗时：2.234085915999998 秒
- 运行环境：`{"os": "macOS-27.2-arm64-arm-64bit", "architecture": "arm64", "rust": "rustc 1.98.1 (48a229cea 2026-09-01)", "cargo": "cargo 1.98.1 (797e8a9bc 2026-08-05)", "commit": "e3fa7f51836b41fd34ad202bf6c738a3a4d2c1e6", "dirty": true, "features": "Cargo 默认 features；未启用 --all-features", "optional_environments": {"NODUS_DEBUG_TEST_REQUIREMENT": false, "NODUS_DEBUG_TEST_SWIFT": false, "NODUS_PROTOCOL_PROFILE": false, "NODUS_PROTOCOL_FIXTURE": false}, "ci_run_id": null, "ci_run_attempt": null}`
- 代码/测试/配置指纹：`d9670c964cdab34c1d34b131936f46956cf02167ea86ba2450a3dc6fe7423345`
- 总体状态：**PASS**
- 原始进程退出状态：0

## 实际用例与结果

| 实际发现的用例 | 独立目标 | 状态 |
| --- | --- | --- |
| `audit_dns_before_after_matrix` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `audit_document_trust_requires_original_sources` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `audit_global_transport_only_allows_reject_and_quic_is_not_udp` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `audit_invalid_documents_are_rejected_with_low_sensitive_locations` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `audit_resource_limits_reject_before_index_expansion` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `audit_seeded_random_unpruned_differential_and_round_trip` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `audit_source_and_port_priority_matrix` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `binary_cache_fault_injection_never_panics_with_recomputed_checksum` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `binary_cache_matches_all_matcher_shapes_before_and_after_reopen` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `binary_cache_rejects_corruption_offsets_truncation_and_unknown_version` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `binary_cache_rejects_legacy_sqlite_without_silent_yaml_fallback` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `binary_cache_reopens_host_export_without_migrating_legacy_yaml` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `both_complete_yaml_examples_strictly_compile_formal_rules` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `clash_payload_is_strict_uppercase_and_supports_both_address_families` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `domain_branch_skips_ip_and_respects_domain_type_and_source_priority` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `domain_hit_decides_before_dns_and_domain_miss_waits` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `domain_hit_skips_ip_reject_but_domain_miss_checks_ip_reject` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `domain_match_skips_ip_for_domain_target` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `duplicate_vec_capacity_is_accounted_without_false_rejection` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `duplicates_record_origins_and_conflicts_choose_first` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `empty_clash_payload_still_validates_outbound_metadata` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `failed_update_preserves_disk_and_snapshot_and_close_releases_owner` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `formal_domain_transport_matrix_and_roundtrip` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `formal_reject_and_same_type_local_precedence_survive_scoped_conditions` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `generation_input_accepts_structured_and_clash_sources` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `ignored_domain_rules_in_global_direct_do_not_hide_ip_reject` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `indexes_equal_reference_for_mixed_inputs_and_overlapping_ranges` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `keyword_reject_is_ordinary_and_domain_hit_skips_ip` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `local_plugin_remote_precedence_is_independent_of_slot_and_input_order` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `mismatched_scoped_domain_does_not_skip_ip_or_early_dns` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `only_complete_nonreject_coverage_is_removed` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `plugin_layer_round_trip_keeps_identity_and_shared_candidates` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `port_partial_overlap_stays_and_full_coverage_is_pruned` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `pruned_index_agrees_with_unpruned_reference_including_original_origin` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `regeneration_restores_pruned_and_disabled_sources_stay_remote` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `rejects_survive_coverage_and_every_mode` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `scoped_flat_cache_retains_both_protocols_and_rejects_previous_layout` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `scoped_rule_validation_is_strict_and_disabled_sources_cannot_hide_illegal_rules` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `source_precedes_type_but_exact_precedes_suffix_within_source` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `special_reject_is_source_independent_but_suffix_and_keyword_are_ordinary` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `strict_yaml_round_trip_unknown_duplicate_alias_and_depth` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `suffix_boundaries_cover_all_depths_and_rejects_are_not_pruned` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |

## 执行汇总

- 实际发现：42
- 实际执行完成：42
- 通过：42
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

- 原始证据：`/private/tmp/nodus-fixes-final/test_runtime_rules.json`、对应 `.log`（实际存在时）。
- CI 工件标识：`本地证据目录；未宣称已上传 CI`
- 最新报告被后续变更影响时必须重新执行；检查器会拒绝指纹不一致的旧结果。
- 未运行的 Swift/外部协议或夹具独立目标均保持 NOT RUN；本报告不代表系统 VPN、外部服务端及跨平台已经验收。

<!-- nodus-test-documentation:begin -->
## 测试目的与目标

本套件验证来源／类型优先级、覆盖裁剪、独立参考匹配、严格输入及缓存往返语义。每项目标对应本文件实际测试函数的固定输入和独立断言；普通功能回归的 PASS 不代表性能目标通过。

## 历史规则生成与查询测量（非本次运行）

资料来自迁移前 `Documentation/RuntimeRules.md`，测量程序、版本和环境以各段原记录为准。未重新运行，未计入本次用例结果；旧版文件写入及替换流程不属于当前 Core 的文件管理职责。单机微基准不证明系统 VPN 或实际代理速度。

## 历史本机验证记录（不能作为本轮语义变更的通过证据）

当前开发代码共 121 项 Rust 测试通过，严格 Clippy 和 SwiftLint 通过；十组真实 Swift FFI 回归通过，另行验证 YAML 文件输入、key 别名、重复键失败保留旧版本。包括真实 DNS 与 TCP 连接验证：两种规则排列下，域名直连均跳过 IP 拦截；域名未命中时多候选 IP 拦截仍拒绝拨号。无新增日志通道。

性能环境：Apple M4、10 核、16 GiB，Mac16,1，macOS 27.2 ARM64，Rust 1.98.1；优化等级 3、thin LTO、一个代码生成单元。逐规模单独进程执行，40 条本地规则与四份固定顺序远程来源。输入约 30% 精确域名、20% 后缀、20% IPv4、10% IPv6、10% 关键词、10% 端口，其中每第 97 条为拦截。各规模最终有效数量与输入数量相同，不使用重复规则冒充有效规模。

| 有效规则数 | 编译时间 | 编译后 RSS | 查询进程峰值 RSS | 首部 p50/p95/p99 | 末尾 p50/p95/p99 | 未命中 p50/p95/p99 | 四线程吞吐 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 10000 | 9 ms | 27.7 MiB | 27.9 MiB | 125/167/167 ns | 166/167/167 ns | 125/167/167 ns | 2044 万次/秒 |
| 50000 | 38 ms | 62.3 MiB | 65.2 MiB | 125/166/167 ns | 166/167/167 ns | 125/167/167 ns | 2060 万次/秒 |
| 100000 | 82 ms | 104.6 MiB | 110.1 MiB | 125/167/167 ns | 166/208/375 ns | 125/167/167 ns | 2127 万次/秒 |

每类查询 20000 次，四线程累计 400000 次，使用 black_box；单次计时具有约 41 ns 的粒度，纳秒值是短查询微基准，不代表真实 DNS、TCP、TLS 或 App 端到端延迟。查询进程 RSS 含输入、诊断和生成阶段的分配器留存；并非只统计索引结构。不存在同条件其他内核的对比数据，不作跨内核性能结论。

| 有效规则数 | 连续两次完整原子更新耗时 | 关闭后进程 RSS | 更新与重载过程峰值 RSS |
| --- | --- | --- | --- |
| 10000 | 140 ms | 39.1 MiB | 39.2 MiB |
| 50000 | 655 ms | 126.6 MiB | 126.6 MiB |
| 100000 | 1287 ms | 258.0 MiB | 258.0 MiB |

更新测量包括归一化、索引编译、逐条标准 YAML 生成、严格重新解析、文件同步与替换，并实际关闭后重新加载验证有效数。第二次更新包含新旧快照共存；峰值由系统计时工具统计整个进程，包含原始输入克隆及分配器留存。并发查询与更新、关闭、规则配额耗尽和最后所有者回收另有回归测试。达到配额时明确返回错误，不伪造该规模通过。

最低 macOS 14 实际运行、其他系统/架构、TUN、网关、真实远端代理协议、App UI 与真机尚未以本次内核验收。此次不新增这些能力。crate/XCFramework/远程 Swift 包尚未发布，App 仍固定旧版；不得据此声称 App 已接入新规则引擎。

## 更新性能优化复测（2026-10-11）

作者：可莉唯一的狗。

对比基线为 dev 的 d679cea4531912d9a0be99311c2fe3ff7da34e36，加上与优化版一致的阶段计时与基准观测。两者使用相同编译选项、输入和宿主；未改变规则优先级、裁剪范围、数量上限、512 MiB 预算或文件严格校验。优化前的可执行基线保存在测试目录，随后重建优化版；不向源码仓库提交二进制。

本次改动：生成专用的原始输入、去重表、冲突表和覆盖索引在最终索引编译前释放；YAML 直接写入有界最终缓冲区，取消逐行格式化及中间文本副本；完整重新解析后直接比较全部类型值，取消两份整文件 JSON 缓冲区。字段、来源、原始序号及诊断仍参与完整校验。新旧活动快照仍按既有 Arc 生命周期共存，不通过提前释放旧连接快照降低峰值。

Store 完成结果提供六段 Duration（compile、encode、validate、write_sync、switch、directory_sync），同步及后台更新复用相同测量点；不在请求匹配路径采样，不新增日志文件或 FFI 日志推送。

同前述 M4 / macOS 27.2 / Rust 1.98.1 环境，优化构建；每个规模三轮，按优化前→优化后交替运行，每次为新进程，运行时没有本任务的并行编译或 FFI 测试。每次先生成一次规则，再执行连续两次完整原子更新、关闭及实际重载。下表为三次中位数；RSS 包含输入、诊断、生成临时对象与分配器留存，峰值还包含更新及重载，不是纯索引字节数。

| 有效规则数 | 两次更新：优化前 → 后 | 编译后 RSS：优化前 → 后 | 全进程峰值 RSS：优化前 → 后 |
| --- | --- | --- | --- |
| 10000 | 138 → 111 ms | 27.89 → 22.30 MiB | 39.56 → 30.70 MiB |
| 50000 | 638 → 507 ms | 62.48 → 52.81 MiB | 137.47 → 106.34 MiB |
| 100000 | 1260 → 1000 ms | 104.83 → 88.73 MiB | 270.00 → 193.20 MiB |

十万条两次更新约减少 20.6%，峰值约减少 28.4%；优化后两次更新范围 971–1022 ms，峰值范围 191.42–204.27 MiB。不能把历史单次 1287 ms / 258 MiB 与本次中位数混作同条件对比。

十万条每次完整更新的六次阶段样本（三轮，每轮两次）中位数：

| 阶段 | 优化前 | 优化后 |
| --- | --- | --- |
| 归一化、去重、裁剪及索引编译 | 74.13 ms | 76.25 ms |
| YAML 生成 | 208.34 ms | 88.93 ms |
| 严格重新解析与完整值校验 | 325.92 ms | 307.08 ms |
| 临时文件写入与同步 | 9.93 ms | 10.41 ms |
| 提交锁、文件替换与快照切换 | 1.79 ms | 1.81 ms |
| 目录同步 | 3.74 ms | 3.62 ms |

各阶段中位数不能简单相加作为总中位数；完整更新基准还含输入克隆、分组与观测。主要速度收益来自 YAML 生成；编译并未测得加速。当前仍以严格 YAML 解析为最大耗时，不通过省略校验换速度。

启动 RSS 中位数：优化前 5.69 MiB、优化后 5.66 MiB。十万条编译后 RSS 扣除启动基线约 99.14 → 83.08 MiB，仍包含分配器留存及整个规则文档，不能标为精确索引内存。优化后输入准备 RSS 约 15.33 MiB。活动快照在两次更新后的 RSS 范围 191.42–204.27 MiB，而关闭后约 67.94–82.42 MiB；这些数据必须分开描述，关闭不会保证分配器立即归还所有页给系统。

查询单独交替复测五轮，每轮每类 20000 次，四线程 400000 次，中位数：

| 规则数 | 优化后首部 p50/p95/p99 | 优化后末尾 p50/p95/p99 | 优化后未命中 p50/p95/p99 | 四线程吞吐：优化前 → 后 |
| --- | --- | --- | --- | --- |
| 10000 | 125/125/167 ns | 125/167/167 ns | 125/125/167 ns | 2152 → 2114 万次/秒 |
| 50000 | 125/125/167 ns | 166/167/208 ns | 125/167/167 ns | 2120 → 2141 万次/秒 |
| 100000 | 125/125/167 ns | 166/167/208 ns | 125/167/167 ns | 2151 → 2143 万次/秒 |

查询结构未修改，不宣称查询加速；十万条优化前末尾中位数为 125/167/167 ns，优化后为 166/167/208 ns，差值接近计时粒度，需要更长测试再解释。五万及十万优化后各有一次约 774 / 792 万次/秒的吞吐低样本；此前单次十万也出现约 779 万次/秒，不能隐藏这些波动或把其中任一次当作稳定吞吐。上述是匹配微基准，不代表 DNS、TLS、连接或 App 端到端速度，也不用于对比其他内核。

后台十万条单次补充检查：两次提交分别 399 / 1065 μs 返回，两次完整更新约 1065 ms，整个更新与重载进程峰值约 205.84 MiB。提交返回不等于生成完成，测量不包含 Swift 值编码时间；编译期间仍使用旧方案。

验证：122 项 Rust 测试全部通过，严格 Clippy 全目标通过，十组真实 Swift FFI 全部通过；新增完整值校验测试覆盖条件、版本、原序号、来源与统计篡改。既有未知字段、重复键、无效更新保留旧文件/快照、并发查询、取消、关闭、配额耗尽及真实 DNS/TCP 行为均回归通过。

开发版本仍为未发布的 1.4.0，未重用已发布版本；优化提交属于这项规则功能的发布前改进。crate、XCFramework、远程 Swift Package 发布及 App 固定依赖升级尚未完成。最低 macOS 14、其他系统/架构、真机和 App 本轮接入未测；本次数据不能代替这些验收。

复现：优化构建 examples/runtime_rules_bench 后分别传入 10000 / 50000 / 100000，查询不加其他参数，完整更新加 --persist，后台更新另加 --background。使用 /usr/bin/time -l 统计全进程峰值；固定相同环境与设置，交替运行前后版本并记录所有样本。

十万条后台生成的优化构建复测（相同 M4 / macOS 27.2 环境）确认两次提交分别 610 μs、450 μs 返回，完整两次原子更新及关闭耗时 1280 ms，进程最大 RSS 为 280428544 字节（约 267.4 MiB）；每次最终均保留 100000 条有效规则并重载验证。提交时间包含 Rust 输入分组、边界检查和线程创建，不包含 Swift 编码及 JSON/YAML 解码，也不等于规则生成完成时间。输入文件读取、YAML 校验与全部编译在工作线程内执行；Swift 默认 async 入口也把值编码与提交放在后台任务中。

2026-10-12 唯一 40 标签后缀压力输入：10000 条实际全进程峰值约 164.7 MiB，旧标签系数的准入估计不足以覆盖这轮观察到的峰值，已将每标签估算由 256 调整为 512 字节。长关键词 10000 条峰值约 113.3 MiB，保留每字符 64 字节估算。50000 条这两类输入在索引构建前受控返回 ResourceLimit；不能把规则数量上限解释为所有形态都能接受十万条。
<!-- nodus-test-documentation:end -->
