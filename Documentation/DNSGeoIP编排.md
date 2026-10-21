# 主配置 DNS 与 GeoIP 编排

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

MMDB Reader 持有受预算约束的共享不可变正文；单库最多 256 MiB，总预算 512 MiB。文件类型、最终链接、长度及读取期间变更在共用只读边界检查。来源认证由宿主负责，不以文件校验代替 HTTPS 或签名。资源路径及运行规则内存导出见 [只读文件接入](共享存储.md)。

## 示例与独立语义证据

离线示例：cargo run --offline --locked --example main_dns_geoip，仅查 hosts、不下载。显式传配置及资源清单才只读加载资源；完整主示例含未实现监听/TUN/MITM 等，不删活动项制造成功；纯校验使用 validate_main_configuration。

测试及同名报告覆盖 DNS 路由、地址族、GeoIP 就绪门禁、损坏替换保留旧快照与 API 状态一致性。临时目录由测试宿主准备，不证明 App Group 签名或真实系统 VPN。旧下载／目录交换／调度验收不适用于当前架构；外部代理节点、移动真机、最低系统和系统接管仍须独立验证。
