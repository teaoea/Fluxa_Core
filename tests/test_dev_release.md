# 测试报告：test_dev_release

<!-- fluxa-test-report:{"command": ["python3", "tests/test_dev_release.py", "-v", "-b"], "exit_code": 0, "duration_seconds": 2.675362625, "status": "PASS", "discovered": ["test_dirty_source_never_publishes", "test_wrong_source_never_publishes", "test_missing_verification_never_builds", "test_wrong_provenance_never_publishes", "test_unknown_manifest_fields_never_become_public", "test_implementation_sources_never_become_public", "test_checksum_failure_never_publishes", "test_symlink_input_never_publishes", "test_new_release_stays_prerelease_and_not_latest", "test_existing_release_is_verified_without_overwrite", "test_failed_upload_never_publishes_draft", "test_download_mismatch_never_publishes_draft", "test_conflicting_tag_is_not_moved", "test_api_auth_failure_is_not_missing_release", "test_policy_blocks_crates_and_source_release", "test_framework_preserves_current_license_files", "test_actions_reference_is_rejected", "test_manual_verification_rejects_stale_and_incomplete_evidence", "test_prepare_does_not_require_upload_credentials"], "results": {"test_dirty_source_never_publishes": "PASS", "test_wrong_source_never_publishes": "PASS", "test_missing_verification_never_builds": "PASS", "test_wrong_provenance_never_publishes": "PASS", "test_unknown_manifest_fields_never_become_public": "PASS", "test_implementation_sources_never_become_public": "PASS", "test_checksum_failure_never_publishes": "PASS", "test_symlink_input_never_publishes": "PASS", "test_new_release_stays_prerelease_and_not_latest": "PASS", "test_existing_release_is_verified_without_overwrite": "PASS", "test_failed_upload_never_publishes_draft": "PASS", "test_download_mismatch_never_publishes_draft": "PASS", "test_conflicting_tag_is_not_moved": "PASS", "test_api_auth_failure_is_not_missing_release": "PASS", "test_policy_blocks_crates_and_source_release": "PASS", "test_framework_preserves_current_license_files": "PASS", "test_actions_reference_is_rejected": "PASS", "test_manual_verification_rejects_stale_and_incomplete_evidence": "PASS", "test_prepare_does_not_require_upload_credentials": "PASS"}, "fingerprint": "195b9d9bf4f4f354ce9a8ad08fd3ffd134d1d1c9f76083014646048b21a9d89b", "time": "2026-10-21T13:02:25.674728+08:00", "scope": "逐项核对实际测试框架输出与本文件登记函数；零用例或缺少实际结果不计通过。", "note": "以真实外部进程退出状态验证程序整体；编译成功不代表测试已运行。内部断言数量未知，不与 Rust 函数数量合并。", "attempt": 1, "suite": "test_dev_release", "file": "tests/test_dev_release.py", "environment": {"os": "macOS-27.2-arm64-arm-64bit", "architecture": "arm64", "rust": "rustc 1.98.1 (48a229cea 2026-09-01)", "cargo": "cargo 1.98.1 (797e8a9bc 2026-08-05)", "commit": "8b17fa48772345e5ef38f042c1aa1b88d4555dbc", "dirty": true, "features": "Cargo 默认 features；未启用 --all-features", "optional_environments": {"FLUXA_DEBUG_TEST_REQUIREMENT": false, "FLUXA_DEBUG_TEST_SWIFT": false, "FLUXA_PROTOCOL_PROFILE": false, "FLUXA_PROTOCOL_FIXTURE": false}, "ci_run_id": null, "ci_run_attempt": null}} -->

## 基本信息

- 测试文件：`tests/test_dev_release.py`
- 测试对象：Tools/dev-release.sh；Tools/publish-core-binaries.py；Tools/verify-manual-release.py；build-macos.sh；.github/workflows/tests.yml：本机手动发布与公开白名单
- 执行机制：python_unit
- 测试范围：逐项核对实际测试框架输出与本文件登记函数；零用例或缺少实际结果不计通过。
- 实际命令：`python3 tests/test_dev_release.py -v -b`
- 执行时间：2026-10-21T13:02:25.674728+08:00
- 实际耗时：2.675362625 秒
- 运行环境：`{"os": "macOS-27.2-arm64-arm-64bit", "architecture": "arm64", "rust": "rustc 1.98.1 (48a229cea 2026-09-01)", "cargo": "cargo 1.98.1 (797e8a9bc 2026-08-05)", "commit": "8b17fa48772345e5ef38f042c1aa1b88d4555dbc", "dirty": true, "features": "Cargo 默认 features；未启用 --all-features", "optional_environments": {"FLUXA_DEBUG_TEST_REQUIREMENT": false, "FLUXA_DEBUG_TEST_SWIFT": false, "FLUXA_PROTOCOL_PROFILE": false, "FLUXA_PROTOCOL_FIXTURE": false}, "ci_run_id": null, "ci_run_attempt": null}`
- 代码/测试/配置指纹：`195b9d9bf4f4f354ce9a8ad08fd3ffd134d1d1c9f76083014646048b21a9d89b`
- 总体状态：**PASS**
- 原始进程退出状态：0

## 实际用例与结果

| 实际发现的用例 | 独立目标 | 状态 |
| --- | --- | --- |
| `test_dirty_source_never_publishes` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `test_wrong_source_never_publishes` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `test_missing_verification_never_builds` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `test_wrong_provenance_never_publishes` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `test_unknown_manifest_fields_never_become_public` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `test_implementation_sources_never_become_public` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `test_checksum_failure_never_publishes` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `test_symlink_input_never_publishes` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `test_new_release_stays_prerelease_and_not_latest` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `test_existing_release_is_verified_without_overwrite` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `test_failed_upload_never_publishes_draft` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `test_download_mismatch_never_publishes_draft` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `test_conflicting_tag_is_not_moved` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `test_api_auth_failure_is_not_missing_release` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `test_policy_blocks_crates_and_source_release` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `test_framework_preserves_current_license_files` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `test_actions_reference_is_rejected` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `test_manual_verification_rejects_stale_and_incomplete_evidence` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `test_prepare_does_not_require_upload_credentials` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |

## 执行汇总

- 实际发现：19
- 实际执行完成：19
- 通过：19
- 失败：0
- 忽略/跳过：0
- 未取得完整执行结果：0

## 失败、异常与限制

以真实外部进程退出状态验证程序整体；编译成功不代表测试已运行。内部断言数量未知，不与 Rust 函数数量合并。
未覆盖范围以本文件实际断言和前置条件为界，不宣称全模块或全平台覆盖。
不自动重试；若手动重跑，旧报告及原始退出结果保存在证据目录。缺少执行结果时不推定成功。

## 独立 L1–L4 审查

- L1：本次测试构建结果见 build 日志；格式/Clippy 等需另行记录，不自动推断通过。
- L2：已登记用例按 Cargo 工件、发现列表与显式文件映射归属；实际结果见上表。
- L3：未由报告生成器验证；迁移保留断言不等于原业务预期已全面证明，需独立语义审查。
- L4：原始日志、机器结果、版本及指纹可在下述证据核对；未上传的本地证据不冒充 CI 工件。

## 证据及后续事项

- 原始证据：`/private/tmp/fluxa-core-platform-formatted-python-20261021/test_dev_release.json`、对应 `.log`（实际存在时）。
- CI 工件标识：`本地证据目录；未宣称已上传 CI`
- 最新报告被后续变更影响时必须重新执行；检查器会拒绝指纹不一致的旧结果。
- 未运行的 Swift/外部协议或夹具独立目标均保持 NOT RUN；本报告不代表系统 VPN、外部服务端及跨平台已经验收。
