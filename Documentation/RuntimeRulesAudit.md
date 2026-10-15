# 运行时规则本地审查记录

日期：2026-10-12。作者：可莉唯一的狗。

以下为 2026-10-12 历史审查证据，保留原始测试与性能数字。现行 08 / 10 补充验收及正式组合规则见 [规则组合与资源验收](规则组合与资源验收.md)；本次授权提交 dev，不合并 main、不发布 crate/二进制、不改 App。

## 16 项审查结果

| 编号 | 处理情况 | 证据与边界 |
| --- | --- | --- |
| 01 | 已修复 | 迁移入口显式字节门禁、解析预算与 RAII；超大迁移回归通过。底层原有 YAML 字节门禁并非完全缺失。 |
| 02 | 已修复 | from_document 在额外集合/Entry/索引前扫描估算和预留；去掉只为预算创建的来源规则副本。 |
| 03 | 已修复信任表达 | 加载文档标记 StructuralOnly；原来源可用时 verify_against_sources 重算完整文档；Swift summary 同步 report_verification。缺少原文不能证明删除理由。 |
| 04 | 已核实并加固 | 原有 YAML 序列访问器在解析第 100001 条前返回 ResourceLimit；新增订阅解析预算，覆盖大 payload、长字段及数量边界。 |
| 05 | 已补系统回归 | 40 个确定性随机配置批次、6000 个目标比较未裁剪 Reference 与优化 Snapshot 的策略/出口/原来源，并执行加载往返；不等于穷举或持续 fuzz。 |
| 06 | 已修复并验证 | 按用户新授权：普通域名/IP 先于端口/全域传输；关键词也是域名规则。RuleClass 编号只是槽位，不作排序，不靠数字加减映射来源。 |
| 07 | 已补测试 | 域名命中、未命中、IPv4/IPv6、拦截及端口规则的 DNS 前后矩阵；未知 IP 时不能由端口直连提前放行。 |
| 08 | 已测量并校准，仍有边界 | 多标签后缀实测暴露旧估算偏低，标签系数提高至 512 字节；长关键词和高标签数量超限在建索引前拒绝。没有证明所有输入/并发均低于512 MiB。 |
| 09 | 不需要更换注册锁 | lookup 位于 Router 编译/绑定阶段；匹配使用缓存 Arc，不逐查询访问注册 Mutex。没有为未经测实的锁竞争另加缓存层。 |
| 10 | 已测量 | 四档混合规则编译/查询/更新/恢复峰值及两种形态压力；是本机单轮观测，不是全平台性能目标证明。 |
| 11 | 已加固并测试 | 来源/序号重复、非法引用、乱序、统计伪造、重复来源指向及原序号空洞受控拒绝；不接受部分快照。 |
| 12 | 已修复 | 完整值篡改测试改为修改类型化 Document 后序列化；重复键、大小写和未知字段仍保留原始文本解析测试。 |
| 13 | 已修复 | 预算缩放统一 checked_mul/checked_add；极端整数溢出测试通过。 |
| 14 | 已补诊断 | from_yaml_diagnosed 提供阶段及来源/规则整数序号，保持普通入口与 C ABI 稳定错误类别，不输出域名或凭据。 |
| 15 | 已集中当前审查常量 | 全局/输入/解析/标签/关键词/句柄估算集中命名；既有 YAML 与新二进制缓存各自边界仍由各格式模块定义。 |
| 16 | 已补契约 | RuntimeRules.md 区分原始来源、生成文档、不可变索引、报告信任程度和独立参考实现；可运行例子见 examples/runtime_rules_audit.rs。 |

## T01–T10 验证

| 编号 | 状态 | 范围 |
| --- | --- | --- |
| T01 | 通过已构造边界 | 数量、字节、标签预算、生命周期以及失败后的再次生成；不等于所有分配的硬上限证明。 |
| T02 | Rust 回归通过 | 旧 Arc 多线程读取、新快照后台构建与原子切换，既有真实TCP夹具仍回归；未运行 Miri/线程消毒器。 |
| T03 | 通过 | 6000 个随机目标、策略/出口/原来源一致，另有直接产品语义断言，避免只有同质参考自洽。 |
| T04 | 通过 | 域名边界、父子后缀、CIDR、重复、冲突、部分/完整端口覆盖与拦截保留。 |
| T05 | 通过 | DNS 前后矩阵；调试 IP 未确定时也不得让正式拦截提前结束决策。 |
| T06 | 通过 | 类型化字段篡改、非法来源和引用、序号空洞、未知版本/字段/重复键及二进制缓存损坏/截断/越界。 |
| T07 | 通过已构造输入 | 大量重复、100001条、超长 scalar、超大 payload；原始输入由调用者分配不受库估算器硬控制。 |
| T08 | 回归通过 | 256轮弱句柄注册/释放、128轮调试替换/清除、原有快照并发与所有者回收；不是堆泄漏工具的证明。 |
| T09 | 已记录 | 1000/10000/50000/100000条混合规则单轮，以及高标签/长关键词压力。 |
| T10 | 通过 | YAML生成/重新加载/查询，二进制恢复和Swift FFI；报告恢复后明确为仅结构验证。 |

## 临时调试 API

- Debug Overlay 独立于正式 RuleClass；只由 Session API 构造，不提供 YAML、订阅或持久化入口。
- 调试规则可以覆盖正式拦截。每条 lifetime_seconds 必填且为1–3600秒；空、0和超过一小时返回错误。
- 内部保存 UTC Unix 过期时间和单调截止点，任何一项到期都不再匹配；决策/API访问丢弃过期条目。旧视图不得延长有效期。
- 配置/订阅/生成文档的调试字段及 debug 来源层级明确拒绝。256条及64会话上限有界，失败保留旧方案。
- 域名范围内TCP/UDP复合条件可通过调试 API 使用；普通配置还不支持复合条件。全域协议 direct/proxy 非法。QUIC 未实现，不能将UDP当作QUIC。
- 当前是本地 Rust API；调试 C ABI/Swift 接口、App UI、已认证控制服务及真实调试网络互通尚未交付。宿主认证与授权要求不等于已经提供认证实现。

## 验证结果

- Rust 全量：176 项通过，另1项既有外部节点测试忽略，不计入通过。
- 严格 Clippy 全目标通过。
- 修改的 Swift 文件通过 swift-format 严格检查；SwiftLint/SwiftFormat 工具本轮环境未安装，不宣称这两项通过。
- 路由与运行时规则两组真实 Swift FFI通过，包含新生成报告已验证/恢复报告仅结构验证状态。
- App 的既有公开示例严格解析测试通过；使用固定远程内核包，无本地 Rust 目录依赖。先前 sandbox模块缓存失败及iCloud构建产物签名失败保留为环境失败记录，切换临时构建目录后通过。
- 核心与 App 完整示例同步中文注释；不新增调试配置字段。
- 本地可运行例子实际执行通过，不建立真实网络连接。
- 最低macOS14、Intel、移动系统、真机VPN、QUIC、App调试接入和审核未测。没有执行发布或修改App依赖。

## 替换 SQLite 前的历史单轮基准

环境：Apple M4，macOS27.2，Rust1.98.1，release优化/thin LTO/单代码生成单元。各规模独立进程、顺序运行；查询与完整更新是两个进程。查询每类20000次、四线程累计400000次，目标为固定热查询。峰值使用time -l全进程RSS，包含恢复和分配器留存。

| 规则数 | 编译 ms | 编译后 RSS MiB | 首/尾/未命中 p99 ns | 两次完整更新 ms | 更新/恢复峰值 MiB |
| --- | --- | --- | --- | --- | --- |
| 1000 | 3 | 13.20 | 292/292/250 | 32 | 15.50 |
| 10000 | 7 | 22.30 | 167/167/167 | 56 | 33.56 |
| 50000 | 36 | 52.80 | 167/167/167 | 200 | 85.83 |
| 100000 | 78 | 88.84 | 167/167/167 | 386 | 157.58 |

上表属于替换 SQLite 之前的单轮记录，当前方案见 BinaryRuleCachePerformance.md；不能与本轮三轮中位数混作性能提升，也不能代表DNS、TLS、网络吞吐或其他内核对比。纳秒结果接近计时粒度，需更长重复采样再解释。此次提高资源估算系数并非提高匹配速度。

## 复现

```sh
cargo test --all-targets -- --test-threads=1
cargo clippy --all-targets -- -D warnings
cargo run --example runtime_rules_audit
cargo build --release --example runtime_rules_bench --example runtime_rules_memory
./target/release/examples/runtime_rules_bench 100000
/usr/bin/time -l ./target/release/examples/runtime_rules_bench 100000 --persist
/usr/bin/time -l ./target/release/examples/runtime_rules_memory 10000 suffix
/usr/bin/time -l ./target/release/examples/runtime_rules_memory 50000 keyword
```

原始日志保存在本轮宿主 /private/tmp/fluxa-audit-last-* 与 /private/tmp/fluxa-audit-calibrated-memory-*，不上传构建二进制或流量导出。

## 二进制缓存替换

SQLite 已移除，当前缓存为 runtime-rules.bin；索引直接借用不可变字节缓冲，不重建执行树。格式与零拷贝范围见 [BinaryRuleCache.md](BinaryRuleCache.md)，三轮同条件对比见 [BinaryRuleCachePerformance.md](BinaryRuleCachePerformance.md)。规则元数据仍解码；本轮不是 mmap 或整份快照免反序列化。缓存恢复拒绝旧 SQLite，需由宿主从原来源重建，不能回退旧 YAML。
