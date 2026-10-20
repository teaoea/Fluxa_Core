# 测试报告：test_configuration_plugins

<!-- fluxa-test-report:{"command": ["/Users/klee/Documents/FLuxa_Core/target/debug/deps/libfluxa_core-00177ba80b5d6f22", "::test_configuration_plugins::", "--test-threads=1"], "exit_code": 0, "duration_seconds": 1.2589706669999998, "status": "PASS", "discovered": ["configuration::plugins::test_configuration_plugins::automatic_refresh_requires_declared_local_slot_and_does_not_repeat_same_slot", "configuration::plugins::test_configuration_plugins::disabled_new_entry_and_read_only_reuse_never_download_or_write", "configuration::plugins::test_configuration_plugins::imports_namespaced_scripts_and_main_rewrites_win", "configuration::plugins::test_configuration_plugins::interrupted_resource_transaction_recovers_single_swap_and_accepts_complete_pair", "configuration::plugins::test_configuration_plugins::invalid_plugin_or_abandoned_batch_preserves_old_resources", "configuration::plugins::test_configuration_plugins::main_mitm_bypass_and_master_switch_override_plugin_decryption", "configuration::plugins::test_configuration_plugins::missing_script_excludes_only_bad_rewrite_and_keeps_native_rules", "configuration::plugins::test_configuration_plugins::only_strict_plugin_fields_and_single_script_mapping_are_allowed", "configuration::plugins::test_configuration_plugins::recovery_journal_cannot_name_external_paths", "configuration::plugins::test_configuration_plugins::reordering_moves_ids_files_and_priority_without_redownload"], "results": {"configuration::plugins::test_configuration_plugins::automatic_refresh_requires_declared_local_slot_and_does_not_repeat_same_slot": "PASS", "configuration::plugins::test_configuration_plugins::disabled_new_entry_and_read_only_reuse_never_download_or_write": "PASS", "configuration::plugins::test_configuration_plugins::imports_namespaced_scripts_and_main_rewrites_win": "PASS", "configuration::plugins::test_configuration_plugins::interrupted_resource_transaction_recovers_single_swap_and_accepts_complete_pair": "PASS", "configuration::plugins::test_configuration_plugins::invalid_plugin_or_abandoned_batch_preserves_old_resources": "PASS", "configuration::plugins::test_configuration_plugins::main_mitm_bypass_and_master_switch_override_plugin_decryption": "PASS", "configuration::plugins::test_configuration_plugins::missing_script_excludes_only_bad_rewrite_and_keeps_native_rules": "PASS", "configuration::plugins::test_configuration_plugins::only_strict_plugin_fields_and_single_script_mapping_are_allowed": "PASS", "configuration::plugins::test_configuration_plugins::recovery_journal_cannot_name_external_paths": "PASS", "configuration::plugins::test_configuration_plugins::reordering_moves_ids_files_and_priority_without_redownload": "PASS"}, "fingerprint": "8160ed086f1f4735b68e69bd474ca8bf413613da306a90bdbb09472d087b13d8", "time": "2026-10-21T05:33:11.104801+08:00", "attempt": 1, "suite": "test_configuration_plugins", "file": "tests/test_configuration_plugins.rs", "environment": {"os": "macOS-27.2-arm64-arm-64bit", "architecture": "arm64", "rust": "rustc 1.98.1 (48a229cea 2026-09-01)", "cargo": "cargo 1.98.1 (797e8a9bc 2026-08-05)", "commit": "a25d89ca8b52adfe4a1f94703b242fe1e4c44e4c", "dirty": false, "features": "Cargo 默认 features；未启用 --all-features", "optional_environments": {"FLUXA_DEBUG_TEST_REQUIREMENT": false, "FLUXA_DEBUG_TEST_SWIFT": false, "FLUXA_PROTOCOL_PROFILE": false, "FLUXA_PROTOCOL_FIXTURE": false}, "ci_run_id": null, "ci_run_attempt": null}} -->

## 基本信息

- 测试文件：`tests/test_configuration_plugins.rs`
- 测试对象：src/configuration/plugins.rs、src/configuration/rewrites/execution.rs、src/script/worker.rs
- 执行机制：unit
- 测试范围：本文件登记且在当前平台实际发现的全部用例
- 实际命令：`/Users/klee/Documents/FLuxa_Core/target/debug/deps/libfluxa_core-00177ba80b5d6f22 ::test_configuration_plugins:: --test-threads=1`
- 执行时间：2026-10-21T05:33:11.104801+08:00
- 实际耗时：1.2589706669999998 秒
- 运行环境：`{"os": "macOS-27.2-arm64-arm-64bit", "architecture": "arm64", "rust": "rustc 1.98.1 (48a229cea 2026-09-01)", "cargo": "cargo 1.98.1 (797e8a9bc 2026-08-05)", "commit": "a25d89ca8b52adfe4a1f94703b242fe1e4c44e4c", "dirty": false, "features": "Cargo 默认 features；未启用 --all-features", "optional_environments": {"FLUXA_DEBUG_TEST_REQUIREMENT": false, "FLUXA_DEBUG_TEST_SWIFT": false, "FLUXA_PROTOCOL_PROFILE": false, "FLUXA_PROTOCOL_FIXTURE": false}, "ci_run_id": null, "ci_run_attempt": null}`
- 代码/测试/配置指纹：`8160ed086f1f4735b68e69bd474ca8bf413613da306a90bdbb09472d087b13d8`
- 总体状态：**PASS**
- 原始进程退出状态：0

## 实际用例与结果

| 实际发现的用例 | 独立目标 | 状态 |
| --- | --- | --- |
| `configuration::plugins::test_configuration_plugins::automatic_refresh_requires_declared_local_slot_and_does_not_repeat_same_slot` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `configuration::plugins::test_configuration_plugins::disabled_new_entry_and_read_only_reuse_never_download_or_write` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `configuration::plugins::test_configuration_plugins::imports_namespaced_scripts_and_main_rewrites_win` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `configuration::plugins::test_configuration_plugins::interrupted_resource_transaction_recovers_single_swap_and_accepts_complete_pair` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `configuration::plugins::test_configuration_plugins::invalid_plugin_or_abandoned_batch_preserves_old_resources` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `configuration::plugins::test_configuration_plugins::main_mitm_bypass_and_master_switch_override_plugin_decryption` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `configuration::plugins::test_configuration_plugins::missing_script_excludes_only_bad_rewrite_and_keeps_native_rules` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `configuration::plugins::test_configuration_plugins::only_strict_plugin_fields_and_single_script_mapping_are_allowed` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `configuration::plugins::test_configuration_plugins::recovery_journal_cannot_name_external_paths` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `configuration::plugins::test_configuration_plugins::reordering_moves_ids_files_and_priority_without_redownload` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |

## 执行汇总

- 实际发现：10
- 实际执行完成：10
- 通过：10
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

- 原始证据：`/private/tmp/fluxa-core-2.8-evidence/rust/test_configuration_plugins.json`、对应 `.log`（实际存在时）。
- CI 工件标识：`本地证据目录；未宣称已上传 CI`
- 最新报告被后续变更影响时必须重新执行；检查器会拒绝指纹不一致的旧结果。
- 未运行的 Swift/外部协议或夹具独立目标均保持 NOT RUN；本报告不代表系统 VPN、外部服务端及跨平台已经验收。


## 本轮 L1–L4 补充记录（2026-10-21）

- 环境：macOS 27.2 ARM64、Rust 1.98.1；确定源码提交 `a25d89ca8b52adfe4a1f94703b242fe1e4c44e4c`，独立干净检出开始执行，未改业务源码。最终指纹 `8160ed086f1f4735b68e69bd474ca8bf413613da306a90bdbb09472d087b13d8`。完整 Rust 实际 553 PASS、1 项外部协议 IGNORED/SKIPPED；发布控制流 19、报告工具 9 项通过。原始退出码分别为 0。
- L1：格式、全目标 Clippy -D warnings、14 项错误契约通过。既有编号、含义和 Schema 未改；完整共享容器修复来自已整合的远程 dev。
- L2：本报告列出的插件用例实际通过；工作线程 7、脚本消息协议 4、原生复写执行 16 与真实 HTTP/TLS 管线 7 分别见对应报告。只读路径不下载/写入；无调度时间不自动刷新，已完成日期不重复下载。
- L3：固定独立预期验证主配置优先、插件 1/2 换序后的目录和优先级、旧快照保留原脚本、缺失脚本不遮挡后续 418 规则、主配置 bypass 压过插件解密声明、单边目录交换恢复及日志路径限制。真实开发签名 Swift 宿主从系统共享容器读取无 .js 后缀资源，在 256 KiB 线程经 C ABI 修改回环请求/响应，缓存清理后读到更新，忙碌请求期间清理返回实际取消 9002；缺授权宿主仍拒绝 9001。未修改 App、默认主配置或系统代理/VPN。
- L4：Rust 原始日志 `/private/tmp/fluxa-core-2.8-evidence/rust`；实际签名 FFI 源码在 `/private/tmp/fluxa-core-2.8-ffi/check.swift`，本次成功日志为 `swift-concurrent2.log` 与 `swift-unauthorized.log`。发布门禁摘要为同证据目录 `verification.json`。这些证据不表示最低系统、Network Extension、系统 VPN、外部独立协议对端或播放器去广告已验收。

### 失败历史保全

首轮插件夹具未配置下载路由导致 InvalidState，修复夹具后继续原断言；目录候选非同级导致原子交换 InvalidInput，已改同级提交；内部目录表使用元组不能被严格解析器回读，已改具名结构。早期 FFI 回归发现空缓存关闭强制 Pending 与 unit 变体忽略字段，已修复并用原断言验证；最后全量 FFI 为 19/19 PASS。曾出现编译/Clippy 错误，原始日志保留于 `/private/tmp/fluxa-when-work`，不计为通过。独立 Swift 并发夹具第二次创建已存在 js 目录出现 Cocoa 516，在进入 Core 调用前退出 133；修复夹具的幂等检查后成功，失败日志保留为 `swift-concurrent.log`。最初普通沙箱回环服务绑定被 PermissionDenied 拒绝，授权的本机回环环境重跑通过；不把该环境拒绝视为业务缺陷或从历史删除。
