# 测试报告：test_dns

<!-- fluxa-test-report:{"command": ["/private/tmp/fluxa-macos-team-storage/target/debug/deps/test_dns-b0b1359c4a083987", "--test-threads=1"], "exit_code": 0, "duration_seconds": 3.4059137500000105, "status": "PASS", "discovered": ["dnssec_signed_root_valid_tampered_expired_and_untrusted", "encrypted_certificate_and_name_failure_never_downgrade", "explicit_upstream_failover_query_type_and_resource_validation", "fake_ip_concurrent_reuse_is_unique_and_reversible", "fake_ip_dns_does_not_contact_upstream_and_never_claims_dnssec", "fake_ip_range_alignment_capacity_and_exhaustion", "fake_ip_stability_reverse_families_exclusions_and_reset_no_reuse", "invalid_config_names_and_mixed_security_are_rejected", "positive_negative_cache_ttl_zero_expiry_clear_and_isolation", "timeout_wrong_transaction_and_failure_recovery", "udp_tcp_dot_doh_cname_aaaa_and_records"], "results": {"dnssec_signed_root_valid_tampered_expired_and_untrusted": "PASS", "encrypted_certificate_and_name_failure_never_downgrade": "PASS", "explicit_upstream_failover_query_type_and_resource_validation": "PASS", "fake_ip_concurrent_reuse_is_unique_and_reversible": "PASS", "fake_ip_dns_does_not_contact_upstream_and_never_claims_dnssec": "PASS", "fake_ip_range_alignment_capacity_and_exhaustion": "PASS", "fake_ip_stability_reverse_families_exclusions_and_reset_no_reuse": "PASS", "invalid_config_names_and_mixed_security_are_rejected": "PASS", "positive_negative_cache_ttl_zero_expiry_clear_and_isolation": "PASS", "timeout_wrong_transaction_and_failure_recovery": "PASS", "udp_tcp_dot_doh_cname_aaaa_and_records": "PASS"}, "fingerprint": "95a77d0e822027d3d8d91dc72bd23cd1b197af94788923845e5c4d07153812fe", "time": "2026-10-21T05:14:07.529593+08:00", "attempt": 1, "suite": "test_dns", "file": "tests/test_dns.rs", "environment": {"os": "macOS-27.2-arm64-arm-64bit", "architecture": "arm64", "rust": "rustc 1.98.1 (48a229cea 2026-09-01)", "cargo": "cargo 1.98.1 (797e8a9bc 2026-08-05)", "commit": "ca03dbe91d30381600e401e82a2cffa3a5d38c87", "dirty": true, "features": "Cargo 默认 features；未启用 --all-features", "optional_environments": {"FLUXA_DEBUG_TEST_REQUIREMENT": false, "FLUXA_DEBUG_TEST_SWIFT": false, "FLUXA_PROTOCOL_PROFILE": false, "FLUXA_PROTOCOL_FIXTURE": false}, "ci_run_id": null, "ci_run_attempt": null}} -->

## 基本信息

- 测试文件：`tests/test_dns.rs`
- 测试对象：src/dns/mod.rs
- 执行机制：integration
- 测试范围：本文件登记且在当前平台实际发现的全部用例
- 实际命令：`/private/tmp/fluxa-macos-team-storage/target/debug/deps/test_dns-b0b1359c4a083987 --test-threads=1`
- 执行时间：2026-10-21T05:14:07.529593+08:00
- 实际耗时：3.4059137500000105 秒
- 运行环境：`{"os": "macOS-27.2-arm64-arm-64bit", "architecture": "arm64", "rust": "rustc 1.98.1 (48a229cea 2026-09-01)", "cargo": "cargo 1.98.1 (797e8a9bc 2026-08-05)", "commit": "ca03dbe91d30381600e401e82a2cffa3a5d38c87", "dirty": true, "features": "Cargo 默认 features；未启用 --all-features", "optional_environments": {"FLUXA_DEBUG_TEST_REQUIREMENT": false, "FLUXA_DEBUG_TEST_SWIFT": false, "FLUXA_PROTOCOL_PROFILE": false, "FLUXA_PROTOCOL_FIXTURE": false}, "ci_run_id": null, "ci_run_attempt": null}`
- 代码/测试/配置指纹：`95a77d0e822027d3d8d91dc72bd23cd1b197af94788923845e5c4d07153812fe`
- 总体状态：**PASS**
- 原始进程退出状态：0

## 实际用例与结果

| 实际发现的用例 | 独立目标 | 状态 |
| --- | --- | --- |
| `dnssec_signed_root_valid_tampered_expired_and_untrusted` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `encrypted_certificate_and_name_failure_never_downgrade` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `explicit_upstream_failover_query_type_and_resource_validation` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `fake_ip_concurrent_reuse_is_unique_and_reversible` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `fake_ip_dns_does_not_contact_upstream_and_never_claims_dnssec` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `fake_ip_range_alignment_capacity_and_exhaustion` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `fake_ip_stability_reverse_families_exclusions_and_reset_no_reuse` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `invalid_config_names_and_mixed_security_are_rejected` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `positive_negative_cache_ttl_zero_expiry_clear_and_isolation` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `timeout_wrong_transaction_and_failure_recovery` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `udp_tcp_dot_doh_cname_aaaa_and_records` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |

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

- 原始证据：`/private/tmp/fluxa-team-final-evidence/test_dns.json`、对应 `.log`（实际存在时）。
- CI 工件标识：`本地证据目录；未宣称已上传 CI`
- 最新报告被后续变更影响时必须重新执行；检查器会拒绝指纹不一致的旧结果。
- 未运行的 Swift/外部协议或夹具独立目标均保持 NOT RUN；本报告不代表系统 VPN、外部服务端及跨平台已经验收。
