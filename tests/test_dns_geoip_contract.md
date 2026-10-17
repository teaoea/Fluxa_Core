# 测试报告：test_dns_geoip_contract

<!-- fluxa-test-report:{"command": ["/Users/klee/Documents/FLuxa_Core/target/debug/deps/test_dns_geoip_contract-6856001dc6ef078e", "--test-threads=1"], "exit_code": 0, "duration_seconds": 2.430074791999999, "status": "PASS", "discovered": ["admitted_family_fallback_survives_configuration_replacement", "explicit_doh_ignores_formal_route_and_retires_old_resolver", "geoip_ready_failure_atomicity_and_disabled_rules", "named_numeric_dns_failure_never_uses_direct", "ordered_upstreams_get_independent_deadlines", "preferred_family_order_ip_reject_and_nxdomain"], "results": {"admitted_family_fallback_survives_configuration_replacement": "PASS", "explicit_doh_ignores_formal_route_and_retires_old_resolver": "PASS", "geoip_ready_failure_atomicity_and_disabled_rules": "PASS", "named_numeric_dns_failure_never_uses_direct": "PASS", "ordered_upstreams_get_independent_deadlines": "PASS", "preferred_family_order_ip_reject_and_nxdomain": "PASS"}, "fingerprint": "c264632751cbd621ac8e99a7fc936c7dc8e38a212ff22a17f1e08e826b8abf4a", "time": "2026-10-17T20:52:25.224146+08:00", "attempt": 1, "suite": "test_dns_geoip_contract", "file": "tests/test_dns_geoip_contract.rs", "environment": {"os": "macOS-27.2-arm64-arm-64bit", "architecture": "arm64", "rust": "rustc 1.98.1 (48a229cea 2026-09-01)", "cargo": "cargo 1.98.1 (797e8a9bc 2026-08-05)", "commit": "16f52121212da73a2ea69047e4c6231b67ab5241", "dirty": true, "features": "Cargo 默认 features；未启用 --all-features", "optional_environments": {"FLUXA_DEBUG_TEST_REQUIREMENT": false, "FLUXA_DEBUG_TEST_SWIFT": false, "FLUXA_PROTOCOL_PROFILE": false, "FLUXA_PROTOCOL_FIXTURE": false}, "ci_run_id": null, "ci_run_attempt": null}} -->

## 基本信息

- 测试文件：`tests/test_dns_geoip_contract.rs`
- 测试对象：src/dns/main.rs、src/providers.rs、src/outbound/configuration.rs、src/outbound/execution.rs
- 执行机制：integration
- 测试范围：本文件登记且在当前平台实际发现的全部用例
- 实际命令：`/Users/klee/Documents/FLuxa_Core/target/debug/deps/test_dns_geoip_contract-6856001dc6ef078e --test-threads=1`
- 执行时间：2026-10-17T20:52:25.224146+08:00
- 实际耗时：2.430074791999999 秒
- 运行环境：`{"os": "macOS-27.2-arm64-arm-64bit", "architecture": "arm64", "rust": "rustc 1.98.1 (48a229cea 2026-09-01)", "cargo": "cargo 1.98.1 (797e8a9bc 2026-08-05)", "commit": "16f52121212da73a2ea69047e4c6231b67ab5241", "dirty": true, "features": "Cargo 默认 features；未启用 --all-features", "optional_environments": {"FLUXA_DEBUG_TEST_REQUIREMENT": false, "FLUXA_DEBUG_TEST_SWIFT": false, "FLUXA_PROTOCOL_PROFILE": false, "FLUXA_PROTOCOL_FIXTURE": false}, "ci_run_id": null, "ci_run_attempt": null}`
- 代码/测试/配置指纹：`c264632751cbd621ac8e99a7fc936c7dc8e38a212ff22a17f1e08e826b8abf4a`
- 总体状态：**PASS**
- 原始进程退出状态：0

## 实际用例与结果

| 实际发现的用例 | 独立目标 | 状态 |
| --- | --- | --- |
| `admitted_family_fallback_survives_configuration_replacement` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `explicit_doh_ignores_formal_route_and_retires_old_resolver` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `geoip_ready_failure_atomicity_and_disabled_rules` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `named_numeric_dns_failure_never_uses_direct` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `ordered_upstreams_get_independent_deadlines` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |
| `preferred_family_order_ip_reject_and_nxdomain` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |

## 执行汇总

- 实际发现：6
- 实际执行完成：6
- 通过：6
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

- 原始证据：`/private/tmp/fluxa-dns-geoip-frozen-20261017/test_dns_geoip_contract.json`、对应 `.log`（实际存在时）。
- CI 工件标识：`本地证据目录；未宣称已上传 CI`
- 最新报告被后续变更影响时必须重新执行；检查器会拒绝指纹不一致的旧结果。
- 未运行的 Swift/外部协议或夹具独立目标均保持 NOT RUN；本报告不代表系统 VPN、外部服务端及跨平台已经验收。

## 本轮独立业务语义核验补充（2026-10-17）

依据为用户已确认的业务约定，不以实现输出或差分一致推导预期。核验范围仅限本轮 DNS/GeoIP 编排：

- DNS 专用 direct/节点路由与业务规则隔离：检查 Plan::build 固定路由和 ManagedRuntime 路径；direct DoH 遇正式域名 REJECT 仍取得回环答案，指定失败节点的数字 DNS 未收到直发包。没有据此宣称外部 VLESS 成功互通。
- Ready 门禁与事务：检查 prepare_ready 的同来源数据库验证、Runtime::update 发布前下载/编译、commit_provider_configuration 的版本和权限检查及缓存提交顺序。损坏来源返回 2405，旧版本和拦截决策保留；真实刷新成功后才清除历史失败状态。Disabled/Ready/no-match 仍为 2410/2411/2412，不混为 Err。
- 地址族：明确输入中的 IPv6 无记录、同族单个 IP REJECT、NXDOMAIN、全候选 REJECT，分别验证换族、排除单 IP、禁止换族；替换配置时已准入事务继续，新请求只用新配置。
- 生命周期：检查普通退休幂等、缓存提交失败不撤销、关闭先停节点连接再等待操作、Drop 等待不可取消操作归还名额；原 test_outbound_dns 和 test_tun_dns 的失败断言完整保留，修复后均 8/8 通过。
- 安全/资源：未修改 Debug 专用 IPC、TTL、权限或正式规则优先级；新资源仍经 Authority/操作令牌和既有预算。错误不转换 DIRECT。MMDB 与 Reader 共用正文，旧内存直至真实销毁才释放预算；哈希不是来源签名。

L1：格式、全目标 check、严格 Clippy、43 项错误契约、完整配置解析、离线示例均退出 0，见 frozen 目录 static.json 和 static-*.log。L2：72 个 Rust 套件实际执行，459 PASS、1 IGNORED/SKIPPED、0 FAIL；Python 报告工具 9 项通过。L3：上述调用链和独立预期在本轮范围内已核实；不是全模块或全平台无缺陷声明。L4：最终 run.json 退出 0，report_errors 为空，80 个映射/报告指纹检查通过。

最终原始证据：/private/tmp/fluxa-dns-geoip-frozen-20261017；Python：/private/tmp/fluxa-dns-geoip-frozen-python。失败与 STALE 历史保留在 fourth、fifth、final-20261017、accepted-20261017 等先前目录，不计为当前通过。受测基线 16f52121212da73a2ea69047e4c6231b67ab5241 + 本轮 dirty 工作树；源码指纹 c264632751cbd621ac8e99a7fc936c7dc8e38a212ff22a17f1e08e826b8abf4a。

尚未验证：外部 VLESS 节点 DNS 成功互通、移动设备路径/权限及 tvOS 缓存回收、真实夏令时、断电恢复、系统 VPN、RSS 硬上限、远程 CI。原有 Publishing/Versioning 两份本地链接修改未纳入本轮提交；本地报告明确记录 dirty 状态，不冒称远端干净提交的运行结果。
