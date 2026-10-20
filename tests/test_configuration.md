# 测试报告：test_configuration

<!-- fluxa-test-report:{"command": ["/Users/klee/Documents/FLuxa_Core/target/debug/deps/libfluxa_core-3173f20207c5ee43", "::test_configuration::", "--test-threads=1"], "exit_code": 0, "duration_seconds": 0.02569229099999859, "status": "PASS", "discovered": ["configuration::test_configuration::address_filters_fake_ip_switch_and_reserved_groups_are_bounded", "configuration::test_configuration::bounded_input_and_lifetime_accounting", "configuration::test_configuration::complete_examples_are_accepted", "configuration::test_configuration::dns_bootstrap_routes_and_disabled_subsystem", "configuration::test_configuration::geoip_and_tun_fail_before_activation", "configuration::test_configuration::lan_authentication_is_strict_and_never_partially_disabled", "configuration::test_configuration::listener_ports_are_independent_and_disjoint", "configuration::test_configuration::minimal_configuration_and_per_server_timeout", "configuration::test_configuration::mitm_exclusions_precede_hosts_and_debug_options_are_not_configurable", "configuration::test_configuration::node_identity_and_references_are_case_sensitive", "configuration::test_configuration::provider_metadata_is_strict_and_cache_names_are_safe", "configuration::test_configuration::strict_fields_and_types_reject_legacy_debug_and_duplicate_input"], "results": {"configuration::test_configuration::address_filters_fake_ip_switch_and_reserved_groups_are_bounded": "PASS", "configuration::test_configuration::bounded_input_and_lifetime_accounting": "PASS", "configuration::test_configuration::complete_examples_are_accepted": "PASS", "configuration::test_configuration::dns_bootstrap_routes_and_disabled_subsystem": "PASS", "configuration::test_configuration::geoip_and_tun_fail_before_activation": "PASS", "configuration::test_configuration::lan_authentication_is_strict_and_never_partially_disabled": "PASS", "configuration::test_configuration::listener_ports_are_independent_and_disjoint": "PASS", "configuration::test_configuration::minimal_configuration_and_per_server_timeout": "PASS", "configuration::test_configuration::mitm_exclusions_precede_hosts_and_debug_options_are_not_configurable": "PASS", "configuration::test_configuration::node_identity_and_references_are_case_sensitive": "PASS", "configuration::test_configuration::provider_metadata_is_strict_and_cache_names_are_safe": "PASS", "configuration::test_configuration::strict_fields_and_types_reject_legacy_debug_and_duplicate_input": "PASS"}, "fingerprint": "c8518b9db6d5213de8febe93d1817bfc88cc8ae0c5afdfc303743d97bfe9bebf", "time": "2026-10-20T21:33:06.291616+08:00", "attempt": 1, "suite": "test_configuration", "file": "tests/test_configuration.rs", "environment": {"os": "macOS-27.2-arm64-arm-64bit", "architecture": "arm64", "rust": "rustc 1.98.1 (48a229cea 2026-09-01)", "cargo": "cargo 1.98.1 (797e8a9bc 2026-08-05)", "commit": "ca03dbe91d30381600e401e82a2cffa3a5d38c87", "dirty": true, "features": "Cargo 默认 features；未启用 --all-features", "optional_environments": {"FLUXA_DEBUG_TEST_REQUIREMENT": false, "FLUXA_DEBUG_TEST_SWIFT": false, "FLUXA_PROTOCOL_PROFILE": false, "FLUXA_PROTOCOL_FIXTURE": false}, "ci_run_id": null, "ci_run_attempt": null}} -->

## 基本信息

- 测试文件：`tests/test_configuration.rs`
- 测试对象：src/configuration.rs
- 执行机制：unit
- 测试范围：本文件登记且在当前平台实际发现的全部用例
- 实际命令：`/Users/klee/Documents/FLuxa_Core/target/debug/deps/libfluxa_core-3173f20207c5ee43 ::test_configuration:: --test-threads=1`
- 执行时间：2026-10-20T21:33:06.291616+08:00
- 实际耗时：0.02569229099999859 秒
- 运行环境：`{"os": "macOS-27.2-arm64-arm-64bit", "architecture": "arm64", "rust": "rustc 1.98.1 (48a229cea 2026-09-01)", "cargo": "cargo 1.98.1 (797e8a9bc 2026-08-05)", "commit": "ca03dbe91d30381600e401e82a2cffa3a5d38c87", "dirty": true, "features": "Cargo 默认 features；未启用 --all-features", "optional_environments": {"FLUXA_DEBUG_TEST_REQUIREMENT": false, "FLUXA_DEBUG_TEST_SWIFT": false, "FLUXA_PROTOCOL_PROFILE": false, "FLUXA_PROTOCOL_FIXTURE": false}, "ci_run_id": null, "ci_run_attempt": null}`
- 代码/测试/配置指纹：`c8518b9db6d5213de8febe93d1817bfc88cc8ae0c5afdfc303743d97bfe9bebf`
- 总体状态：**PASS**
- 原始进程退出状态：0

## 实际用例与结果

| 实际发现的用例 | 独立目标 | 状态 |
| --- | --- | --- |
| `configuration::test_configuration::address_filters_fake_ip_switch_and_reserved_groups_are_bounded` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `configuration::test_configuration::bounded_input_and_lifetime_accounting` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `configuration::test_configuration::complete_examples_are_accepted` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `configuration::test_configuration::dns_bootstrap_routes_and_disabled_subsystem` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `configuration::test_configuration::geoip_and_tun_fail_before_activation` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `configuration::test_configuration::lan_authentication_is_strict_and_never_partially_disabled` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `configuration::test_configuration::listener_ports_are_independent_and_disjoint` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `configuration::test_configuration::minimal_configuration_and_per_server_timeout` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `configuration::test_configuration::mitm_exclusions_precede_hosts_and_debug_options_are_not_configurable` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `configuration::test_configuration::node_identity_and_references_are_case_sensitive` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `configuration::test_configuration::provider_metadata_is_strict_and_cache_names_are_safe` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `configuration::test_configuration::strict_fields_and_types_reject_legacy_debug_and_duplicate_input` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |

## 执行汇总

- 实际发现：12
- 实际执行完成：12
- 通过：12
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

- 原始证据：`/private/tmp/fluxa-when-work/final/test_configuration.json`、对应 `.log`（实际存在时）。
- CI 工件标识：`本地证据目录；未宣称已上传 CI`
- 最新报告被后续变更影响时必须重新执行；检查器会拒绝指纹不一致的旧结果。
- 未运行的 Swift/外部协议或夹具独立目标均保持 NOT RUN；本报告不代表系统 VPN、外部服务端及跨平台已经验收。
