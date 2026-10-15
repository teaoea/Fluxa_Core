# IP 分片与扩展头阶段审查

作者：可莉唯一的狗。范围：`src/tun/ip.rs`、共用 Session 分派及其真实 Rust/Swift 回归。
本阶段仍在 dev，不包含系统 VPN、正式发布、ICMP/PMTU 或后续 TCP 扩展。

## 独立规范与 L3 语义核对

| 约束 | 源码与应有后果 | 独立断言/边界 |
| --- | --- | --- |
| RFC 791 / RFC 8200 | 每片严格验证 IP 长度、TTL/Hop Limit、保留位和偏移；IPv4 头校验和有效 | 独立手工包及全部截断/字节变异，不从生产编码器生成预期 |
| RFC 5722 重叠处理 | 重叠整组作废；在 60 秒绝对截止前保留墓碑，不能重建 | 重复首片、后续尾片、59 秒重试及 60 秒到期分别断言 |
| RFC 7112 首片 | 必须包含剩余扩展链和完整传输头 | 不完整 TCP 首片拒绝；未完成重组时真实对端收不到数据 |
| RFC 6946 原子片段 | IPv6 offset=0/M=0 独立处理，不污染普通片段缓存 | 同标识普通重组在原子片段前后仍可完成 |
| RFC 8200 扩展头 | Hop-by-Hop 只能位于固定头之后，不能剥掉 Fragment 后伪装成首头 | Fragment 下一头为 Hop-by-Hop 必须拒绝 |
| 完整扩展链预算 | 分片前后头数/字节总计；不能分别重置计数 | 8 字节 Hop-by-Hop + 8 字节 Fragment + 248 字节 Destination Options 共 264 字节，明确资源拒绝 |
| 片段一致性 | 相同标识的基础头和未分片前缀变化时整组废弃 | 前缀摘要用于一致性比较，不作为来源认证或密码安全声明 |
| 输入不触发提前放行 | 完整包仍经过原校验和、规则、DNS/Fake-IP 与出站 | 重组前不拨号；错误不转 DIRECT；既有规则与生命周期完整回归 |
| 会话权限 | 正在关闭/已关闭不能再存片；配置 revision 和映射 generation 隔离 | 真实双栈对端、并发关闭及保留旧引用的预算断言 |
| 资源真实释放 | 租约覆盖双缓冲；任务未首次轮询或被中止也须清理 | 全局预算耗尽无部分登记，未轮询 future 析构归还，关闭后未完成缓存归零 |

未知选项 action=00 允许跳过，其余明确拒绝；不宣称已经实现相应 ICMP Parameter Problem。
AH/ESP 不剥除后转发，未执行源路由，不协商任何未实现 TCP 扩展。重组上限为既有最大完整包 9000 字节；
这属于明确资源边界，不宣称支持 65535 字节完整重组、Jumbogram 或输出分片。

## 已确认并修正的实现问题

1. 最初清理仅位于后台任务取消分支，但既有会话关闭会 abort 登记任务。真实回归显示关闭完成后仍占用
   **18512 字节**重组预算，外部持有 Session 时不能自动释放。现在由任务按值持有 RAII guard，未轮询及 abort
   均执行清理；并发关闭、旧引用保留回归要求预算归零，不靠释放外部引用通过。
2. 初次规范化移除了 Fragment 之前的扩展头，可能把 Fragment 之后的 Hop-by-Hop 误当首头，并分别计算两段
   链预算。现在在 Fragment 入口拒绝该位置错误，保存未分片前缀摘要与已消耗头数/字节，并在首片及完成路径
   核对整条链。用独立的 264 字节向量验证资源拒绝，而非依赖重复选项触发其他错误。

## L1 / L2 / L4 执行与证据

执行入口如下，实际最新状态以同名测试报告为准，未执行/STALE/失败不能算通过：

```sh
cargo fmt --check
cargo check --locked --offline --all-targets
cargo clippy --locked --offline --all-targets -- -D warnings
python3 Tools/test_reports.py --external test_test_reporting -- python3 tests/test_test_reporting.py
python3 Tools/test_reports.py --run
cargo run --locked --offline --example error_contract -- --check
python3 Tools/test_reports.py --check-reports
```

Fluxa 仓库以显式 `FLUXA_CORE_ROOT` 执行 `check-ffi.sh all` 及 `check-protocol.sh tun_tcp`；生产 App
继续使用固定远程 v2.4.1，不使用本地 Rust 回退。独立对端仅作测试，未引入内核业务依赖。

Linux 在已有 OrbStack Debian ARM64 虚拟机的独立 `unshare --net` 命名空间执行，临时源码与缓存位于
`/var/tmp/fluxa-ip-test-source` 和 `/var/tmp/fluxa-tcp-test`，不修改 macOS 网络。该副本从实际 dev 基线还原 Git
元数据并携带 dirty 源码，报告不得将它称为纯净提交。真实系统夹具核对双栈、1 MiB、丢包、乱序和半关闭；
新增 Rust 用例额外核对真实双栈 UDP 分片路径，Swift 用例核对分片 TCP 路径。

原始证据位于本机 `/private/tmp/fluxa-ip-*`、Linux `/var/tmp/fluxa-ip-linux-*-evidence`。
已发生并保留：Swift 边界方法放错类型和临时借用的编译错误；本机受限环境无法绑定回环端口；测试夹具
误用默认 REJECT；关闭预算未归还断言失败；源码更新后真实 FFI 报告为 STALE；Linux 缺 Git/版本元数据及
链接/报告写入空间不足。每项均按真实后果处理，不把框架失败或旧版本通过算为当前版本通过。

未验证：macOS Network Extension、完整 VPN 启动与回滚、最低系统/Intel、真实用户公网节点、长期高负载及
全部分配/锁故障注入。此记录不代表所有安全场景或所有 RFC 扩展已经验收。
