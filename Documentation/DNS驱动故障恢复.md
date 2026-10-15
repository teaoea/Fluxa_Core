# DNS 驱动准入故障恢复与验收

作者：可莉唯一的狗。日期：2026-10-15。

## 已确认缺陷与复现

基线提交 `8431d5ed9843e5c13ad4f0643b198c8612948e71`：`src/dns/runtime.rs` 的 `Tasks::failed` 只向 watch 写入 `Some(error)`，`ManagedRuntime::guard` 立即读取历史值；`src/dns/mod.rs` 的 `Resolver::resolve_allowed` 还在查询开始及结束各读取一次该历史值。没有任何恢复边界。

最小复现：构造运行时，记录一次 `Error::ResourceLimit`，然后执行独立的 `guard(async { Ok(()) })`。修复前新增测试 `historical_admission_failure_does_not_poison_new_query` 实际失败：期望 `Ok(())`，实际 `Err(ResourceLimit)`。这不是超时推断，而是运行得到的永久失败路径。生产入口会在后续任何上游查询之前拒绝；缓存和 Fake-IP 也被前置检查误伤。

真实触发入口是 `ManagedHandle::spawn_bg`：清理已结束的任务后，仍达到 64 个后台任务则返回 `8001`；共享 16 MiB 任务预算无法再预留 128 KiB 则返回 `8002`。Hickory 0.26.3 的 `Spawn::spawn_bg` 没有 Result 返回，必须另外向受影响操作传播准入错误，不能简单删除全部故障传播后等待 DNS 超时。

## 独立业务语义与修复

约束依据是资源超限拒绝当前操作、不绕过预算、不静默降级，以及内核负责恢复和会话撤销、前端不管理解析器。瞬时资源竞争不等于解析器永久损坏；会话关闭也不等于瞬时资源竞争。

- watch 保存最近故障及其版本，语义改为**事件通知**，不作为永久健康标记。
- 每次上游查询在首次轮询查询 future 前建立事件边界，只观察此后的驱动准入失败。不会清空共享状态；同一解析器同期受影响的查询保守失败，后开始的查询不消费历史错误。
- 轮询查询完成后再次检查版本，覆盖同一次 poll 内 `spawn_bg` 失败而查询立即返回的情况，禁止错误变成功。
- 移除 `resolve_allowed` 两处读取历史错误的门禁。缓存、Fake-IP 仍受原有输入校验、查询额度与会话权限约束。
- 后台任务数、字节预算、任务所有权、取消及排空规则保持原值；持续压力仍明确失败。没有自动重试循环、备用解析器、系统 DNS 或 DIRECT 回退。
- guard 开始和结束验证会话身份；`spawn_bg` 仍通过 `Authority::with_active` 原子检查与提交。正常 Resolver 入口另有取消分支及结束检查，不能恢复被撤销的旧对象。

事件传播保持解析器级保守处理，不承诺精确定位到某一条共享连接的单独请求。这与原先对同期查询全部失败的范围相同，区别在于后续新操作可以重新准入。watch 只保留一个定长 Error，不增加错误队列或无界历史。

## 测试与示例

新增六项回归，并调整原有预算失败测试为查询期间注入：

| 测试 | 独立预期 |
| --- | --- |
| historical_admission_failure_does_not_poison_new_query | 新操作不会被历史错误拒绝 |
| background_budget_failure_is_not_dns_timeout | 受影响操作返回真实预算错误，不误报 DNS 超时 |
| same_poll_failure_cannot_be_returned_as_success | 同一次轮询故障不能返回成功 |
| concurrent_queries_observe_event_without_clearing_each_other | 并发旧操作均得到错误，新操作不清除其通知 |
| task_budget_exhaustion_is_retryable_after_lease_release | 走相同准入流程的隔离预算返回 8002；释放预算后任务成功，排空后占用归零 |
| revoked_runtime_cannot_recover_or_spawn | 会话撤销返回 9001，不能启动新驱动 |
| all_transports_recover_after_worker_limit | UDP、TCP、DoT、HTTP/2 DoH 各自实际触发 64 任务限制；返回 8001，排空占位任务后同一解析器真实查询与缓存成功，关闭后旧引用返回 9001 |

预算测试使用独立静态 ByteBudget 和生产共用的私有准入方法，避免耗尽并行测试的全局预算；没有公开注入接口。四协议测试占位任务也有真实预算租约。真实查询夹具复用 `Tests/support/dns.rs`，DoT/DoH 使用仅该解析器信任的测试证书，不更改系统信任。

可运行示例：`cargo run --offline --example dns_recovery`。示例只运行本地 UDP 查询、缓存与关闭验证；确定性资源耗尽示例由上述私有测试提供，不伪造外部可以耗尽内核预算的 API，也不声称示例本身注入了资源故障。

## L1–L4 证据

- L1：`cargo fmt --check`、`cargo check --offline --all-targets`、`cargo clippy --offline --all-targets -- -D warnings` 已实际通过。
- L2：七项 DNS 驱动回归已实际通过，四协议真实查询恢复通过；`cargo run --offline --example dns_recovery` 已通过。完整串行 Rust 回归为 262 项通过、0 项失败、1 项原有外部互通测试忽略；五项真实 Swift FFI（dns、dns_service、tun_dns、lifecycle、outbound）全部通过；错误契约检查为 43 项代码全部登记，重复/遗漏/未定义/空字段均为 0。
- L3：按资源失败仅拒绝受影响操作、关闭不允许恢复、错误不误报成功/超时逐条核对。事件边界位于 future 首次轮询之前；完成后检查覆盖同 poll，且没有跨查询清空。规则引擎、Debug、DNSSEC 验证分支与上游策略未修改；本次不宣称覆盖全部依赖内部状态或所有并发交错。
- L4：修复前失败断言、修复后七项测试、四协议真实回环结果与示例输出均已取得。基线及源码位置可追溯；临时日志在 `/private/tmp/fluxa-dns-fault-*`，不作为需要打包的运行数据。

## 兼容性与边界

不新增或改号错误代码：8001 为任务数量上限，8002 为预算准入失败，9001 为会话状态无效。Rust 公开接口、C ABI、Swift 封装、配置字段、缓存格式及规则优先级均无变更。查询表通过 `ErrorContractIntroduction.md` 同步生成说明，权威代码仍为 `src/error.rs`。

真实系统 VPN、外部 DNS 服务、最低部署系统与跨平台运行本次未验证；回环恢复不代表系统隧道已验收。不发布 crates.io、不合入 main、不改 App 或固定二进制依赖。

## 实际命令与补充边界

| 实际命令 | 结果 |
| --- | --- |
| `cargo fmt --check` | 通过 |
| `cargo check --offline --all-targets` | 通过 |
| `cargo clippy --offline --all-targets -- -D warnings` | 通过 |
| `cargo test --offline --lib dns::runtime::tests -- --test-threads=1` | 7 通过 |
| `cargo test --offline --all-targets` | 首轮并行：254 通过、1 失败、1 忽略；遇到失败后未执行剩余目标 |
| `cargo test --offline --all-targets -- --test-threads=1` | 完整重跑：262 通过、0 失败、1 原有测试忽略 |
| `cargo run --offline --example dns_recovery` | 本地查询、缓存、关闭后 9001 均通过 |
| `cargo run --offline --example error_contract -- --check` | 43 项契约一致 |
| 缓存构建的静态库 + `swiftc` 编译并运行五个 `Tests/ffi_*.swift` 客户端 | DNS、DNS 服务、TUN DNS、生命周期、出站全部通过 |

完整并行首轮的失败位于 `Tests/tun_dns.rs` 的 `ordinary_mapping_before_and_after_tun_remains_owned_and_never_collides`，`Session::new` 返回 `ResourceBudgetExceeded { actual: 1, limit: 8 }`。实际对应 `src/tun/udp.rs` 的进程级 `SESSIONS` 配额，串行重跑该项及整个目标均成功。本次未改该配额或测试断言；并行测试间的全局名额竞争作为独立测试隔离风险保留，不宣称默认并行全套已经通过。后续可独立治理测试名额协调，不属于本 DNS 修复范围。

验证在基线 8431d5e 的隔离源码副本执行；交付只包含本次六个文件，未纳入其他在途规则缓存修改或文档删除。初次新增示例对依赖 API 的调用编译失败，已按锁定的 Hickory 0.26.3 报文类型修正，并重新执行全部静态检查、示例和完整串行测试；不把先前失败计为通过。
