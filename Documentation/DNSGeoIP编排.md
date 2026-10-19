# 主配置 DNS 与 GeoIP 编排

当前 dev 源码契约，2026-10-17。本文不表示 App、系统 VPN、外部节点或移动设备已经验收。历史阶段记录不能替代当前 tests 同名报告的源码指纹和实际退出状态。

## 入口与事务

Rust 使用 Arc<outbound::Manager>::apply_configuration_async；显式企业 TLS 根使用 apply_configuration_with_client。同步 apply_configuration 只接受无需异步子系统的配置。C ABI 沿用 v2 configuration_apply、configuration_status、dns_query、geoip_lookup、geoip_refresh。没有新错误代码、通信架构或 App 修改。

解析、DNS 引导、GeoIP 下载/验证、规则生成和缓存暂存都在发布前完成；失败返回原错误并保留旧配置。提交检查控制权限和基线版本，一次发布路由、节点、解析器、数据库与状态。缓存提交失败不退休旧 DNS。目录交换后的父目录 fsync 失败不冒称回滚，provider_status 明确 directory_synced=false 并记录失败。

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

关闭/未配置为 WARN Disabled 2410，不初始化/下载，国家行不参与合并。enabled=true 必须有有效 HTTPS URL；下载完整且 MMDB 验证 Ready 2411 后才编译国家规则。同来源更新失败 Err，保留旧库/快照/连接；不同来源不能借旧库冒充 Ready。合法 IP 无记录为 no-match 2412，执行故障为 2409。

新管理器启动启用配置重新下载；不指定更新时间则不周期更新。update-time 使用设备本地民用日期，每日最多成功一次，失败至少间隔 60 秒重试。refresh_geoip 强制下载并重新编译/提交。首次无库时仅控制下载候选暂不含国家规则，不发布为业务快照；Ready 后给后续提供者下载和最终业务候选绑定国家规则。

| 平台 | 资产目录 |
| --- | --- |
| Apple 平台 | 系统 App Group 容器的 rule/，组标识 group.app.fluxa.ios |
| 非 Apple 宿主 | 本轮未提供系统共享容器适配，明确 Unsupported，不回退 HOME |

提供者文件 <name>.yaml，数据库固定 geoip.mmdb，共用受保护目录和整目录交换。清单写版本 2、接受 1/2。启动流式验证已有二进制摘要，不加载为 Ready；禁用不初始化。禁用保留文件，规则更新用 16 KiB 缓冲保留其内容。损坏资产受控拒绝；显式强制刷新可用新验证库修复损坏 GeoIP。摘要保证一致性，不代替 HTTPS 认证或外部签名。

Database Reader 与持久化正文共享 Arc<Vec<u8>>，不为保存再复制整库；预算直到真实数据库销毁归还。256 MiB 单库门禁与预算独立，不宣称预算等于 RSS。I/O 保留系统类别；摘要错误 2406、超限 2407、下载/超时 2403/2404、格式 2405、初始化 2408。没有新增代码，参数沿用中央契约。

## 示例与独立语义证据

离线示例：cargo run --offline --locked --example main_dns_geoip，仅查 hosts、不下载。显式传文件才执行其下载；完整主示例含未实现监听/TUN/MITM 等，不删活动项制造成功；纯校验使用 validate_main_configuration。

| 独立规范 | 源码 | 测试 |
| --- | --- | --- |
| DNS 路由独立、失败不直连 | dns/main.rs build，dns/runtime.rs | test_dns_geoip_contract：direct DoH 遇业务 reject；失败节点无直发包 |
| 每上游完整期限 | dns/config.rs build，dns/mod.rs resolve_allowed | ordered_upstreams_get_independent_deadlines |
| 无部分端点、零 TTL、主动关闭 | dns/main.rs initialize/bootstrap，dns/mod.rs cleanup | test_dns_main：失败重复初始化、零 TTL、保留旧引用及空闲连接关闭 |
| 仅允许失败换族 | outbound/execution.rs | preferred_family_order_ip_reject_and_nxdomain；admitted_family_fallback_survives_configuration_replacement |
| Ready 门禁、失败旧版本保留 | providers.rs update，configuration/rules.rs | geoip_ready_failure_atomicity_and_disabled_rules；test_providers 本地日期幂等 |
| 资产原子提交与损坏拒绝 | providers/cache.rs | geoip_binary_and_rules_share_atomic_batch；私有目录夹具的文件检查 |
| API 状态一致，无独立关闭破坏事务 | ffi.rs | main_dns_and_geoip_status_use_same_configuration |

L1/L2 以真实检查命令和报告退出码为准；L3 以表中用户确认规范核对，不能凭两个实现一致；L4 核对报告、原始日志和指纹。尚未验证：外部 VLESS DNS 成功互通、移动设备路径/权限与 tvOS 清缓存、真实夏令时、系统 VPN、断电恢复、RSS 硬上限。系统代理、局域网共享、文件日志保持 TODO 暂存。

全量回归补充边界：底层固定族 DomainTarget 无地址仍返回 5002，不改为网络拨号失败；普通旧解析器退休保持幂等，已关闭旧引用不阻止 configure_dns 绑定新实例。带缓存提交的 retire_after 仍严格验证旧准入并保持提交失败不撤销。对应原有 test_outbound_dns 与 test_tun_dns 断言完整保留。

OS 引导调用不能强制终止，操作与预算留在阻塞线程直到真实返回；未发布 Resolver 的 Drop 兜底也等待这些操作退出后才归还活动名额。test_dns_mod 的 drop_waits_for_real_operation_before_releasing_quota 保留操作令牌，独立检查 Closing 状态和真实名额归还时点。
