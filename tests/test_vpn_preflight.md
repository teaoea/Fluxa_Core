# 测试报告：test_vpn_preflight

<!-- nodus-test-report:{"command": ["/Users/klee/Documents/Nodus Core/target/debug/deps/nodus_core-e109a20f5477cf6e", "::test_vpn_preflight::", "--test-threads=1"], "exit_code": 0, "duration_seconds": 1.2737477089999985, "status": "PASS", "discovered": ["vpn::preflight::test_vpn_preflight::cancellation_and_deadline_stop_probes", "vpn::preflight::test_vpn_preflight::compiled_requirements_respect_mode_and_sources", "vpn::preflight::test_vpn_preflight::errors_redirects_and_rewrites_cannot_fake_success", "vpn::preflight::test_vpn_preflight::every_dns_upstream_is_checked_without_hosts_or_fake_ip", "vpn::preflight::test_vpn_preflight::explicit_geoip_failure_is_not_disabled", "vpn::preflight::test_vpn_preflight::listening_proxy_is_not_a_working_chain", "vpn::preflight::test_vpn_preflight::missing_proxy_coverage_is_not_success", "vpn::preflight::test_vpn_preflight::one_good_dns_server_does_not_hide_failure", "vpn::preflight::test_vpn_preflight::read_only_rule_resources_are_enforced_and_unchanged", "vpn::preflight::test_vpn_preflight::real_https_is_not_full_permission_or_authorization", "vpn::preflight::test_vpn_preflight::reject_policy_never_contacts_probe", "vpn::preflight::test_vpn_preflight::strict_input_rejects_before_admission", "vpn::preflight::test_vpn_preflight::wrong_tls_root_fails_closed"], "results": {"vpn::preflight::test_vpn_preflight::cancellation_and_deadline_stop_probes": "PASS", "vpn::preflight::test_vpn_preflight::compiled_requirements_respect_mode_and_sources": "PASS", "vpn::preflight::test_vpn_preflight::errors_redirects_and_rewrites_cannot_fake_success": "PASS", "vpn::preflight::test_vpn_preflight::every_dns_upstream_is_checked_without_hosts_or_fake_ip": "PASS", "vpn::preflight::test_vpn_preflight::explicit_geoip_failure_is_not_disabled": "PASS", "vpn::preflight::test_vpn_preflight::listening_proxy_is_not_a_working_chain": "PASS", "vpn::preflight::test_vpn_preflight::missing_proxy_coverage_is_not_success": "PASS", "vpn::preflight::test_vpn_preflight::one_good_dns_server_does_not_hide_failure": "PASS", "vpn::preflight::test_vpn_preflight::read_only_rule_resources_are_enforced_and_unchanged": "PASS", "vpn::preflight::test_vpn_preflight::real_https_is_not_full_permission_or_authorization": "PASS", "vpn::preflight::test_vpn_preflight::reject_policy_never_contacts_probe": "PASS", "vpn::preflight::test_vpn_preflight::strict_input_rejects_before_admission": "PASS", "vpn::preflight::test_vpn_preflight::wrong_tls_root_fails_closed": "PASS"}, "fingerprint": "da2f5a38a7946f25e3ec56b8d672e1185f8984902d8fcb5d5512252cee0afa6e", "time": "2026-10-24T20:17:30.808385+08:00", "attempt": 1, "suite": "test_vpn_preflight", "file": "tests/test_vpn_preflight.rs", "environment": {"os": "macOS-27.2-arm64-arm-64bit", "architecture": "arm64", "rust": "rustc 1.98.1 (48a229cea 2026-09-01)", "cargo": "cargo 1.98.1 (797e8a9bc 2026-08-05)", "commit": "7008c5063e30286e844dfd29b40e9776b1bd129e", "dirty": true, "features": "Cargo 默认 features；未启用 --all-features", "optional_environments": {"NODUS_DEBUG_TEST_REQUIREMENT": false, "NODUS_DEBUG_TEST_SWIFT": false, "NODUS_PROTOCOL_PROFILE": false, "NODUS_PROTOCOL_FIXTURE": false}, "ci_run_id": null, "ci_run_attempt": null}} -->

## 基本信息

- 测试文件：`tests/test_vpn_preflight.rs`
- 测试对象：src/vpn/preflight.rs
- 执行机制：unit
- 测试范围：本文件登记且在当前平台实际发现的全部用例
- 实际命令：`/Users/klee/Documents/Nodus Core/target/debug/deps/nodus_core-e109a20f5477cf6e ::test_vpn_preflight:: --test-threads=1`
- 执行时间：2026-10-24T20:17:30.808385+08:00
- 实际耗时：1.2737477089999985 秒
- 运行环境：`{"os": "macOS-27.2-arm64-arm-64bit", "architecture": "arm64", "rust": "rustc 1.98.1 (48a229cea 2026-09-01)", "cargo": "cargo 1.98.1 (797e8a9bc 2026-08-05)", "commit": "7008c5063e30286e844dfd29b40e9776b1bd129e", "dirty": true, "features": "Cargo 默认 features；未启用 --all-features", "optional_environments": {"NODUS_DEBUG_TEST_REQUIREMENT": false, "NODUS_DEBUG_TEST_SWIFT": false, "NODUS_PROTOCOL_PROFILE": false, "NODUS_PROTOCOL_FIXTURE": false}, "ci_run_id": null, "ci_run_attempt": null}`
- 代码/测试/配置指纹：`da2f5a38a7946f25e3ec56b8d672e1185f8984902d8fcb5d5512252cee0afa6e`
- 总体状态：**PASS**
- 原始进程退出状态：0

## 实际用例与结果

| 实际发现的用例 | 独立目标 | 状态 |
| --- | --- | --- |
| `vpn::preflight::test_vpn_preflight::cancellation_and_deadline_stop_probes` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `vpn::preflight::test_vpn_preflight::compiled_requirements_respect_mode_and_sources` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `vpn::preflight::test_vpn_preflight::errors_redirects_and_rewrites_cannot_fake_success` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `vpn::preflight::test_vpn_preflight::every_dns_upstream_is_checked_without_hosts_or_fake_ip` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `vpn::preflight::test_vpn_preflight::explicit_geoip_failure_is_not_disabled` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `vpn::preflight::test_vpn_preflight::listening_proxy_is_not_a_working_chain` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `vpn::preflight::test_vpn_preflight::missing_proxy_coverage_is_not_success` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `vpn::preflight::test_vpn_preflight::one_good_dns_server_does_not_hide_failure` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `vpn::preflight::test_vpn_preflight::read_only_rule_resources_are_enforced_and_unchanged` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `vpn::preflight::test_vpn_preflight::real_https_is_not_full_permission_or_authorization` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `vpn::preflight::test_vpn_preflight::reject_policy_never_contacts_probe` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `vpn::preflight::test_vpn_preflight::strict_input_rejects_before_admission` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `vpn::preflight::test_vpn_preflight::wrong_tls_root_fails_closed` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |

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

- 原始证据：`/private/tmp/nodus-dns-commit-evidence/test_vpn_preflight.json`、对应 `.log`（实际存在时）。
- CI 工件标识：`本地证据目录；未宣称已上传 CI`
- 最新报告被后续变更影响时必须重新执行；检查器会拒绝指纹不一致的旧结果。
- 未运行的 Swift/外部协议或夹具独立目标均保持 NOT RUN；本报告不代表系统 VPN、外部服务端及跨平台已经验收。

<!-- nodus-test-documentation:begin -->
## 测试目的与验收目标

目的：阻止配置解析／候选编译、监听端口、DIRECT成功、单个DNS上游或复写生成响应冒充完整启动前置检查。
目标：严格校验；实际TLS／HTTP和所有配置DNS上游参与；有效规则、主配置及插件的必要代理均被覆盖；失败不联网绕行、不改文件、不进入Prepare／Running；取消和期限区分并确定清理。
方法：仅本机回环DNS/TLS、测试根证书和明确授权临时只读文件；使用共享事务门控，断言真实请求计数、HTTP／证书失败、DNS上游计数、有效快照及资源字节不变。13个函数的子场景保持在同名测试中，不用测试数量代替业务结论。性能测试不适用，未测吞吐、RSS及系统峰值。

## 原始失败及修复

- 首轮12项执行、10通过、2失败：DNS夹具漏填必填IPv6范围；显式GeoIP缺失授权文件没有归入GeoIP失败。原始attempt-1.log／json保留，退出101。
- 修正后13项执行通过，为最终代码之前的中间结果，不代替上方最终指纹。
- 静态检查期间曾发现示例误用不存在的Error变体和注释段落格式，编译／Clippy退出101；修正为标准示例错误传播与正确注释格式，不新增错误代码或关闭检查。

## 独立语义判断与未测边界

- 依据VPN规范确定：REJECT不发请求；缺必要代理覆盖拒绝；代理只监听不能通过；上游之一失败不由其他上游掩盖；合法数据库Ready、未配置WARN/Disabled、显式失败不Disabled；配置／资源不被改写；缺权限／系统／防回环证据不进入后续阶段。断言来自这些预期，非从实现输出生成。
- 本轮没有有效真实VLESS／REALITY远端互通成功证据，代理成功分支复用既有协议管线但不因此宣称远端验收。DNS地址族、DNSSEC和所有错误组合并非全部覆盖；相关既有回归另行列明。
- 完整平台权限、系统能力、防回环、OS FD余量、完整峰值、可信无节点授权、系统接管及最低系统均未验证。完整前置检查仍未完成，不能将network_checks_passed作为接管资格。
- 本地原始证据位于自动报告指定目录，未上传审查材料；测试文档不冒充系统验收。
<!-- nodus-test-documentation:end -->
