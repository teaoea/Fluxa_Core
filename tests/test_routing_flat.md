# 测试报告：test_routing_flat

<!-- fluxa-test-report:{"command": ["/private/tmp/fluxa-h1-session-lease/target/debug/deps/libfluxa_core-84ce2e4fcc560243", "::test_routing_flat::", "--test-threads=1"], "exit_code": 0, "duration_seconds": 0.6635814170000032, "status": "PASS", "discovered": ["routing::runtime::flat::test_routing_flat::public_contract::collision_fallback_preserves_other_sections", "routing::runtime::flat::test_routing_flat::public_contract::control_rewrite_with_recomputed_digest_still_loads", "routing::runtime::flat::test_routing_flat::public_contract::normalized_child_outside_parent_is_rejected_for_both_families", "routing::runtime::flat::test_routing_flat::public_contract::prefix_address_bits_and_leaf_boundaries_are_validated", "routing::runtime::flat::test_routing_flat::public_contract::previous_cache_layout_requires_explicit_rebuild", "routing::runtime::flat::test_routing_flat::public_contract::scenario_01_sixty_five_domains_in_one_slot", "routing::runtime::flat::test_routing_flat::public_contract::scenario_02_ipv4_zero_prefix_must_be_normalized", "routing::runtime::flat::test_routing_flat::public_contract::scenario_03_ipv6_zero_prefix_must_be_normalized", "routing::runtime::flat::test_routing_flat::public_contract::scenario_04_child_prefix_must_belong_to_parent", "routing::runtime::flat::test_routing_flat::public_contract::scenario_05_child_must_be_on_the_right_branch", "routing::runtime::flat::test_routing_flat::public_contract::scenario_06_keyword_failures_return_every_candidate", "routing::runtime::flat::test_routing_flat::public_contract::scenario_07_tampered_keyword_failure_transition_is_rejected", "routing::runtime::flat::test_routing_flat::public_contract::scenario_08_candidate_must_correspond_to_its_node", "routing::runtime::flat::test_routing_flat::public_contract::scenario_09_same_class_candidate_order_is_preserved", "routing::runtime::flat::test_routing_flat::public_contract::scenario_10_domain_and_ip_priority", "routing::runtime::flat::test_routing_flat::public_contract::scenario_11_build_peak_stays_within_budget", "routing::runtime::flat::test_routing_flat::public_contract::scenario_12_retained_snapshot_is_still_counted", "routing::runtime::flat::test_routing_flat::public_contract::scenario_13_incompatible_format_version_is_reported", "routing::runtime::flat::test_routing_flat::public_contract::scenario_14_domain_normalization_never_misses"], "results": {"routing::runtime::flat::test_routing_flat::public_contract::collision_fallback_preserves_other_sections": "PASS", "routing::runtime::flat::test_routing_flat::public_contract::control_rewrite_with_recomputed_digest_still_loads": "PASS", "routing::runtime::flat::test_routing_flat::public_contract::normalized_child_outside_parent_is_rejected_for_both_families": "PASS", "routing::runtime::flat::test_routing_flat::public_contract::prefix_address_bits_and_leaf_boundaries_are_validated": "PASS", "routing::runtime::flat::test_routing_flat::public_contract::previous_cache_layout_requires_explicit_rebuild": "PASS", "routing::runtime::flat::test_routing_flat::public_contract::scenario_01_sixty_five_domains_in_one_slot": "PASS", "routing::runtime::flat::test_routing_flat::public_contract::scenario_02_ipv4_zero_prefix_must_be_normalized": "PASS", "routing::runtime::flat::test_routing_flat::public_contract::scenario_03_ipv6_zero_prefix_must_be_normalized": "PASS", "routing::runtime::flat::test_routing_flat::public_contract::scenario_04_child_prefix_must_belong_to_parent": "PASS", "routing::runtime::flat::test_routing_flat::public_contract::scenario_05_child_must_be_on_the_right_branch": "PASS", "routing::runtime::flat::test_routing_flat::public_contract::scenario_06_keyword_failures_return_every_candidate": "PASS", "routing::runtime::flat::test_routing_flat::public_contract::scenario_07_tampered_keyword_failure_transition_is_rejected": "PASS", "routing::runtime::flat::test_routing_flat::public_contract::scenario_08_candidate_must_correspond_to_its_node": "PASS", "routing::runtime::flat::test_routing_flat::public_contract::scenario_09_same_class_candidate_order_is_preserved": "PASS", "routing::runtime::flat::test_routing_flat::public_contract::scenario_10_domain_and_ip_priority": "PASS", "routing::runtime::flat::test_routing_flat::public_contract::scenario_11_build_peak_stays_within_budget": "PASS", "routing::runtime::flat::test_routing_flat::public_contract::scenario_12_retained_snapshot_is_still_counted": "PASS", "routing::runtime::flat::test_routing_flat::public_contract::scenario_13_incompatible_format_version_is_reported": "PASS", "routing::runtime::flat::test_routing_flat::public_contract::scenario_14_domain_normalization_never_misses": "PASS"}, "fingerprint": "53a0d8684b23090b4531725c609fe4e835b392c994fa8b0e37a5afebb2ba8d56", "time": "2026-10-15T18:50:38.813158+08:00", "attempt": 4, "suite": "test_routing_flat", "file": "tests/test_routing_flat.rs", "environment": {"os": "macOS-27.2-arm64-arm-64bit", "architecture": "arm64", "rust": "rustc 1.98.1 (48a229cea 2026-09-01)", "cargo": "cargo 1.98.1 (797e8a9bc 2026-08-05)", "commit": "2f9dbdc65374da5ecea85f6a2dc49a49fcdee898", "dirty": true, "features": "Cargo 默认 features；未启用 --all-features", "optional_environments": {"FLUXA_DEBUG_TEST_REQUIREMENT": false, "FLUXA_DEBUG_TEST_SWIFT": false, "FLUXA_PROTOCOL_PROFILE": false, "FLUXA_PROTOCOL_FIXTURE": false}, "ci_run_id": null, "ci_run_attempt": null}} -->

## 基本信息

- 测试文件：`tests/test_routing_flat.rs`
- 测试对象：src/routing/flat.rs
- 执行机制：unit
- 测试范围：本文件登记且在当前平台实际发现的全部用例
- 实际命令：`/private/tmp/fluxa-h1-session-lease/target/debug/deps/libfluxa_core-84ce2e4fcc560243 ::test_routing_flat:: --test-threads=1`
- 执行时间：2026-10-15T18:50:38.813158+08:00
- 实际耗时：0.6635814170000032 秒
- 运行环境：`{"os": "macOS-27.2-arm64-arm-64bit", "architecture": "arm64", "rust": "rustc 1.98.1 (48a229cea 2026-09-01)", "cargo": "cargo 1.98.1 (797e8a9bc 2026-08-05)", "commit": "2f9dbdc65374da5ecea85f6a2dc49a49fcdee898", "dirty": true, "features": "Cargo 默认 features；未启用 --all-features", "optional_environments": {"FLUXA_DEBUG_TEST_REQUIREMENT": false, "FLUXA_DEBUG_TEST_SWIFT": false, "FLUXA_PROTOCOL_PROFILE": false, "FLUXA_PROTOCOL_FIXTURE": false}, "ci_run_id": null, "ci_run_attempt": null}`
- 代码/测试/配置指纹：`53a0d8684b23090b4531725c609fe4e835b392c994fa8b0e37a5afebb2ba8d56`
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

## 执行汇总

- 实际发现：19
- 实际执行完成：19
- 通过：19
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

- 原始证据：`/private/tmp/fluxa-flat-six-final-evidence/test_routing_flat.json`、对应 `.log`（实际存在时）。
- CI 工件标识：`本地证据目录；未宣称已上传 CI`
- 最新报告被后续变更影响时必须重新执行；检查器会拒绝指纹不一致的旧结果。
- 未运行的 Swift/外部协议或夹具独立目标均保持 NOT RUN；本报告不代表系统 VPN、外部服务端及跨平台已经验收。

## 人工独立语义与本轮最终验收

- L1：cargo fmt --check、cargo check --offline --all-targets、cargo clippy --offline --all-targets -- -D warnings 实际通过。检查 /0、/32、/128、IPv4 高位以及最长前缀挂子节点时的移位短路；不扩大公开 API，不新增 unsafe 或第三方依赖。日志 /private/tmp/fluxa-flat-six-final-check.log、/private/tmp/fluxa-flat-six-final-clippy.log。
- L2：本套件 19 项通过，原六项失败全部保留并通过；全量 Rust 303 通过、0 失败、1 既有外部对端忽略，报告工具另有 9 项通过。新集成目标 test_flat_cache_round_trip 的八组固定策略和出口在更新、恢复后均满足预期。
- L3：依据合法规则准入和缓存格式关系约束审查，不从参考实现推导预期。65 个同槽域名必须各自命中原规则序号，混合后缀/关键词/双栈节点在字符串池复用后不能串用偏移；父子包含测试修改无候选中间子树，不能靠规则编号验证代替父子校验。双栈规范化、分支、规则对应分别验证。新代码不改变规则优先级、DNS、Debug 授权、预算或快照生命周期；非法缓存不变成成功或默认 DIRECT，摘要仍不充当认证。此结论仅覆盖明确列出的契约。
- L4：历史六项失败见提交 2f9dbdc。开发期新增父子夹具选错节点、原未知版本输入 3 与正式登记 v3 冲突，均有原始失败证据，未删除断言；最终全量 run.json 退出 0，位于 /private/tmp/fluxa-flat-six-final-evidence，逐次 attempt 和更新前报告保留。新增示例日志 /private/tmp/fluxa-flat-six-final-example.log；错误契约日志 /private/tmp/fluxa-flat-six-final-errors.log 显示 43 项登记且重复/遗漏/未定义为 0。
- Swift 实际联调：routing、runtime_rules 两个完整程序通过，日志 /private/tmp/fluxa-flat-six-ffi-routing.log、/private/tmp/fluxa-flat-six-ffi-runtime.log，逐次证据 /private/tmp/fluxa-flat-six-ffi-evidence。在临时 App 稀疏副本运行，未修改 App 工作区或固定依赖。
- 兼容：内部缓存 v3，旧 v2 明确返回 Unsupported 且原文件保留；需备份旧缓存并从原始规则来源在新目录重建，再受控切换。main、已发布 API、配置 YAML、错误代码与 C ABI 未变，不发布 crate/二进制。
- 未验证：既有忽略的独立协议服务器、真实 VPN、最低系统和其他平台；后缀子边碰撞准入不在本轮范围，不宣称全仓或所有输入均安全。
