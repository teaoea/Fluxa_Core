# 测试报告：test_apple_platforms

<!-- fluxa-test-report:{"command": ["python3", "tests/test_apple_platforms.py", "-v", "-b"], "exit_code": 0, "duration_seconds": 0.2513835, "status": "PASS", "discovered": ["test_supported_matrix", "test_excluded_targets_rejected_before_build", "test_deployment_environment_isolated", "test_existing_output_preserved", "test_local_only_entrypoints", "test_package_names_and_contents", "test_packages_reject_excluded_architecture"], "results": {"test_supported_matrix": "PASS", "test_excluded_targets_rejected_before_build": "PASS", "test_deployment_environment_isolated": "PASS", "test_existing_output_preserved": "PASS", "test_local_only_entrypoints": "PASS", "test_package_names_and_contents": "PASS", "test_packages_reject_excluded_architecture": "PASS"}, "fingerprint": "23e34507a613c48170087b90501299e7bca9fe06332f55ac09e5a298857e3243", "time": "2026-10-21T20:02:00.841291+08:00", "scope": "逐项核对实际测试框架输出与本文件登记函数；零用例或缺少实际结果不计通过。", "note": "以真实外部进程退出状态验证程序整体；编译成功不代表测试已运行。内部断言数量未知，不与 Rust 函数数量合并。", "attempt": 1, "suite": "test_apple_platforms", "file": "tests/test_apple_platforms.py", "environment": {"os": "macOS-27.2-arm64-arm-64bit", "architecture": "arm64", "rust": "rustc 1.98.1 (48a229cea 2026-09-01)", "cargo": "cargo 1.98.1 (797e8a9bc 2026-08-05)", "commit": "1a798addfabd84db3e26bdccf2794318eba7e1ce", "dirty": true, "features": "Cargo 默认 features；未启用 --all-features", "optional_environments": {"FLUXA_DEBUG_TEST_REQUIREMENT": false, "FLUXA_DEBUG_TEST_SWIFT": false, "FLUXA_PROTOCOL_PROFILE": false, "FLUXA_PROTOCOL_FIXTURE": false}, "ci_run_id": null, "ci_run_attempt": null}} -->

## 基本信息

- 测试文件：`tests/test_apple_platforms.py`
- 测试对象：Tools/build-apple.py；build-apple.sh；build-macos.sh；src/lib.rs：Apple 最低系统与 ARM64 构建契约
- 执行机制：python_unit
- 测试范围：逐项核对实际测试框架输出与本文件登记函数；零用例或缺少实际结果不计通过。
- 实际命令：`python3 tests/test_apple_platforms.py -v -b`
- 执行时间：2026-10-21T20:02:00.841291+08:00
- 实际耗时：0.2513835 秒
- 运行环境：`{"os": "macOS-27.2-arm64-arm-64bit", "architecture": "arm64", "rust": "rustc 1.98.1 (48a229cea 2026-09-01)", "cargo": "cargo 1.98.1 (797e8a9bc 2026-08-05)", "commit": "1a798addfabd84db3e26bdccf2794318eba7e1ce", "dirty": true, "features": "Cargo 默认 features；未启用 --all-features", "optional_environments": {"FLUXA_DEBUG_TEST_REQUIREMENT": false, "FLUXA_DEBUG_TEST_SWIFT": false, "FLUXA_PROTOCOL_PROFILE": false, "FLUXA_PROTOCOL_FIXTURE": false}, "ci_run_id": null, "ci_run_attempt": null}`
- 代码/测试/配置指纹：`23e34507a613c48170087b90501299e7bca9fe06332f55ac09e5a298857e3243`
- 总体状态：**PASS**
- 原始进程退出状态：0

## 实际用例与结果

| 实际发现的用例 | 独立目标 | 状态 |
| --- | --- | --- |
| `test_supported_matrix` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `test_excluded_targets_rejected_before_build` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `test_deployment_environment_isolated` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `test_existing_output_preserved` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `test_local_only_entrypoints` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `test_package_names_and_contents` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `test_packages_reject_excluded_architecture` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |

## 执行汇总

- 实际发现：7
- 实际执行完成：7
- 通过：7
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

- 原始证据：`/private/tmp/fluxa-readonly-policy-python/test_apple_platforms.json`、对应 `.log`（实际存在时）。
- CI 工件标识：`本地证据目录；未宣称已上传 CI`
- 最新报告被后续变更影响时必须重新执行；检查器会拒绝指纹不一致的旧结果。
- 未运行的 Swift/外部协议或夹具独立目标均保持 NOT RUN；本报告不代表系统 VPN、外部服务端及跨平台已经验收。

## 本轮补充：默认仅 macOS 的真实本地产物

- 当前构建列表只启用 macOS arm64；8 项 Python 契约测试包括默认目标、保留完整支持矩阵、排除平台、部署隔离、命名与本地白名单；不将这些工具测试视为其他平台已经构建。
- 实际使用无 --platform 的默认命令，输出 `Release/2.8.0-dev/1a798ad/FluxaEngine.xcframework` 及唯一 `fluxa-core-macos-arm64.zip`；未构建本轮其他目标。
- 源码完整提交、dirty 状态、架构、最低系统及 ZIP SHA-256 见同目录 artifacts/manifest.json。dirty=true 如实保留用户原有文档修改和报告变更；编译相关 src/include/Cargo/中英文模板与该提交逐字匹配。
- 实际检查 569 个 Mach-O 对象的 LC_BUILD_VERSION，平台均为 macOS；对象最低版本为 11.0 或 13.0，不高于规范 13.0。Framework 最低版本为 13.0，实际架构仅 arm64，内部库名 fluxa_core，ZIP 只含该切片。
- 证据：`/private/tmp/fluxa-readonly-ffi/macos-package-verification.json`、`/private/tmp/fluxa-core-macos-1a798ad-build.log`。最终框架真实 Swift ABI 验证见 test_ffi.md。
- 在用户新构建列表要求之前曾生成 7740625 的其他 Apple 本地切片；保留为历史文件，不作为当前交付或继续构建授权。当前无二进制上传。
- 未验证 macOS 13 实际运行、移动真机、签名／公证、NE 和系统 VPN；其余平台仅保留开发兼容目标。
