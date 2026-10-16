# 测试报告：test_dns_fake_ip

<!-- fluxa-test-report:{"command": ["/Users/klee/Documents/FLuxa_Core/target/debug/deps/libfluxa_core-1494eed5db643679", "::test_dns_fake_ip::", "--test-threads=1"], "exit_code": 0, "duration_seconds": 0.01467033299999998, "status": "PASS", "discovered": ["dns::fake_ip::test_dns_fake_ip::public_contract::allocation_is_stable_reversible_and_does_not_rewind", "dns::fake_ip::test_dns_fake_ip::public_contract::capacity_and_exclude_are_bounded", "dns::fake_ip::test_dns_fake_ip::public_contract::capacity_limit_is_enforced", "dns::fake_ip::test_dns_fake_ip::public_contract::excluded_literals_and_root_are_not_mapped", "dns::fake_ip::test_dns_fake_ip::public_contract::range_must_stay_inside_reserved_blocks", "dns::fake_ip::test_dns_fake_ip::tests::active_limit_is_reclaimed_and_concurrent_shared_allocations_are_unique", "dns::fake_ip::test_dns_fake_ip::tests::either_family_history_prevents_reinterpretation_after_reconfiguration", "dns::fake_ip::test_dns_fake_ip::tests::fragmented_history_limit_is_atomic_and_adjacent_history_merges", "dns::fake_ip::test_dns_fake_ip::tests::retired_pool_can_expand_without_reusing_either_address_family", "dns::fake_ip::test_dns_fake_ip::tests::shared_pool_remains_active_until_last_owner_and_allocations_are_unique", "dns::fake_ip::test_dns_fake_ip::tests::unused_retired_namespaces_do_not_consume_lifetime_quota"], "results": {"dns::fake_ip::test_dns_fake_ip::public_contract::allocation_is_stable_reversible_and_does_not_rewind": "PASS", "dns::fake_ip::test_dns_fake_ip::public_contract::capacity_and_exclude_are_bounded": "PASS", "dns::fake_ip::test_dns_fake_ip::public_contract::capacity_limit_is_enforced": "PASS", "dns::fake_ip::test_dns_fake_ip::public_contract::excluded_literals_and_root_are_not_mapped": "PASS", "dns::fake_ip::test_dns_fake_ip::public_contract::range_must_stay_inside_reserved_blocks": "PASS", "dns::fake_ip::test_dns_fake_ip::tests::active_limit_is_reclaimed_and_concurrent_shared_allocations_are_unique": "PASS", "dns::fake_ip::test_dns_fake_ip::tests::either_family_history_prevents_reinterpretation_after_reconfiguration": "PASS", "dns::fake_ip::test_dns_fake_ip::tests::fragmented_history_limit_is_atomic_and_adjacent_history_merges": "PASS", "dns::fake_ip::test_dns_fake_ip::tests::retired_pool_can_expand_without_reusing_either_address_family": "PASS", "dns::fake_ip::test_dns_fake_ip::tests::shared_pool_remains_active_until_last_owner_and_allocations_are_unique": "PASS", "dns::fake_ip::test_dns_fake_ip::tests::unused_retired_namespaces_do_not_consume_lifetime_quota": "PASS"}, "fingerprint": "b0d9cf1d146c16629d16ec189f573f171b934ef9ba879d70d56835c03fd4a304", "time": "2026-10-16T11:05:33.822306+08:00", "attempt": 1, "suite": "test_dns_fake_ip", "file": "tests/test_dns_fake_ip.rs", "environment": {"os": "macOS-27.2-arm64-arm-64bit", "architecture": "arm64", "rust": "rustc 1.98.1 (48a229cea 2026-09-01)", "cargo": "cargo 1.98.1 (797e8a9bc 2026-08-05)", "commit": "d8e44846fe2de2fffc7d33b4d6ef7a4350fc94c3", "dirty": true, "features": "Cargo 默认 features；未启用 --all-features", "optional_environments": {"FLUXA_DEBUG_TEST_REQUIREMENT": false, "FLUXA_DEBUG_TEST_SWIFT": false, "FLUXA_PROTOCOL_PROFILE": false, "FLUXA_PROTOCOL_FIXTURE": false}, "ci_run_id": null, "ci_run_attempt": null}} -->

## 基本信息

- 测试文件：`tests/test_dns_fake_ip.rs`
- 测试对象：src/dns/fake_ip.rs
- 执行机制：unit
- 测试范围：本文件登记且在当前平台实际发现的全部用例
- 实际命令：`/Users/klee/Documents/FLuxa_Core/target/debug/deps/libfluxa_core-1494eed5db643679 ::test_dns_fake_ip:: --test-threads=1`
- 执行时间：2026-10-16T11:05:33.822306+08:00
- 实际耗时：0.01467033299999998 秒
- 运行环境：`{"os": "macOS-27.2-arm64-arm-64bit", "architecture": "arm64", "rust": "rustc 1.98.1 (48a229cea 2026-09-01)", "cargo": "cargo 1.98.1 (797e8a9bc 2026-08-05)", "commit": "d8e44846fe2de2fffc7d33b4d6ef7a4350fc94c3", "dirty": true, "features": "Cargo 默认 features；未启用 --all-features", "optional_environments": {"FLUXA_DEBUG_TEST_REQUIREMENT": false, "FLUXA_DEBUG_TEST_SWIFT": false, "FLUXA_PROTOCOL_PROFILE": false, "FLUXA_PROTOCOL_FIXTURE": false}, "ci_run_id": null, "ci_run_attempt": null}`
- 代码/测试/配置指纹：`b0d9cf1d146c16629d16ec189f573f171b934ef9ba879d70d56835c03fd4a304`
- 总体状态：**PASS**
- 原始进程退出状态：0

## 实际用例与结果

| 实际发现的用例 | 独立目标 | 状态 |
| --- | --- | --- |
| `dns::fake_ip::test_dns_fake_ip::public_contract::allocation_is_stable_reversible_and_does_not_rewind` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `dns::fake_ip::test_dns_fake_ip::public_contract::capacity_and_exclude_are_bounded` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `dns::fake_ip::test_dns_fake_ip::public_contract::capacity_limit_is_enforced` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `dns::fake_ip::test_dns_fake_ip::public_contract::excluded_literals_and_root_are_not_mapped` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `dns::fake_ip::test_dns_fake_ip::public_contract::range_must_stay_inside_reserved_blocks` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `dns::fake_ip::test_dns_fake_ip::tests::active_limit_is_reclaimed_and_concurrent_shared_allocations_are_unique` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `dns::fake_ip::test_dns_fake_ip::tests::either_family_history_prevents_reinterpretation_after_reconfiguration` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `dns::fake_ip::test_dns_fake_ip::tests::fragmented_history_limit_is_atomic_and_adjacent_history_merges` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `dns::fake_ip::test_dns_fake_ip::tests::retired_pool_can_expand_without_reusing_either_address_family` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `dns::fake_ip::test_dns_fake_ip::tests::shared_pool_remains_active_until_last_owner_and_allocations_are_unique` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `dns::fake_ip::test_dns_fake_ip::tests::unused_retired_namespaces_do_not_consume_lifetime_quota` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |

## 执行汇总

- 实际发现：11
- 实际执行完成：11
- 通过：11
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

- 原始证据：`/private/tmp/fluxa-f01-delivery-evidence/test_dns_fake_ip.json`、对应 `.log`（实际存在时）。
- CI 工件标识：`本地证据目录；未宣称已上传 CI`
- 最新报告被后续变更影响时必须重新执行；检查器会拒绝指纹不一致的旧结果。
- 未运行的 Swift/外部协议或夹具独立目标均保持 NOT RUN；本报告不代表系统 VPN、外部服务端及跨平台已经验收。
