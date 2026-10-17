# 规则提供者生命周期

作者：可莉唯一的狗。当前契约：2026-10-17 dev；源码入口 `src/providers.rs`、`src/providers/document.rs`、`src/providers/cache.rs`、`src/platform/providers.rs`。本文件描述主配置 `rule-providers`，不将独立旧 `subscription::Check` 的 UTC 纯计划接口当作新配置的调度实现。

## 配置与正文

完整配置见 [example_config.yaml](../example_config.yaml)，远程正文见 [rules.yaml](../rules.yaml)。提供者名称唯一，使用安全文件名；策略为 `direct`、`reject` 或已声明的大小写敏感节点名，不能用 `policy: proxy` 加 `outbound`。插件和代理提供者执行仍暂缓。

```yaml
rule-providers:
  - name: ads
    url: https://example.com/ads.yaml
    policy: reject
    enable: true
    update-time: "12:00"
rules: ['final,direct']
```

远程正文为 `rules` 列表和小写类型，只声明条件。`domain`、`domain-suffix`、`domain-keyword`、`ip-cidr`、`protocol`、`port` 等按主规则条件校验；不接受远程 final、来源身份或 Debug 声明。`protocol,quic,example.com` 和 `port,443,example.com` 的域名采用 suffix 语义。非拦截的全域 protocol 不合法。正文自行附加策略的行单独忽略；为避免三列歧义，组合条件的域名如果等于 direct、reject 或已声明节点名，也单独忽略。

所有远程 process-name 条目均忽略，不区分提供者策略。提供者 policy=reject 时另忽略 domain-suffix、domain-keyword 条目；只允许既定精确 domain、ip-cidr、protocol、port 条件执行拦截。非字符串、非法及不支持的行单独忽略，不影响其他合法行。YAML 语法、根结构、重复/未知字段及资源上限错误拒绝整次更新。正文 `payload` 不兼容：旧 `subscription::parse_rules` 的 Clash classical API 独立保留，不能将其格式传给新提供者入口。

## 下载、缓存及事务

内核只访问显式 HTTPS URL，沿用系统/显式企业 TLS 根，不关闭证书验证。下载使用候选本地路由与节点策略；没有配置业务 DNS 的当前执行范围允许系统解析下载源域名，不是已启用 DNS 的失败回退。活动业务 DNS、域名节点与 GeoIP 已接入异步主配置编排，见 [DNS/GeoIP 编排](DNSGeoIP编排.md)。

正文单项最多 8 MiB、URL 最多 4096 字节、最多五次 HTTPS 重定向，拒绝 HTTP 降级、重定向环和非 200 最终响应；整次下载上限 60 秒。正文常驻共享预算 128 MiB；解析租约保留至编译完成；编译仍受已有规则、来源和索引预算约束，不能把数量上限解释为所有内容形态都可接受。

macOS 缓存保存为 `~/.fluxa/rules/<name>.yaml`；iOS/iPadOS 为应用沙箱 `Library/Application Support/Fluxa/rules/<name>.yaml`；tvOS 为应用沙箱 `Library/Caches/Fluxa/rules/<name>.yaml`，不是 App 包内资源。`.state.json` 登记来源 URL 哈希、正文 SHA-256、成功时间及本地日期槽；`.fluxa/.provider-attempts.json` 独立保存有限尝试记录。内容哈希用于一致性校验，不代替 HTTPS 来源认证。目录不允许符号链接或组/其他用户写入；本设计不声称防御已控制相同用户账户的攻击者。

首次启用且没有同 URL 的有效缓存时下载；未配置 update-time 的提供者之后只手动更新，重启复用有效缓存。停用提供者不自动下载，手动更新不会自动启用。只改本地规则或提供者策略会重新编译已验证正文，不重新下载、不重写远程文件，忽略条数按当前策略重新统计。

所有待更新正文先下载、解析、编译并写入暂存目录，通过后使用 macOS/Linux 原子目录交换，并在同一内核发布边界替换网络快照。任何提供者中途失败不部分替换旧规则或旧正文，旧连接不因此断开；尝试日志仍保留失败时间。新请求读取新版本，连接完成建立后无需持有整份旧规则快照。目录交换后 fsync 失败属于已提交但持久化确认失败：版本已经发布，`directory_synced=false` 且 `error_info` 返回受控错误，不能伪称已回滚。磁盘/内存不具备跨进程断电的共同原子事务；恢复以通过完整校验的磁盘批次为准。

重用缓存前核验真实文件及清单；文件篡改/损坏拒绝普通配置复用。仍存活内核可通过明确手动刷新重新下载修复；启动时已损坏缓存不能被默默信任，当前须停机清理该专用缓存后重新导入，不提供自动损坏恢复保证。只删除单个正文而保留清单不能视为合法修复。

## 设备本地日历与关闭

update-time 使用设备本地时区 HH:mm。Rust 每秒观察是否到期，按民用日期补最近一个到期槽，不将一天假定为固定 86400 秒。错过多天只补一次；夏令时跳过的时刻在之后首次观察时补跑，重复的时刻按日期去重。失败自动重试至少相隔 60 秒，尝试日志先持久化；重启及墙钟回拨不会绕过退避。手动更新可绕过退避。系统休眠/进程退出不会被此工作器主动唤醒，恢复后再判断，不能承诺网络请求严格恰好一次。

Manager 拥有运行控制器，前端只提交配置、刷新、关闭意图及读取状态。关闭先撤销 Authority，再取消下载/等候者，等待已准入的阻塞解析、编译和系统查询实际结束，退出后台工作器并显式释放目录锁。旧 Arc 不保留资源占用权，真实正文内存仍计预算。`active=false` 仅表示停止准入，不是关闭完成证明；`Manager::stop().await` 返回才表示必要清理完成。系统域名查询不能由标准库强制中断，关闭会等待它真实返回，不虚报释放。

## 公开接口与错误

Rust 使用 `Arc<Manager>::apply_configuration_async`；自定义受信根使用 `apply_configuration_with_client`。同步 apply_configuration 只用于没有活动提供者的本地配置，不能绕过既有提供者所有者。`refresh_providers(None)` 刷新所有已声明提供者，`Some(name)` 刷新单项，未知名称失败；`provider_status` 返回只读状态，不返回 URL、正文、凭据或可变运行句柄。

现有 C ABI v2 JSON 命令增加 `provider_status`、`provider_refresh`（name 可选），configuration_apply 复用异步内核事务并返回 provider_status。无新增通信架构或 Debug 权限入口。示例见 [rule_providers.rs](../examples/rule_providers.rs)。本轮不修改 App。

失败沿用 `src/error.rs` 固定整数协议：非法配置/正文 1001，严格 YAML 语法/字段 2001/2002，资源超限 8001，状态/租约冲突 9001，取消 9002，未支持能力 1021；HTTPS/DNS/网络返回真实底层对应类别，整次下载超时使用 NetworkTimeout。完整代码以[错误代码查询表](错误代码查询表.md)为准，不根据自然语言判断。失败保留旧发布方案，不默认 DIRECT。后台失败保留 error_info，纯本地编辑不假冒下载恢复。

## 验证边界

同名报告位于 tests/test_providers.md、test_providers_cache.md、test_providers_document.md、test_platform_providers.md、test_provider_contract.md；HTTPS、FFI 与规则回归以各自报告为准。测试使用私有临时 HOME 和真实回环 TLS/目录交换，不修改用户 HOME 或系统代理。Linux 文件系统、实际夏令时切换、长期睡眠/重启、公网源及真实系统 VPN 不属于本机回环测试通过的证明范围。旧性能文档数值不作为本轮提供者性能结论。

进程规则来源约束：process-name 只在 macOS 本地配置参与；iOS/iPadOS/tvOS 本地编译忽略；远程规则集一律单行忽略（direct、reject 或节点策略均相同）。无法由操作系统确定流归属时跳过进程规则继续匹配，禁止接受远程自报进程身份。

## 独立语义核验及实际证据（L3/L4）

本轮基线是 dev 03b3081b98dbd70be284fa19feea9910b8479f8b 加工作区变更，不是纯净提交验收。失败原始记录保留在 /private/tmp/fluxa-providers-second-20261017（定时测试两个执行者竞争）、/private/tmp/fluxa-provider-final-20261017 和 /private/tmp/fluxa-provider-ffi-retry-20261017（同步 ABI 异步上下文适配）。修复后专项断言通过；最终完整证据以 /private/tmp/fluxa-provider-accepted-20261017/run.json、同名原始日志及 tests 报告的退出码和指纹为准，文件缺失或 STALE 不计通过。证据未上传 CI，临时目录不构成长久归档保证。

| 独立业务规范 | 实际源码边界 | 独立输入与预期 |
| --- | --- | --- |
| 正文无策略、Debug 和远程进程权限 | providers/document::condition；configuration/rules::compile_sources | 三种策略均忽略远程 process-name；自带策略、Debug、final 不注入快照，合法同行保留 |
| 显式 CIDR 在同来源 GeoIP 之前 | routing/runtime::candidate_priority 和参考线性选择 | test_geoip 正反输入顺序均选 CIDR；跨来源与特殊 REJECT 约定不变 |
| 任一源失败不部分提交 | providers::update 先 load/prepare/stage，再 commit_provider_configuration | 第二源损坏时两个旧文件和旧版本都保留；旧 UDP 连接仍传送 |
| 本地编辑不重下或改写缓存 | providers::verify_cache/load/update | 文件 inode 和 HTTPS 请求数不变；策略切换重新统计忽略数；真实文件损坏拒绝复用 |
| Arc 不代表资源占用权 | providers::request_close、Authority::wait_idle、Cache::close | 延迟 HTTPS 中并发关闭；旧 Arc 存活但新 Cache 可锁定，旧 refresh 报错 |
| 内核掌握日历和退避 | providers::slot/due/automatic/attempted；platform::providers::local_day | 到期失败保留旧规则，立即 tick 不重复下载；恢复成功同一日期不再下载 |
| 错误不默认放行、不脱离所有者 | providers 的 Result 传播；ffi::run_network 的任务前准入 | 同步 ABI 嵌套调用返回 9001 且版本不变；未知名称失败，既定 final,direct 不等于异常回退 |
| 预算与恢复记录有界 | 下载/解析/正文/索引预算；cache::open | 旧清单和尝试日志各 128 条，恢复只保留最新 128 条；预算超限拒绝，不绕过限制 |

表中预期来自用户业务规范，不由生产实现反推。L1 静态检查和 L2 测试一致不替代 L3。L4 通过真实日志、报告指纹、退出状态与源码定位核验范围；Linux、实际夏令时转换、断电、公网、Swift 和系统 VPN 仍未验证。

缓存实现补充：尝试记录在清单和独立日志中均以最多 128 个 {key: URL哈希, time: Unix秒} 结构记录序列保存，不使用超过严格 YAML 64 字段门禁的映射。恢复拒绝重复哈希和非法时间，旧清单与新日志合并保留最新 128 条。回归发现并修复了旧映射表示写出后无法重新加载的问题，失败证据保留于 /private/tmp/fluxa-provider-accepted-20261017/test_providers_cache.attempt-1.log，修复专项证据位于 /private/tmp/fluxa-provider-cache-retry-20261017；最终以完整回归最新报告为准。

并发补充：providers::status 在取得 State 前先结束 Authority 检查，保持与提交路径相同的锁顺序，避免状态查询和原子发布反向持锁；test_providers 的真实刷新并发状态查询保留最终策略断言。常驻调度使用 lifecycle::runtime 的内核运行时，scheduler_outlives_request_runtime 验证外部请求运行时销毁不等于关闭内核；关闭仍由 stop 的撤销和等待完成协议裁决。缓存恢复与生命周期修复后的专项原始记录见 /private/tmp/fluxa-provider-lifecycle-verified-20261017；最终证据仍以完整回归最新记录为准。

GeoIP geoip.mmdb 与提供者 YAML 共用整目录交换事务。状态清单写入版本 2，读取版本 1/2；新增可选 GeoIP 来源哈希、正文摘要及本地日期槽。启动只流式验证已有二进制摘要，不以磁盘文件冒充 Ready；启用时重新下载并验证，未启用不初始化。禁用后保留的库在规则更新时用有界缓冲复制。
