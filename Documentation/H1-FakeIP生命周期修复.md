# H1：Fake-IP 会话权限、资源租约与确定性关闭修复

日期：2026-10-14。作者：可莉唯一的狗。范围：内核 dev 的 DNS/Fake-IP、TUN UDP、共用出站及相关 FFI。依据《API全生命周期管理》及《会话生命周期与资源所有权隔离原则》，不是引用计数回收方案。

## 缺陷与独立预期

原 H1 的全局 TUN_POOLS 永久保留旧配置，改变重叠范围或累计 16 个历史配置后拒绝新会话。已取消的 9357bfc 候选虽然改成 Weak，仍将外部对象存活当成占用权：保留旧解析器导致 /16 → /15 返回 9001；八个已 close 会话仍占额度，第九个返回 8002；旧解析器关闭后仍能分配地址。实际旧实现取证见本文“取消候选的所有权取证”，不能用该候选 230 项通过证明新规范。

本次独立预期：关闭立即撤销权限，完成必要清理后才返回成功；旧 Arc 仅保留数据，不能恢复活动使用权，也不能阻止新会话合法重绑。保存的实际配置/映射/缓存内存仍受预算约束。DNS 失败、资源失败和取消不得变成 DIRECT 或伪造成功。

## 生产关闭链与锁边界

| 实现 | 职责与不变量 |
| --- | --- |
| src/lifecycle/session.rs Authority | 内核非复用身份与 Generation；Active → Closing → Closed/Failed；同一状态锁检查与登记操作/提交，关闭不依赖 Arc 数量；Notify 先启用后检查以避免丢失完成通知 |
| src/dns/fake_ip.rs Pools/Table | 依据有效权限状态检查逻辑地址租约，Closed 记录可回收，Closing/Failed 不冒充已释放；表锁 → 池锁 → 权限锁，分配提交与撤销串行；历史地址继续禁止重新归属 |
| src/dns/mod.rs Resolver | 所有解析器均登记 TUN 会话归属，包括未启用 Fake-IP 的解析器；旧对象查询/分配/绑定返回 9001；关闭监听、取消查询、销毁上游、等待驱动退出后归还活动解析器额度；旧元数据预算保留到真实 Drop |
| src/dns/runtime.rs ManagedRuntime | 驱动任务由内核自有运行时托管、有限任务/预算准入，停止时 abort 并排空 join；驱动预算错误通过通知立即返回真实原因，不误报 DNS 超时 |
| src/dns/service.rs Control | 实际 TCP/UDP 双绑定后才登记于解析器；关闭竞争登记失败则等待回滚；保存共享 join 状态，取消某个等待者不会提前移走 JoinHandle；解析器关闭直接等待监听与子任务退出 |
| src/outbound/configuration.rs NetworkSnapshot | 仅保留配置与受控对象，不持有直接 Socket；新会话从已关闭解析器的验证元数据创建新对象/代次，不恢复旧权限 |
| src/outbound.rs Connection | 真实 Socket 在可撤销资源槽内；拨号与已建资源都有在途/资源租约计数；权限撤销后禁止登记，失败回滚；资源释放先于归还操作计数；即使连接已移出注册表，关闭也不会漏等真实资源 |
| src/outbound/stream.rs ManagedStream | 旧流对象只保留受控资源槽，每次轮询核验权限；确定性清空槽释放真正 TCP；Arc::get_mut 仅用于 Rust 独占数据转换，不判断占用权 |
| src/tun/udp.rs Session | 内核托管清理任务，调用方取消等待不会取消清理；撤销后取消所属连接/DNS、等待在途操作及接收任务、清空输出/流表并归还会话配额；并发/重复 close 等待同一结果 |
| src/ffi.rs | C ABI/Swift close 签名保留，正常同步调用等待内核完成；关闭登记不提前 take。同步 FFI 不能在 Rust 异步线程中阻塞待完成资源；空资源可同步完成，需异步等待时返回状态错误，不嵌套运行时或伪造完成 |

关闭失败记录 Failed 和实际错误，不宣称资源可重用；错误路径仍可能需要后续诊断。纯读取规则数据的 routing::Snapshot 没有 Socket，不因为 H1 无故改动规则匹配/裁剪。新的会话不得借旧身份再次使用资源。

## 历史隔离、预算与错误

- IPv4 历史位图 16 KiB 覆盖 198.18.0.0/15；IPv6 合并区间最多 4096，首次登记从原全局 Fake-IP 16 MiB 预算保留 256 KiB。历史额度限制实际碎片，不限制已关闭配置次数；不持久化域名或凭据。
- 会话活动上限仍为 8，解析器活动上限 16；关闭完成立即回收。映射/缓存/配置预算随真实数据生命周期保留，不能通过 close 或旧快照长期引用绕过。元数据全局 16 MiB；驱动全局 16 MiB、每项保守预留 128 KiB、单解析器最多 64 项。
- 9001 为已撤销/不符身份等状态错误；8001 为数量、地址/碎片额度等上限；8002 保留实际预算申请量与限额；原取消代码和失败语义保留，关闭等待/资源耗尽不转换为策略 REJECT。没有新增或改号错误代码，40 项查询表继续自动检查。
- 本文预算是可核验准入与已知容量估算，不能声称全部上游/外部 TLS  opaque 对象、分配器和线程栈的实际 RSS 都被准确计量；外部自定义 TLS/信任锚内部容量的完整计量尚未验证。

## Rust 迁移与可运行示例

```rust
// dev 公开 Rust 关闭接口允许调整；必须等待内核实际完成。
session.close().await?;
manager.close(connection_id).await?;
manager.close_all().await?;
manager.stop().await?;
resolver.close().await?; // resolver 为 Arc<Resolver>
// 不需要 drop 旧 session、resolver 或 snapshot 才能重绑；旧对象不能恢复活动操作。
```

实际可运行：cargo run --example fake_ip_reconfigure。示例故意保留旧会话、解析器及快照，关闭后扩大 /16 为 /15，确认旧地址无新映射、旧操作失败。无系统 VPN、无上游查询。现有调用方和测试同步 async；未删除原断言或将业务预期改成新实现输出。

## 独立运行证据

- Tests/fake_ip_lifecycle.rs：保留旧引用 /16 → /15；同一 Manager 自动建立新代次；40 组闭合对象继续存活、并发重复 close 和取消等待后内核仍清理；真实监听/空闲 TCP 子连接/在途 DNS 查询停止后同端口 TCP/UDP 重绑；8 MiB 保留配置 close 后仍阻止下一次超预算申请，真正 Drop 后恢复准入。
- outbound::stream::lifecycle_tests：保留 ManagedStream、旧快照与解析器，关闭 TUN 会话后远端真实 TCP 收到 EOF；旧流写入失败；同 Manager 可创建新会话。
- lifecycle::session 两项测试：身份/Generation、在途未完成禁止成功 Closed、失败不是成功。dns::runtime 测试：驱动预算失败不误报 DNS 超时。
- 原 Fake-IP 六项地址历史/双族/额度/并发分配/失败原子性测试保持；原 DNS/TUN/出站/错误/规则回归继续执行。
- 真实 Swift FFI 全 17 组通过，包括 TUN DNS 40 次关闭与 20 次重叠区间变更；最后的驱动原因传播修改追加 DNS FFI 复验，不能把旧编译产物当成新改动证据。

最终计数、命令与源码提交记录在本文“最终验收证据”中，只有实际命令完成后才登记。

## L1–L4 与未测边界

L1：格式、全部目标 check、严格 Clippy 实际执行。L2：全量 Rust 以单线程夹具模式执行，原全局八会话预算没有放宽，独立并发测试仍执行；真实 FFI 与契约/示例验证单独记录。早期未提权监听失败和原并行夹具额度争用不计入通过。

L3：独立依据用户十三条原则核对有效租约、真实释放、旧对象失效、生成/撤销同锁、在途拨号回滚、历史不复用和实际预算保留；不以编译或差分一致代替。L4：实际代码、旧实现复现、最终测试日志、兼容性与边界可追溯。

未验证：真实 macOS Network Extension/系统 VPN、最低部署版本、其它架构/平台、跨重启缓存隔离、所有分配故障/锁中毒注入、长期高并发压力和整个进程 RSS 硬上限。暂停的自研 TCP/TUN 下一阶段未改动。

dev API 可不兼容改变；未经授权不合入 main，不发布 crates.io，不修改 App。取消的 9357bfc 二进制候选没有标签/上传，不能恢复发布。Package.swift 恢复最后已发布 v2.4.1 URL 与原 SHA-256；H1 的本地验证内核与该远程旧包必须区分，未生成或发布新的远程包。

## 取消候选的所有权取证（合并归档）

原独立取证完整保留，明确区分旧候选与最终修复；对象引用计数不再作为资源租约依据。

## H1：Resolver、Snapshot 与 Pool 所有权关系核验

核验对象：旧候选 9357bfc；已取消，不能作为最终修复。本报告区分源码确认与实际运行结果，不以引用计数或编译结果证明业务正确性。

| 文件与类型 | 实际所有权关系 | 实际资源 | 结论与新约定差异 |
| --- | --- | --- | --- |
| src/outbound/configuration.rs NetworkSnapshot | resolver: Option<Arc<Resolver>> | 间接持有解析器资源 | 外部旧快照可延长 Resolver 对象；不能据此延长业务租约 |
| src/dns/mod.rs Resolver | inner: Vec<TokioResolver>、fake_ip: Table、_permit: SemaphorePermit | 上游连接池与 DNS 客户端额度；缓存另有 ByteLease | 配额和池仍依赖对象析构，须增加显式撤销、停止和完成边界 |
| src/dns/fake_ip.rs Table/State、Pools | Table.namespace: Arc<()>；Pools 的 Weak 计数决定 active 占用 | Fake-IP 逻辑地址空间，没有监听 Socket | 当前候选违反禁止引用计数判定占用的原则；原 H1 则为永久全局 active/历史范围 |
| src/dns/service.rs Service | 工作任务持有 TcpListener、Arc<UdpSocket> 及服务额度；子任务克隆 Resolver | 真正的本地 TCP/UDP DNS 监听 Socket | 监听器不属于 Fake-IP Pool，也没有 Resolver → Service 的所有权反向边；stop 会取消并等待工作及子任务退出 |
| src/tun/udp.rs Session | dns: Arc<Resolver>、_lease: ByteLease、各 Flow JoinHandle | 会话额度、连接、后台任务 | close 只标记和 abort，不等待任务退出；_lease 仍随对象析构释放，必须改造 |
| src/outbound.rs Connection、Manager | 注册表保存 Arc<Connection>；Connection 持有 Socket 与 ByteLease | 出站 Socket 与连接预算 | close 发取消信号、移出注册表；不能仅凭此认定所有活动引用及 Socket 已释放，需核验关闭完成与获取竞态 |
| src/routing/runtime.rs Snapshot | 规则 Document、Entry、Index、FlatIndex 与 ByteLease | 规则数据与预算；没有监听 Socket 或 DNS Pool | 不应误报为监听器占用；旧快照保留的实际内存仍须计入预算 |

### 上游池源码证据

固定依赖 Hickory 0.26.3：resolver.rs 的 Resolver 持有客户端/池；name_server_pool.rs 的 NameServerPool 通过 Arc<PoolState> 管理上游；name_server.rs 的 NameServer 持有 AsyncMutex<Vec<ConnectionState>>。runtime.rs 的 TokioHandle 持有 Arc<Mutex<JoinSet>>。这些为上游连接/驱动任务，不是本项目 DNS Service 的本地监听器；仅销毁外层 Snapshot 不是确定性停止接口。

### 实际运行取证

在旧候选干净副本运行 `/private/tmp/fluxa-h1-publish-242/examples/ownership_probe.rs`，没有改写被测生产实现。日志 `/private/tmp/fluxa-h1-owner-probe.log`：

1. close 后保留旧解析器/快照，旧解析器仍成功分配 Fake-IP：违反新约定。
2. `/16` → `/15` 重绑定仍返回结构化 9001：外部旧引用仍阻止绑定，违反新约定。
3. 保留八个已经 close 的 Session，第九个返回 8002，params 为 actual=1、limit=8：会话额度未在关闭时释放，违反新约定。不能误写为普通 8001。
4. 保留旧 Resolver/NetworkSnapshot，DNS Service.stop 等待完成后，同端口 TCP/UDP 系统重绑定成功：证实旧快照没有通过 Fake-IP Pool 持有该监听器。

### 本次改造状态

生产链已由独立权限组件接入 Resolver、Fake-IP、TUN、共用出站与 DNS Service。旧引用仅保留只读数据和实际内存预算，不能继续使用活动资源。实际 TCP EOF、监听重绑定、取消查询、40 组旧引用及关闭后配置预算保留验证详见 [本次 H1 审查](H1-FakeIP生命周期修复.md)。旧候选的测试结论不代替本次验证。

## 最终验收证据（合并）

本节保留最终实跑计数、命令、原始日志与未验证范围；H1 源码已提交并推送 dev：eac1a37908fedc9fe17e060c651a4e4a53779e5a。

## H1 最终验收记录

日期：2026-10-14。作者：可莉唯一的狗。生产修改来自已验证隔离副本并同步到 dev，Git 提交记录确定最终源码身份；不能把已取消的 9357bfc 当作最终修复。

| 层级 | 实际执行/证据 | 结果 |
| --- | --- | --- |
| L1 | cargo fmt --check；cargo check --all-targets；cargo clippy --all-targets -- -D warnings | 通过，严格检查未放宽 |
| L2 | cargo test --locked --no-fail-fast -- --test-threads=1 | 234 通过、0 失败、1 既有外部服务测试忽略 |
| L2 | bash check-ffi.sh all | 全部 17 组真实 Swift→C ABI→Rust 通过 |
| L2 | 最后驱动错误传播修改后 bash check-ffi.sh dns | Fake-IP、UDP/TCP/DoT/HTTP2 DoH、关闭及100次回收通过 |
| L2 | cargo run --example error_contract -- --check | 40 项全部登记，重复/遗漏/未定义/空字段均0 |
| L2 | cargo run --example fake_ip_reconfigure | 保留旧对象时 /16→/15 重绑通过；旧 IPv6 ::1 不重用，新地址 ::2 |
| L3 | 独立生命周期测试及生产关闭/获取锁边界核验 | 40组旧引用、旧操作失败、并发关闭、取消等待、实际监听重绑/TCP EOF、配置预算保留、真实资源原因均有独立预期和实跑证据 |
| L4 | 旧候选实际复现、最终源码、原始日志、迁移示例及未测范围 | 可追溯；不以技术检查代替全平台或业务证明 |

原始本机日志位于 /private/tmp/：fluxa-h1-final-verification.log、fluxa-h1-clippy.log、fluxa-h1-check-final.log、fluxa-h1-ffi-final.log、fluxa-h1-final-dns-ffi.log、fluxa-h1-final-contract.log、fluxa-h1-final-example.log。生产关闭链、测试文件及迁移说明见 [H1修复审查](H1-FakeIP生命周期修复.md)。

早期沙箱监听权限失败、迁移遗漏、并行夹具共享八会话配额争用、取消错误误报及嵌套运行时回归均保留失败日志，修复后重跑；没有删除测试、减弱断言或扩大原额度。单线程夹具不代替独立并发测试。

未验证：系统 Network Extension/VPN、最低系统、全部平台、跨重启缓存隔离、全部分配/锁中毒故障、长时间压力及全部上游/外部 TLS 对象和分配器 RSS 的精确计量。

dev Rust 异步关闭为已允许 API 改造，调用、注释、示例同步；C ABI v2/Swift 签名保留。main 未合并、crates.io 未发布、App 未修改。远程 Swift 包恢复已发布 v2.4.1 URL与原校验值，不能把它当成本轮新 H1 二进制。新规范发布守卫和权限配置的完整落地不属于本次 H1 已验证项。
