# v2.4.0 统一错误协议修复包发布记录

本次按用户最新要求仅提交与发布内核修复包，不更新 Fluxa App 的依赖、源码或版本。公开协议为分段四位整数，原接口直接迁移，C ABI 版本为 2。

## 发布身份与下载校验

- 发布源码：`66fae1b72415d854d54e41e5c553be87ec2df457`。
- 版本标签：`v2.4.0`；标签提交：`8f161c96b74332ee8e76d29e492bf11572e24463`。
- 标签提交相对构建源码仅更新 Package.swift 的二进制校验值；Rust 源码没有变化。
- GitHub 发布：[v2.4.0](https://github.com/teaoea/Fluxa_Core/releases/tag/v2.4.0)。发布流程 38513693030 的全部步骤实际成功，包括 Rust/真实 Swift FFI、静态框架、附件上传及远程包下载验证。
- crates.io：[libfluxa_core 2.4.0](https://crates.io/crates/libfluxa_core/2.4.0)。从干净发布源码打包并实际上传，远程下载的 crate 与上传归档逐字节一致，归档 VCS 来源为上述发布源码。

| 交付物 | 独立下载计算的 SHA-256 |
| --- | --- |
| libfluxa_core-2.4.0.crate | `90c9501bafe6e28caf1ea1e4e273ae73340f39881ad5a70d7a0468ee21bfa0d7` |
| FluxaEngine.xcframework.zip | `8909f5629c8d19e8ade3aaa39dc482e47f211ea2ec55923f19aad51c82e2d690` |
| fluxa-test-fixture-macos-arm64.zip | `05ecf9b7eb62249a79e6db5778008a2d5076c317ef848af6e51963af1619bcc6` |

独立下载的 provenance.json、checksum.txt、fixture-checksum.txt 与附件哈希一致；远程 v2.4.0 Package.swift 固定此版本 URL 与对应框架校验值。验证附件保存于 /private/tmp/fluxa-240-release-verified，临时文件不是长期交付物。

## 错误协议与兼容性

权威定义为 src/error.rs，28 个现行代码均有中文注释。前两位表示登记类别，后两位表示具体失败；编号显式固定，不依赖枚举顺序。旧 1002–1020 永久退役，避免已发布编号改义；能力不支持使用 1021。新协议解析拒绝退役号。

ErrorCode 是 repr(u16) 强类型整数；ErrorInfo 返回整数 code 与受约束 params。字段名使用固定缓冲区；未知或重复字段的安全标识不能公开时仍保持 2002/2003，并使用 field:null。缺失 TTL 返回 4001、actual:null、limit:3600。预算申请失败返回 8002 及实际申请量与预算上限，实际申请量不必大于上限。

旧标量、命令与诊断入口统一新方案，成功 0 与队列空 11 是非错误状态。结构化错误编码失败返回 1202，不能伪装无错误。规则 REJECT、远端拒绝连接与协议报文不合法分别表达；错误表示不擅改策略、预算或配置事务。

这是已授权的协议及 Rust 源码兼容性变化：按旧整数判断、穷尽匹配 Error、构造新增 error_info 可选字段的调用方需要迁移；旧 ABI 客户端不能搭配新框架。迁移示例见 examples/error_protocol.rs、include/fluxa_engine.h、Tests/ffi_error_code.swift 和错误查询表。回滚须同时还原客户端与框架。App 未更新，其现有固定依赖继续保持原版本。

## 实际验证与 L1–L4

| 层级 | 实际执行或独立核验 | 结果与边界 |
| --- | --- | --- |
| L1 | cargo fmt --check；cargo check --offline --all-targets；cargo clippy --offline --all-targets -- -D warnings | 通过；只证明静态正确性 |
| L2 | cargo test --offline --no-fail-fast | 217 通过、0 失败、1 忽略；独立服务端互通另行执行 |
| L2 | 错误契约测试；干净 crate 包内 cargo test --test error_contract | 17 项通过；含四位分段、退休编号、完整 Schema、上下文、非法序列化与敏感数据限制 |
| L2 | cargo run --example error_contract -- --check；实际删掉查询表 1001 再检查 | 正常 28 项登记通过；负向样本检查实际失败，恢复后通过 |
| L2 | cargo run --example error_protocol；真实 Swift FFI 套件 | 示例实际运行；17 组 FFI 通过，最终发布源码在发布流程重新验证 |
| L2 | 独立对端协议脚本 | 37 轮 Rust 与 37 轮真实 Swift 互通通过 |
| L3 | 基于用户规范独立检查分类、TTL、预算、拒绝、原子失败保留及具体 FFI 路径 | 本轮已验证项目详见 ErrorReworkReview.md；不得宣布整个 23 章全部通过 |
| L4 | 实际源码、冻结契约、测试日志、归档 VCS、发布流程、远程下载与哈希 | 来源及交付物已核验，未测项目明确登记 |

文档一致性：现行公开代码 28，登记 28；重复、遗漏、未定义、缺少说明、触发条件、处理后果、源码和参数 Schema 均为 0。8 个参数条目由权威定义生成。自动一致性不替代业务语义正确性。

## 未解决及未验证事项

- H1 Fake-IP 历史 TUN 池生命周期问题未在本包处理，仍按用户指定顺序排队。
- Debug 在完整出站路径的既有先行正式传输校验问题、宿主认证与授权未完成；不能宣布全部安全边界验收通过。
- 核心 Job 查询提供 error_info，但 Swift SDK NativeRuntimeRulesClient.waitForJob 的 failed 分支仍转为 rejected 文本，便利接口不能声称完整保留机器上下文；原查询接口仍可读取结构化错误。该限制真实存在，没有用测试通过掩盖。
- 非空 FFI 指针可读性仍是调用者契约；没有提供恶意同进程内存隔离。
- 最低部署系统、所有平台、真实用户节点、系统 VPN、断电恢复、全部分配故障及错误风暴性能尚未验证。
- 泛化 INPUT/LIFECYCLE/IO 分类仍保留明确归类范围，并非每个输入字段或内部竞态都有独立代码。

## 实际变更文件

以下是本次错误修复及发布标签相对此前基线的文件变更，包含用户要求的规范全文替换；不包含 App，也不包含本地另有的 LICESE.md 修改。

- `.github/workflows/release.yml`
- `AGENTS.md`
- `Cargo.lock`
- `Cargo.toml`
- `Documentation/ErrorContractIntroduction.md`
- `Documentation/ErrorReworkReview.md`
- `Documentation/ErrorSystemReview.md`
- `Documentation/Overview.md`
- `Documentation/Overview_ENGLISH.md`
- `Documentation/Publishing.md`
- `"Documentation/Rust\347\275\221\347\273\234\344\273\243\347\220\206\345\206\205\346\240\270\347\273\237\344\270\200\351\224\231\350\257\257\344\273\243\347\240\201\344\275\223\347\263\273\350\256\276\350\256\241\343\200\201\345\256\236\347\216\260\344\270\216\345\256\241\346\237\245.md"`
- `Documentation/error-codes.json`
- `"Documentation/\351\224\231\350\257\257\344\273\243\347\240\201\344\275\223\347\263\273\350\256\276\350\256\241.md"`
- `"Documentation/\351\224\231\350\257\257\344\273\243\347\240\201\346\237\245\350\257\242\350\241\250.md"`
- `Package.swift`
- `README.md`
- `README_ENGLISH.md`
- `Release.version`
- `Sources/FluxaEngine/NativeCommandClient.swift`
- `Sources/FluxaEngine/NativeMitmClient.swift`
- `Sources/FluxaEngine/NativeRuntimeRulesClient.swift`
- `Tests/error_contract.rs`
- `Tests/ffi_dns.swift`
- `Tests/ffi_dns_service.swift`
- `Tests/ffi_error_code.swift`
- `Tests/ffi_lifecycle.swift`
- `Tests/ffi_outbound.swift`
- `Tests/ffi_protocol_block.swift`
- `Tests/ffi_routing.swift`
- `Tests/ffi_runtime_rules.swift`
- `Tests/ffi_script.swift`
- `Tests/ffi_tun.swift`
- `Tests/ffi_tun_dns.swift`
- `Tests/ffi_tun_udp.swift`
- `Tests/fixtures/error-contract-v2.json`
- `Tests/routing.rs`
- `Tests/runtime_rules.rs`
- `examples/error_contract.rs`
- `examples/error_protocol.rs`
- `include/fluxa_engine.h`
- `src/budget.rs`
- `src/dns/mod.rs`
- `src/error.rs`
- `src/ffi.rs`
- `src/routing/background.rs`
- `src/routing/debug.rs`
- `src/routing/runtime.rs`
- `src/routing/yaml.rs`
- `src/script.rs`
