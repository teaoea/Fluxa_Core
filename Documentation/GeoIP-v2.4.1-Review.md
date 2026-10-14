# GeoIP v2.4.1 独立审查与验证记录

本修复按用户最终约定实施：未配置 = 用户不需要 = WARN + Disabled；明确配置正常运行 = INFO + Ready；明确配置执行失败 = ERROR + Err，禁止静默降级。所有处理结果使用中央固定整数，错误使用用户指定 2401–2409，状态使用同类别 2410–2412。App 不修改、不更新依赖；H1 不在本修复范围。

## L1 静态正确性

cargo fmt --check、cargo check --offline --all-targets、cargo clippy --offline --all-targets -- -D warnings 实际通过。Swift 随包门面在真实 FFI 中编译和运行。没有新第三方依赖，没有业务 unsafe。测试模块重复加载已修正为共享同一个夹具模块，未禁用 Clippy。

## L2 实现一致性

- Rust 全量回归 223 通过、0 失败、1 忽略；独立服务端互通按发布流程另外验证。
- 错误契约 17 项通过；公开代码 40 项，包含 37 个执行错误和 3 个非错误状态。旧冻结 v1/v2 未修改，新契约另有 v2.4.1 冻结基线。查询表、Schema 和参数一致，重复、遗漏、未定义及必要元数据缺失均为 0。
- GeoIP 5 个集成测试通过，包含真实 TLS 下载、超过 8 MiB 的 MMDB、非法 URL/协议、截断、失败 HTTP 状态、超限、搜索树损坏、合法无匹配及实际查询故障。
- 实际 TLS 延迟响应触发 60 秒下载期限，返回 2404，5 项测试最终在 60.21 秒完成。不是仅按枚举推断超时处理。
- 实际 Swift 全套 FFI 的其它 16 组已通过；GeoIP 最终专项重新通过。过程中发现无字段 Serde 枚举会忽略旧 path 参数，已改成严格空字段结构，原拒绝断言保留并实跑通过。
- 可运行 geoip_configuration 示例打印 Disabled/2410，不联网下载占位 URL。两份完整示例通过有界 YAML 解析并单独验证 GeoIP 字段，未冒充已验证所有主配置业务。

日志为 /private/tmp/fluxa-geoip-final-tests.log、fluxa-geoip-deadline.log、fluxa-geoip-final-clippy.log、fluxa-geoip-final-contract.log、fluxa-geoip-all-ffi.log 和 fluxa-geoip-final-ffi.log。临时日志不是发布身份；归档与远程验证另记最终发布记录。

## L3 独立业务语义

| 用户规范 | 真实入口 | 独立预期与实际证据 |
| --- | --- | --- |
| 只取配置 URL，无默认 | geoip::Config、Service；ffi GeoipConfigure/NetworkConfigure | 缺失/null 返回 Disabled；空串 2401；非 HTTPS 2402；无默认厂商、宿主文件或远程镜像回退 |
| 未配置不初始化、不下载 | ffi GeoipLookup/GeoipRefresh；Service::lookup/refresh | 没有 DNS/证书配置时调用仍返回 Disabled，测试用不可解析目标验证先行状态分支 |
| 无效配置整体拒绝 | ffi 配置先校验再替换 Arc；网络校验成功后提交来源 | Swift 空/HTTP URL 返回 2401/2402，随后从原有效 URL 下载成功，证明非法更新未覆盖旧来源 |
| 配置后失败是 Err | geoip::access_error、database、Database::lookup | 2403–2409 分离；DNS/TLS 等底层分类在有界诊断环保留；失败不会生成 Ready 或 Disabled |
| 正常加载 Info Ready | Service::refresh 的校验后提交 | 实际 HTTPS MMDB 成功，IPv4/IPv6 返回记录和 Ready/2411 |
| 合法无记录非错误 | Database::lookup、Status::no_match | 合法 192.0.2.1 返回 success:true、no-match/2412，没有 error_info |
| 实际查询故障不能冒充无匹配 | Database::lookup | 有效 MMDB 编码但非法国家字段返回 2409；非法输入仍使用原输入校验，不伪造有效地址 |
| 完整性与资源限制 | 专用 HTTP 分帧、from_bytes、verify_records | 截断 2406；循环搜索树拒绝；超过 256 MiB 的声明长度在分配前拒绝；普通 HTTP 8 MiB 上限保持 |
| 来源替换隔离 | 独立不可变 Service Arc | 变更/移除来源换服务，旧下载只持旧服务；新来源未加载不能使用旧来源库；同来源更新失败仍明确 Err，旧有效库保留符合原子更新契约 |

错误代码不会改变正式规则优先级、DNS 规则语义、Debug 授权、REJECT 或出站策略。代码表达失败，不能代替来源认证或授权。

## L4 证据与兼容性

代码、Rust/Swift 真实测试、中央契约和本文共同提供证据。错误参数为空、有界，不公开 URL、IP、密码、文件路径；日志为固定 64 条诊断环，等级由中央分类决定，不分配无界日志对象。

C ABI 仍为 2，底层函数签名不变；JSON 新增可选 warn/geoip_status/level，新增明确整数代码。geoip_open 不再接受 path；Swift open(database:) 迁移为 open()/refresh()，调用方先配置 geoip_url。Rust Error 新增九个变体，穷尽匹配调用方需补充分支。因用户要求本次按修复包处理，版本为 v2.4.1。配置字段及示例同步；App 保持旧固定依赖，不在本次更新。

## 未验证边界

独立协议回归最初因 GitHub 对端下载 TLS 握手超时未完成，不能用此前版本互通代替本轮结果。后续通过项以实际发布流程和最终发布记录为准。最低部署系统、全部平台、真实用户数据库/来源、真实 VPN、全部内存故障及高并发压力未验证。MMDB 校验覆盖格式、布局、分隔区和全部可达记录；未配置外部预期哈希或签名，不能宣称验证来源签名、不可达附加数据或地理真实性。既有 H1 与 Debug 完整出站优先级/宿主鉴权问题没有在本轮修复。

## 本轮文件

- `.github/workflows/release.yml`
- `Cargo.lock`
- `Cargo.toml`
- `Documentation/ErrorContractIntroduction.md`
- `Documentation/GeoIP.md`
- `Documentation/error-codes.json`
- `"Documentation/\351\224\231\350\257\257\344\273\243\347\240\201\346\237\245\350\257\242\350\241\250.md"`
- `Package.swift`
- `README.md`
- `README_ENGLISH.md`
- `Release.version`
- `Sources/FluxaEngine/NativeCommandClient.swift`
- `Sources/FluxaEngine/NativeGeoIPClient.swift`
- `Tests/error_contract.rs`
- `Tests/ffi_geoip.swift`
- `Tests/geoip.rs`
- `Tests/support/mod.rs`
- `check-ffi.sh`
- `fluxa.yaml`
- `fluxa_english.yaml`
- `src/error.rs`
- `src/error/diagnostics.rs`
- `src/ffi.rs`
- `src/geoip.rs`
- `src/http.rs`
- `src/https.rs`
- `Tests/fixtures/error-contract-v2.4.1.json`
- `examples/geoip_configuration.rs`
- `Documentation/GeoIP-v2.4.1-Review.md`
