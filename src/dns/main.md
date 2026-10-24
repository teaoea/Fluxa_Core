# main.rs 代码说明

## 职责（干嘛的）

对应源码：`src/dns/main.rs`。本页说明实现职责、数据流程和边界；公开调用的请求／响应契约单独维护于 `Documentation/`，开发约束维护于 `AGENTS/`。

## 目标（想达成什么）

将主配置中的 DNS 上游、自举、专用出站和地址族策略转为可验证的解析计划。

## 主要流程与模块协作

从主配置构造 Plan，复用共用出站与上游解析；准备阶段失败保留旧运行状态，GeoIP 的只读资源由 providers／geoip 参与准备，不在解析器自行落盘。

## 输入、输出与失败边界

输入来自受约束的配置、明确授权的资源或校验后的网络数据；各入口的实际类型、字段和值域以下文及对应源码为准。正常结果只能说明该模块完成其职责，不能推定网络接管、其他模块初始化或外部权限已经成功。非法输入、协议错误、系统故障、取消和资源不足沿用 `src/error.rs` 的真实分类；不静默改为 DIRECT，也不修复或覆盖外部配置。

活动资源由 Core 的会话身份、有效租约、取消及明确关闭操作管理；只读快照引用不构成活动权限。内存与任务的实际存活期计入对应预算。具体模块没有活动任务时，不因本说明增加后台线程或额外权限。

## 测试与示例入口

对应测试及实际结果：[test_dns_main.rs](../../tests/test_dns_main.rs)、[test_dns_main.md](../../tests/test_dns_main.md)。

性能结果及执行记录保存在对应测试文档；下文涉及旧版本的验证陈述仅是历史资料，不是本次重新执行的结果。已发布版本的历史能力不能替代当前代码、真实平台或系统 VPN 验收。

## 实现细节

当前 main 开发源码契约，2026-10-21。本文不表示 App、系统 VPN、外部节点或移动设备已经验收。历史阶段记录不能替代当前 tests 同名报告的源码指纹和实际退出状态。

## 入口与事务

Rust 使用 Arc<outbound::Manager>::apply_configuration_async；显式企业 TLS 根使用 apply_configuration_with_client。同步 apply_configuration 只接受无需异步子系统的配置。C ABI 沿用 v2 configuration_apply、configuration_status、dns_query、geoip_lookup、geoip_refresh。没有新错误代码、通信架构或 App 修改。

宿主提供配置和资源绝对路径；Core 只读解析、验证 MMDB、准备 DNS 并编译规则，全部成功后一次发布。失败保留原有效快照和原始文件，不进行下载、暂存目录、缓存交换或自动修复。权限和基线版本在提交处检查。

新请求读取新配置；已准入建连事务使用原解析器完成地址族尝试；已建立业务连接不随更新关闭。旧解析器立即拒绝新准入，排空后内核停止连接/监听/驱动。close 主动取消并等待释放，不依赖外部 Arc 归零。专用节点连接持有权限令牌，必须先关闭连接再等待令牌归零。实际旧对象内存继续持有预算。

主配置活动时，独立 geoip_configure/close 和 dns_close 在副作用之前拒绝，避免绕过事务破坏状态；统一关闭使用 Manager::stop。前端不拥有解析器或租约控制权。

## DNS

- server 按列表顺序，每项独享 timeout 秒（1–30，越界归一为 3）。数字 UDP、HTTPS DoH、TLS DoT 和显式 system 可混合。
- DNS 的 direct/指定节点是专用出站，不再次查询业务规则；业务访问同域名仍按规则。指定节点失败不会改走直连。
- bootstrap 仅解析 DoH/DoT 上游域名及这些 DNS 使用的节点域名，不解析业务目标。只接受数字引导地址；缺省仅上述引导允许受限系统解析。
- 显式引导保留有界 A/AAAA 候选，取有效 TTL 较小值；零 TTL 不缓存。系统引导无 TTL，按 3600 秒重建。端点全准备成功后一次发布，失败无部分列表。
- hosts 优先 Fake-IP/上游，仅返回对应地址族。DNS 关闭停用 hosts/Fake-IP，使用系统 DNS；启用后仅显式 system 才参与业务列表，全部失败不能自动系统回退。
- OS 解析无 DNSSEC 证明；dnssec=true 遇 system 返回 5003，不伪造 secure。system 支持 A/AAAA，其他类型返回 1021；零 TTL，不伪造缓存期。

## 地址族

主配置 Auto 按 ip-strategy 顺序：prefer-ipv4 先 A/IPv4，prefer-ipv6 先 AAAA/IPv6；only 禁止另一族。NOERROR 无对应记录、无路由、连接失败/超时才可换族。NXDOMAIN、DNSSEC、资源/权限错误及当前候选全部被规则拒绝不触发换族。同族某 IP 被拒绝仅剔除该地址，继续其他候选。数字目标不查询 DNS；only 拒绝不允许的族。

每次地址族建连仍受调用者建连期限约束，DNS 每上游独立期限；不承诺两族总耗时等于单族期限。既有底层 DomainTarget::Auto 不等于主配置顺序策略。

## GeoIP 与资产

关闭/未配置为 WARN Disabled 2410，不初始化/下载，国家行不参与合并。enabled=true 必须有有效 HTTPS URL；宿主文件只读加载且 MMDB 验证 Ready 2411 后才编译国家规则。同来源更新失败 Err，保留旧库/快照/连接；不同来源不能借旧库冒充 Ready。合法 IP 无记录为 no-match 2412，执行故障为 2409。

数据库由宿主下载及校验来源后，通过 resources.geoip 提供现有只读文件。首次启用必须提供数据库，关闭时不得暗中启用；读取、格式及记录验证成功后才编译国家规则。宿主更新文件后须显式重新提交配置，Core 不安排 update-time、不重启下载、不修复损坏文件；refresh_geoip 返回 Unsupported。

MMDB Reader 持有受预算约束的共享不可变正文；单库最多 256 MiB，总预算 512 MiB。文件类型、最终链接、长度及读取期间变更在共用只读边界检查。来源认证由宿主负责，不以文件校验代替 HTTPS 或签名。资源路径及运行规则内存导出见 [只读文件接入](../storage.md)。

## 示例与独立语义证据

离线示例：cargo run --offline --locked --example main_dns_geoip，仅查 hosts、不下载。显式传配置及资源清单才只读加载资源；完整主示例含未实现监听/TUN/MITM 等，不删活动项制造成功；纯校验使用 validate_main_configuration。

测试及同名报告覆盖 DNS 路由、地址族、GeoIP 就绪门禁、损坏替换保留旧快照与 API 状态一致性。临时目录由测试宿主准备，不证明 App Group 签名或真实系统 VPN。旧下载／目录交换／调度验收不适用于当前架构；外部代理节点、移动真机、最低系统和系统接管仍须独立验证。

## 禁止行为（不允许做什么）

不得用未验证的系统 DNS 结果伪造 DNSSEC 证明，不得跳过显式专用出站或失败后替换原有效网络状态。

违反输入、授权或资源边界时，使用实际模块的既有错误／拒绝路径，不伪造成功或擅自扩大权限；具体失败后果以上文流程和公开契约为准。

## DNS实时日志（2.16.1-dev）

Plan::setting只借用实际配置项供日志标记尝试索引；不改变hosts优先、引导、DNSSEC或重试。配置准备、端点重建和预检显式传入Scope，Plan不保存消费者；独立无Scope调用不回放历史。 完整字段、错误和未验收范围见`Documentation/API/External/logging_api.md`与`src/dns/logging.md`。
