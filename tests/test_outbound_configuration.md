# 测试报告：test_outbound_configuration

<!-- fluxa-test-report:{"command": ["/private/tmp/fluxa-macos-team-storage/target/debug/deps/libfluxa_core-318794320ad432f5", "::test_outbound_configuration::", "--test-threads=1"], "exit_code": 0, "duration_seconds": 2.1632631250000003, "status": "PASS", "discovered": ["outbound::configuration::test_outbound_configuration::concurrent_candidates_have_one_commit_winner", "outbound::configuration::test_outbound_configuration::debug_overlay_survives_main_commit", "outbound::configuration::test_outbound_configuration::dns_geoip_contract::admitted_family_fallback_survives_configuration_replacement", "outbound::configuration::test_outbound_configuration::dns_geoip_contract::explicit_doh_ignores_formal_route_and_retires_old_resolver", "outbound::configuration::test_outbound_configuration::dns_geoip_contract::geoip_ready_failure_atomicity_and_disabled_rules", "outbound::configuration::test_outbound_configuration::dns_geoip_contract::named_numeric_dns_failure_never_uses_direct", "outbound::configuration::test_outbound_configuration::dns_geoip_contract::ordered_upstreams_get_independent_deadlines", "outbound::configuration::test_outbound_configuration::dns_geoip_contract::preferred_family_order_ip_reject_and_nxdomain", "outbound::configuration::test_outbound_configuration::failed_preparation_preserves_previous_configuration", "outbound::configuration::test_outbound_configuration::partial_updates_invalidate_summary_but_retain_budget", "outbound::configuration::test_outbound_configuration::replacement_releases_unreferenced_old_snapshot", "outbound::configuration::test_outbound_configuration::stale_preparation_cannot_overwrite_newer_configuration", "outbound::configuration::test_outbound_configuration::stopped_manager_rejects_prepared_candidate"], "results": {"outbound::configuration::test_outbound_configuration::concurrent_candidates_have_one_commit_winner": "PASS", "outbound::configuration::test_outbound_configuration::debug_overlay_survives_main_commit": "PASS", "outbound::configuration::test_outbound_configuration::dns_geoip_contract::admitted_family_fallback_survives_configuration_replacement": "PASS", "outbound::configuration::test_outbound_configuration::dns_geoip_contract::explicit_doh_ignores_formal_route_and_retires_old_resolver": "PASS", "outbound::configuration::test_outbound_configuration::dns_geoip_contract::geoip_ready_failure_atomicity_and_disabled_rules": "PASS", "outbound::configuration::test_outbound_configuration::dns_geoip_contract::named_numeric_dns_failure_never_uses_direct": "PASS", "outbound::configuration::test_outbound_configuration::dns_geoip_contract::ordered_upstreams_get_independent_deadlines": "PASS", "outbound::configuration::test_outbound_configuration::dns_geoip_contract::preferred_family_order_ip_reject_and_nxdomain": "PASS", "outbound::configuration::test_outbound_configuration::failed_preparation_preserves_previous_configuration": "PASS", "outbound::configuration::test_outbound_configuration::partial_updates_invalidate_summary_but_retain_budget": "PASS", "outbound::configuration::test_outbound_configuration::replacement_releases_unreferenced_old_snapshot": "PASS", "outbound::configuration::test_outbound_configuration::stale_preparation_cannot_overwrite_newer_configuration": "PASS", "outbound::configuration::test_outbound_configuration::stopped_manager_rejects_prepared_candidate": "PASS"}, "fingerprint": "95a77d0e822027d3d8d91dc72bd23cd1b197af94788923845e5c4d07153812fe", "time": "2026-10-21T05:14:07.529593+08:00", "attempt": 1, "suite": "test_outbound_configuration", "file": "tests/test_outbound_configuration.rs", "environment": {"os": "macOS-27.2-arm64-arm-64bit", "architecture": "arm64", "rust": "rustc 1.98.1 (48a229cea 2026-09-01)", "cargo": "cargo 1.98.1 (797e8a9bc 2026-08-05)", "commit": "ca03dbe91d30381600e401e82a2cffa3a5d38c87", "dirty": true, "features": "Cargo 默认 features；未启用 --all-features", "optional_environments": {"FLUXA_DEBUG_TEST_REQUIREMENT": false, "FLUXA_DEBUG_TEST_SWIFT": false, "FLUXA_PROTOCOL_PROFILE": false, "FLUXA_PROTOCOL_FIXTURE": false}, "ci_run_id": null, "ci_run_attempt": null}} -->

## 基本信息

- 测试文件：`tests/test_outbound_configuration.rs`
- 测试对象：src/outbound/configuration.rs、src/dns/main.rs、src/providers.rs、src/outbound/execution.rs
- 执行机制：unit
- 测试范围：本文件登记且在当前平台实际发现的全部用例
- 实际命令：`/private/tmp/fluxa-macos-team-storage/target/debug/deps/libfluxa_core-318794320ad432f5 ::test_outbound_configuration:: --test-threads=1`
- 执行时间：2026-10-21T05:14:07.529593+08:00
- 实际耗时：2.1632631250000003 秒
- 运行环境：`{"os": "macOS-27.2-arm64-arm-64bit", "architecture": "arm64", "rust": "rustc 1.98.1 (48a229cea 2026-09-01)", "cargo": "cargo 1.98.1 (797e8a9bc 2026-08-05)", "commit": "ca03dbe91d30381600e401e82a2cffa3a5d38c87", "dirty": true, "features": "Cargo 默认 features；未启用 --all-features", "optional_environments": {"FLUXA_DEBUG_TEST_REQUIREMENT": false, "FLUXA_DEBUG_TEST_SWIFT": false, "FLUXA_PROTOCOL_PROFILE": false, "FLUXA_PROTOCOL_FIXTURE": false}, "ci_run_id": null, "ci_run_attempt": null}`
- 代码/测试/配置指纹：`95a77d0e822027d3d8d91dc72bd23cd1b197af94788923845e5c4d07153812fe`
- 总体状态：**PASS**
- 原始进程退出状态：0

## 实际用例与结果

| 实际发现的用例 | 独立目标 | 状态 |
| --- | --- | --- |
| `outbound::configuration::test_outbound_configuration::concurrent_candidates_have_one_commit_winner` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `outbound::configuration::test_outbound_configuration::debug_overlay_survives_main_commit` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `outbound::configuration::test_outbound_configuration::dns_geoip_contract::admitted_family_fallback_survives_configuration_replacement` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `outbound::configuration::test_outbound_configuration::dns_geoip_contract::explicit_doh_ignores_formal_route_and_retires_old_resolver` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `outbound::configuration::test_outbound_configuration::dns_geoip_contract::geoip_ready_failure_atomicity_and_disabled_rules` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `outbound::configuration::test_outbound_configuration::dns_geoip_contract::named_numeric_dns_failure_never_uses_direct` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `outbound::configuration::test_outbound_configuration::dns_geoip_contract::ordered_upstreams_get_independent_deadlines` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `outbound::configuration::test_outbound_configuration::dns_geoip_contract::preferred_family_order_ip_reject_and_nxdomain` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `outbound::configuration::test_outbound_configuration::failed_preparation_preserves_previous_configuration` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `outbound::configuration::test_outbound_configuration::partial_updates_invalidate_summary_but_retain_budget` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `outbound::configuration::test_outbound_configuration::replacement_releases_unreferenced_old_snapshot` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `outbound::configuration::test_outbound_configuration::stale_preparation_cannot_overwrite_newer_configuration` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `outbound::configuration::test_outbound_configuration::stopped_manager_rejects_prepared_candidate` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |

## 执行汇总

- 实际发现：13
- 实际执行完成：13
- 通过：13
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

- 原始证据：`/private/tmp/fluxa-team-final-evidence/test_outbound_configuration.json`、对应 `.log`（实际存在时）。
- CI 工件标识：`本地证据目录；未宣称已上传 CI`
- 最新报告被后续变更影响时必须重新执行；检查器会拒绝指纹不一致的旧结果。
- 未运行的 Swift/外部协议或夹具独立目标均保持 NOT RUN；本报告不代表系统 VPN、外部服务端及跨平台已经验收。


## 迁移前历史失败记录（不作为当前执行结论）

旧套件已在基线迁入本测试源码，保留原始记录如下；它不再是独立活动报告。

> STALE：2026-10-19 六项用例已整体迁入 tests/test_outbound_configuration.rs。下文是迁移前失败记录，不代表当前代码；新运行结果见同名 test_outbound_configuration.md，历史原始证据保留在 /private/tmp/fluxa-core-storage-evidence。

# 测试报告：test_dns_geoip_contract

<!-- fluxa-test-report:{"command": ["/Users/klee/Documents/FLuxa_Core/target/debug/deps/test_dns_geoip_contract-126039fa5931c8af", "--test-threads=1"], "exit_code": 101, "duration_seconds": 0.6375710419999976, "status": "FAIL", "discovered": ["admitted_family_fallback_survives_configuration_replacement", "explicit_doh_ignores_formal_route_and_retires_old_resolver", "geoip_ready_failure_atomicity_and_disabled_rules", "named_numeric_dns_failure_never_uses_direct", "ordered_upstreams_get_independent_deadlines", "preferred_family_order_ip_reject_and_nxdomain"], "results": {"admitted_family_fallback_survives_configuration_replacement": "FAIL", "explicit_doh_ignores_formal_route_and_retires_old_resolver": "FAIL", "geoip_ready_failure_atomicity_and_disabled_rules": "FAIL", "named_numeric_dns_failure_never_uses_direct": "FAIL", "ordered_upstreams_get_independent_deadlines": "FAIL", "preferred_family_order_ip_reject_and_nxdomain": "FAIL"}, "fingerprint": "c0124041134696174ce59ac109e0902f66a5b6f141aebda4cdbcee65dbf259ea", "time": "2026-10-19T14:44:23.149307+08:00", "attempt": 1, "suite": "test_dns_geoip_contract", "file": "tests/test_dns_geoip_contract.rs", "environment": {"os": "macOS-27.2-arm64-arm-64bit", "architecture": "arm64", "rust": "rustc 1.98.1 (48a229cea 2026-09-01)", "cargo": "cargo 1.98.1 (797e8a9bc 2026-08-05)", "commit": "a84c532496de4d2c98da8cf847aec4ea0b9abd3c", "dirty": true, "features": "Cargo 默认 features；未启用 --all-features", "optional_environments": {"FLUXA_DEBUG_TEST_REQUIREMENT": false, "FLUXA_DEBUG_TEST_SWIFT": false, "FLUXA_PROTOCOL_PROFILE": false, "FLUXA_PROTOCOL_FIXTURE": false}, "ci_run_id": null, "ci_run_attempt": null}} -->

## 基本信息

- 测试文件：`tests/test_dns_geoip_contract.rs`
- 测试对象：src/dns/main.rs、src/providers.rs、src/outbound/configuration.rs、src/outbound/execution.rs
- 执行机制：integration
- 测试范围：本文件登记且在当前平台实际发现的全部用例
- 实际命令：`/Users/klee/Documents/FLuxa_Core/target/debug/deps/test_dns_geoip_contract-126039fa5931c8af --test-threads=1`
- 执行时间：2026-10-19T14:44:23.149307+08:00
- 实际耗时：0.6375710419999976 秒
- 运行环境：`{"os": "macOS-27.2-arm64-arm-64bit", "architecture": "arm64", "rust": "rustc 1.98.1 (48a229cea 2026-09-01)", "cargo": "cargo 1.98.1 (797e8a9bc 2026-08-05)", "commit": "a84c532496de4d2c98da8cf847aec4ea0b9abd3c", "dirty": true, "features": "Cargo 默认 features；未启用 --all-features", "optional_environments": {"FLUXA_DEBUG_TEST_REQUIREMENT": false, "FLUXA_DEBUG_TEST_SWIFT": false, "FLUXA_PROTOCOL_PROFILE": false, "FLUXA_PROTOCOL_FIXTURE": false}, "ci_run_id": null, "ci_run_attempt": null}`
- 代码/测试/配置指纹：`c0124041134696174ce59ac109e0902f66a5b6f141aebda4cdbcee65dbf259ea`
- 总体状态：**FAIL**
- 原始进程退出状态：101

## 实际用例与结果

| 实际发现的用例 | 独立目标 | 状态 |
| --- | --- | --- |
| `admitted_family_fallback_survives_configuration_replacement` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | FAIL |
| `explicit_doh_ignores_formal_route_and_retires_old_resolver` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | FAIL |
| `geoip_ready_failure_atomicity_and_disabled_rules` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | FAIL |
| `named_numeric_dns_failure_never_uses_direct` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | FAIL |
| `ordered_upstreams_get_independent_deadlines` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | FAIL |
| `preferred_family_order_ip_reject_and_nxdomain` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | FAIL |

## 执行汇总

- 实际发现：6
- 实际执行完成：6
- 通过：0
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

- 原始证据：`/private/tmp/fluxa-core-storage-evidence/test_dns_geoip_contract.json`、对应 `.log`（实际存在时）。
- CI 工件标识：`本地证据目录；未宣称已上传 CI`
- 最新报告被后续变更影响时必须重新执行；检查器会拒绝指纹不一致的旧结果。
- 未运行的 Swift/外部协议或夹具独立目标均保持 NOT RUN；本报告不代表系统 VPN、外部服务端及跨平台已经验收。

## 异常原始输出节选

最小重现：`python3 Tools/test_reports.py --run --suite test_dns_geoip_contract`（Rust 套件）；外部套件使用上方真实命令。

```text

running 6 tests
test admitted_family_fallback_survives_configuration_replacement ... FAILED
test explicit_doh_ignores_formal_route_and_retires_old_resolver ... FAILED
test geoip_ready_failure_atomicity_and_disabled_rules ... FAILED
test named_numeric_dns_failure_never_uses_direct ... FAILED
test ordered_upstreams_get_independent_deadlines ... FAILED
test preferred_family_order_ip_reject_and_nxdomain ... FAILED

failures:

---- admitted_family_fallback_survives_configuration_replacement stdout ----

thread 'admitted_family_fallback_survives_configuration_replacement' (9219712) panicked at tests/test_dns_geoip_contract.rs:424:159:
called `Result::unwrap()` on an `Err` value: InvalidState
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

---- explicit_doh_ignores_formal_route_and_retires_old_resolver stdout ----

thread 'explicit_doh_ignores_formal_route_and_retires_old_resolver' (9219715) panicked at tests/test_dns_geoip_contract.rs:67:10:
called `Result::unwrap()` on an `Err` value: InvalidState

---- geoip_ready_failure_atomicity_and_disabled_rules stdout ----

thread 'geoip_ready_failure_atomicity_and_disabled_rules' (9219716) panicked at tests/test_dns_geoip_contract.rs:114:9:

running 1 test
test geoip_ready_failure_atomicity_and_disabled_rules ... FAILED

failures:

---- geoip_ready_failure_atomicity_and_disabled_rules stdout ----

thread 'geoip_ready_failure_atomicity_and_disabled_rules' (9219718) panicked at tests/test_dns_geoip_contract.rs:133:10:
called `Result::unwrap()` on an `Err` value: InvalidState
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    geoip_ready_failure_atomicity_and_disabled_rules

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 5 filtered out; finished in 0.00s




---- named_numeric_dns_failure_never_uses_direct stdout ----

thread 'named_numeric_dns_failure_never_uses_direct' (9219721) panicked at tests/test_dns_geoip_contract.rs:356:52:
called `Result::unwrap()` on an `Err` value: InvalidState

---- ordered_upstreams_get_independent_deadlines stdout ----

thread 'ordered_upstreams_get_independent_deadlines' (9219722) panicked at tests/test_dns_geoip_contract.rs:323:52:
called `Result::unwrap()` on an `Err` value: InvalidState

---- preferred_family_order_ip_reject_and_nxdomain stdout ----

thread 'preferred_family_order_ip_reject_and_nxdomain' (9219723) panicked at tests/test_dns_geoip_contract.rs:285:52:
called `Result::unwrap()` on an `Err` value: InvalidState


failures:
    admitted_family_fallback_survives_configuration_replacement
    explicit_doh_ignores_formal_route_and_retires_old_resolver
    geoip_ready_failure_atomicity_and_disabled_rules
    named_numeric_dns_failure_never_uses_direct
    ordered_upstreams_get_independent_deadlines
    preferred_family_order_ip_reject_and_nxdomain

test result: FAILED. 0 passed; 6 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.63s


```

节选不替代完整日志；输入及独立预期可追踪到上表同名函数。
