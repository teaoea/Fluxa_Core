# 测试报告：test_dev_release

<!-- fluxa-test-report:{"command": ["python3", "tests/test_dev_release.py", "-v", "-b"], "exit_code": 0, "duration_seconds": 3.451389708, "status": "PASS", "discovered": ["test_dirty_source_never_publishes", "test_wrong_source_never_publishes", "test_existing_release_is_verified_without_overwrite", "test_existing_checksum_failure_does_not_overwrite", "test_new_release_stays_prerelease_and_not_latest", "test_failed_upload_never_publishes_draft"], "results": {"test_dirty_source_never_publishes": "PASS", "test_wrong_source_never_publishes": "PASS", "test_existing_release_is_verified_without_overwrite": "PASS", "test_existing_checksum_failure_does_not_overwrite": "PASS", "test_new_release_stays_prerelease_and_not_latest": "PASS", "test_failed_upload_never_publishes_draft": "PASS"}, "fingerprint": "0ab210900f6c1a572f9d17ed7b2b300142a30c1517e24f0c54af051832960502", "time": "2026-10-17T21:24:36.994003+08:00", "scope": "逐项核对实际测试框架输出与本文件登记函数；零用例或缺少实际结果不计通过。", "note": "以真实外部进程退出状态验证程序整体；编译成功不代表测试已运行。内部断言数量未知，不与 Rust 函数数量合并。", "attempt": 1, "suite": "test_dev_release", "file": "tests/test_dev_release.py", "environment": {"os": "macOS-27.2-arm64-arm-64bit", "architecture": "arm64", "rust": "rustc 1.98.1 (48a229cea 2026-09-01)", "cargo": "cargo 1.98.1 (797e8a9bc 2026-08-05)", "commit": "4cd3dfe7adfa6bec4c12cf8ba3abc4db7b75e575", "dirty": true, "features": "Cargo 默认 features；未启用 --all-features", "optional_environments": {"FLUXA_DEBUG_TEST_REQUIREMENT": false, "FLUXA_DEBUG_TEST_SWIFT": false, "FLUXA_PROTOCOL_PROFILE": false, "FLUXA_PROTOCOL_FIXTURE": false}, "ci_run_id": null, "ci_run_attempt": null}} -->

## 基本信息

- 测试文件：`tests/test_dev_release.py`
- 测试对象：Tools/dev-release.sh；.github/workflows/tests.yml：dev 不稳定附件控制流
- 执行机制：python_unit
- 测试范围：逐项核对实际测试框架输出与本文件登记函数；零用例或缺少实际结果不计通过。
- 实际命令：`python3 tests/test_dev_release.py -v -b`
- 执行时间：2026-10-17T21:24:36.994003+08:00
- 实际耗时：3.451389708 秒
- 运行环境：`{"os": "macOS-27.2-arm64-arm-64bit", "architecture": "arm64", "rust": "rustc 1.98.1 (48a229cea 2026-09-01)", "cargo": "cargo 1.98.1 (797e8a9bc 2026-08-05)", "commit": "4cd3dfe7adfa6bec4c12cf8ba3abc4db7b75e575", "dirty": true, "features": "Cargo 默认 features；未启用 --all-features", "optional_environments": {"FLUXA_DEBUG_TEST_REQUIREMENT": false, "FLUXA_DEBUG_TEST_SWIFT": false, "FLUXA_PROTOCOL_PROFILE": false, "FLUXA_PROTOCOL_FIXTURE": false}, "ci_run_id": null, "ci_run_attempt": null}`
- 代码/测试/配置指纹：`0ab210900f6c1a572f9d17ed7b2b300142a30c1517e24f0c54af051832960502`
- 总体状态：**PASS**
- 原始进程退出状态：0

## 实际用例与结果

| 实际发现的用例 | 独立目标 | 状态 |
| --- | --- | --- |
| `test_dirty_source_never_publishes` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `test_wrong_source_never_publishes` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `test_existing_release_is_verified_without_overwrite` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `test_existing_checksum_failure_does_not_overwrite` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `test_new_release_stays_prerelease_and_not_latest` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `test_failed_upload_never_publishes_draft` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |

## 执行汇总

- 实际发现：6
- 实际执行完成：6
- 通过：6
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

- 原始证据：`/private/tmp/fluxa-pmtu-accepted-release/test_dev_release.json`、对应 `.log`（实际存在时）。
- CI 工件标识：`本地证据目录；未宣称已上传 CI`
- 最新报告被后续变更影响时必须重新执行；检查器会拒绝指纹不一致的旧结果。
- 未运行的 Swift/外部协议或夹具独立目标均保持 NOT RUN；本报告不代表系统 VPN、外部服务端及跨平台已经验收。
