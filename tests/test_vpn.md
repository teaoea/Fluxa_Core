# 测试报告：test_vpn

<!-- fluxa-test-report:{"command": ["/private/tmp/fluxa-tcp-latest/target/debug/deps/libfluxa_core-60a1bb1b1376270f", "::test_vpn::", "--test-threads=1"], "exit_code": 0, "duration_seconds": 0.4729697080000079, "status": "PASS", "discovered": ["vpn::test_vpn::close_cancels_download_and_releases_slot_with_old_reference_alive", "vpn::test_vpn::failure_of_second_source_never_publishes_partial_candidate", "vpn::test_vpn::geoip_failure_is_error_not_disabled_success", "vpn::test_vpn::invalid_input_and_unsupported_modes_fail_without_partial_candidate", "vpn::test_vpn::local_candidate_is_not_activation_and_does_not_replace_live_rules", "vpn::test_vpn::remote_sources_are_memory_only_and_keep_policy", "vpn::test_vpn::slots_are_bounded_before_background_work", "vpn::test_vpn::total_deadline_has_its_own_error_and_releases_resources"], "results": {"vpn::test_vpn::close_cancels_download_and_releases_slot_with_old_reference_alive": "PASS", "vpn::test_vpn::failure_of_second_source_never_publishes_partial_candidate": "PASS", "vpn::test_vpn::geoip_failure_is_error_not_disabled_success": "PASS", "vpn::test_vpn::invalid_input_and_unsupported_modes_fail_without_partial_candidate": "PASS", "vpn::test_vpn::local_candidate_is_not_activation_and_does_not_replace_live_rules": "PASS", "vpn::test_vpn::remote_sources_are_memory_only_and_keep_policy": "PASS", "vpn::test_vpn::slots_are_bounded_before_background_work": "PASS", "vpn::test_vpn::total_deadline_has_its_own_error_and_releases_resources": "PASS"}, "fingerprint": "84a5bd1f61bb1f3b023e6c033ed2dc855855aed867790eb1315f82f0d514eb96", "time": "2026-10-19T00:46:41.044044+08:00", "attempt": 1, "suite": "test_vpn", "file": "tests/test_vpn.rs", "environment": {"os": "macOS-27.2-arm64-arm-64bit", "architecture": "arm64", "rust": "rustc 1.98.1 (48a229cea 2026-09-01)", "cargo": "cargo 1.98.1 (797e8a9bc 2026-08-05)", "commit": "32881558a5945f704ddc169892fe486f2dce9575", "dirty": true, "features": "Cargo 默认 features；未启用 --all-features", "optional_environments": {"FLUXA_DEBUG_TEST_REQUIREMENT": false, "FLUXA_DEBUG_TEST_SWIFT": false, "FLUXA_PROTOCOL_PROFILE": false, "FLUXA_PROTOCOL_FIXTURE": false}, "ci_run_id": null, "ci_run_attempt": null}} -->

## 基本信息

- 测试文件：`tests/test_vpn.rs`
- 测试对象：src/vpn.rs、src/providers.rs、src/configuration.rs、src/outbound/configuration.rs
- 执行机制：unit
- 测试范围：本文件登记且在当前平台实际发现的全部用例
- 实际命令：`/private/tmp/fluxa-tcp-latest/target/debug/deps/libfluxa_core-60a1bb1b1376270f ::test_vpn:: --test-threads=1`
- 执行时间：2026-10-19T00:46:41.044044+08:00
- 实际耗时：0.4729697080000079 秒
- 运行环境：`{"os": "macOS-27.2-arm64-arm-64bit", "architecture": "arm64", "rust": "rustc 1.98.1 (48a229cea 2026-09-01)", "cargo": "cargo 1.98.1 (797e8a9bc 2026-08-05)", "commit": "32881558a5945f704ddc169892fe486f2dce9575", "dirty": true, "features": "Cargo 默认 features；未启用 --all-features", "optional_environments": {"FLUXA_DEBUG_TEST_REQUIREMENT": false, "FLUXA_DEBUG_TEST_SWIFT": false, "FLUXA_PROTOCOL_PROFILE": false, "FLUXA_PROTOCOL_FIXTURE": false}, "ci_run_id": null, "ci_run_attempt": null}`
- 代码/测试/配置指纹：`84a5bd1f61bb1f3b023e6c033ed2dc855855aed867790eb1315f82f0d514eb96`
- 总体状态：**PASS**
- 原始进程退出状态：0

## 实际用例与结果

| 实际发现的用例 | 独立目标 | 状态 |
| --- | --- | --- |
| `vpn::test_vpn::close_cancels_download_and_releases_slot_with_old_reference_alive` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `vpn::test_vpn::failure_of_second_source_never_publishes_partial_candidate` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `vpn::test_vpn::geoip_failure_is_error_not_disabled_success` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `vpn::test_vpn::invalid_input_and_unsupported_modes_fail_without_partial_candidate` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `vpn::test_vpn::local_candidate_is_not_activation_and_does_not_replace_live_rules` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `vpn::test_vpn::remote_sources_are_memory_only_and_keep_policy` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `vpn::test_vpn::slots_are_bounded_before_background_work` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `vpn::test_vpn::total_deadline_has_its_own_error_and_releases_resources` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |

## 执行汇总

- 实际发现：8
- 实际执行完成：8
- 通过：8
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

- 原始证据：`/private/tmp/fluxa-vpn-candidate-accepted/test_vpn.json`、对应 `.log`（实际存在时）。
- CI 工件标识：`本地证据目录；未宣称已上传 CI`
- 最新报告被后续变更影响时必须重新执行；检查器会拒绝指纹不一致的旧结果。
- 未运行的 Swift/外部协议或夹具独立目标均保持 NOT RUN；本报告不代表系统 VPN、外部服务端及跨平台已经验收。

## 本阶段独立语义复核及历史保全

- L1：cargo fmt --check、cargo check、全部目标严格 Clippy 通过；未新增 unsafe。首轮 Clippy 发现测试头文档缺少段落间隔，已修正并重跑，未禁用 lint。
- L2：本次固定源码全量 Rust 474 PASS、1 IGNORED/SKIPPED，退出 0；本套件 8/8。44 项错误契约及同名报告校验通过。真实 Swift 源码 FFI 单独登记在 Fluxa 的 test_ffi_vpn.md，不能用它替代固定远程交付物验收。
- L3：按 VPN 规范核对：候选使用独立管理器、无磁盘资产/定时任务；现有 configuration_apply 不放开 TUN；无节点和资产就绪都不产生接管权限；配置/多来源/GeoIP 失败不发布部分候选；关闭等待在途解析编译和资源释放，旧引用不保留活动槽。DNS 实际可用性、必要代理数据路径、FD/平台预算、可信授权和系统接管仍未实现，不能宣称完整 Preflight 已完成。
- L4：/private/tmp/fluxa-vpn-candidate-accepted/run.json 退出 0，源码指纹及逐项结果可查；全量报告由显式套件映射生成。CLI/最低系统/真实系统 VPN 验收未运行。
- 历史 /private/tmp/fluxa-vpn-candidate-first：1 PASS、6 FAIL，含沙箱拒绝回环和系统证书；/private/tmp/fluxa-vpn-candidate-authorized：5 PASS、2 FAIL，暴露测试请求没有使用夹具支持的 /provider- 路径。随后修正路径，没有弱化数据断言。
- /private/tmp/fluxa-vpn-candidate-pathfix：7/7 执行通过，但运行期间新增总期限断言导致源码指纹变化，进程整体退出 3（STALE），不作为最终通过证据。
- /private/tmp/fluxa-vpn-candidate-full：全量首次失败，ABI 精确能力位预期未同步新位 29；保留首轮结果。修正完整精确掩码后重跑 accepted，未改成宽松包含断言。
- 外部协议对端未提供，test_protocol 独立互通用例明确跳过；不将本轮回环或候选准备当作协议外部验收。
