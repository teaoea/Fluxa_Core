//! 测试文件：tests/test_budget.rs
//! 测试类型：单元测试、迁移回归
//! 测试对象：src/budget.rs
//! 测试内容：
//! - quota_is_returned_once_and_overflow_is_rejected：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - budget_lifecycle_releases_quota_and_rejects_overflow：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - budget_invariants_hold_at_any_time：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//!
//! 测试依据：保留原测试的业务/格式/授权/资源约束；《测试文件与报告统一管理原则》，独立语义审查不以实现输出反推预期。
//!
//! 前置条件：本地测试需文件/回环套接字权限；平台限定及忽略项以实际登记为准。
//!
//! 执行状态：由同名 Markdown 报告记录，不在源码中固化通过结论。

#[cfg(test)]
mod tests {
    use super::super::*;
    #[test]
    fn quota_is_returned_once_and_overflow_is_rejected() {
        static BUDGET: ByteBudget = ByteBudget::new(10);
        let lease = BUDGET.reserve(8).unwrap();
        assert!(BUDGET.reserve(3).is_err());
        assert!(BUDGET.reserve(usize::MAX).is_err());
        let mut lease = lease;
        lease.shrink_to(2);
        let extra = BUDGET.reserve(8).unwrap();
        drop(extra);
        drop(lease);
        assert!(BUDGET.reserve(10).is_ok());
        assert_eq!(BUDGET.used.load(Ordering::Acquire), 0);
    }
}
// 以下保留迁移前的用例说明；历史执行状态不代表当前版本，当前结果仅见同名报告。
mod public_contract {
    use crate as libfluxa_core;
    // # 字节资源预算测试
    //
    // ## 测试类型
    // 集成测试；回归测试（资源准入的既定行为）。
    //
    // ## 测试对象
    // 生产代码路径：`src/budget.rs`（`ByteBudget::reserve` 的准入判定、`ByteLease` 的归还与峰值统计）。
    //
    // `src/budget.rs` 在 `src/lib.rs` 中是私有模块（`mod budget;`），`ByteBudget` 与 `ByteLease` 均为
    // `pub(crate)`。按《Rust 测试文件头注释强制规范》第八条，不得为编写测试而扩大生产 API 可见性，
    // 因此本文件不直接构造预算对象，而是经公开准入路径覆盖其可观测行为：
    // `libfluxa_core::routing::runtime::{memory_usage, MemoryUsage}` 与 `Snapshot::generate`。
    //
    // ## 测试内容
    // 1. `budget_lifecycle_releases_quota_and_rejects_overflow`：持有快照时准入占用上升、释放后归还到基线；
    //    超预算输入以 `ResourceBudgetExceeded` 拒绝；失败与重试都不得泄漏配额。
    // 2. `budget_invariants_hold_at_any_time`：任意时刻 `reserved <= peak <= limit`，且 `limit` 为既定的 512 MiB。
    //
    // ## 断言依据
    // `AGENTS.md`「资源、任务、连接和缓冲区必须有上限」，以及 `Documentation/规则组合与资源验收.md`
    // 登记的既有验收口径：512 MiB 统一准入池、失败后必须归还、压力结束池占用归零。
    // 预期不取自当前实现的输出。
    //
    // ## 隔离边界
    // 只使用公开 API；未修改任何业务代码，也未扩大 `ByteBudget` 的可见性。
    // 本文件只有一处用例会申请配额，其余用例只读取计数，因此默认并行执行亦成立；
    // 若后续新增会申请配额的用例，必须改为串行（`--test-threads=1`）后重新验证。
    //
    // ## 执行状态（2026-10-15）
    // 2 项已实现、含有效断言且实际执行：**2 通过 / 0 失败**。
    // 已确认：快照释放后准入占用精确回到基线（用 `assert_eq` 而非区间判断），拒绝与重试均不泄漏配额。

    use libfluxa_core::error::Error;
    use libfluxa_core::routing::{
        Config, Match, Policy, Rule,
        runtime::{Layer, Snapshot, Source, memory_usage},
    };

    /// 与 `src/routing/runtime.rs` 的 `MAX_MEMORY` 一致：全局准入池上限。
    const LIMIT_BYTES: usize = 512 * 1024 * 1024;

    fn rule(kind: Match, value: &str, policy: Policy) -> Rule {
        Rule {
            protocol: None,
            r#type: kind,
            value: value.into(),
            policy,
            outbound: None,
        }
    }

    fn source(rules: Vec<Rule>) -> Source {
        Source {
            id: "local".into(),
            layer: Layer::Local,
            enabled: true,
            rules,
        }
    }

    fn settings() -> Config {
        Config {
            fallback: Policy::Direct,
            ..Config::default()
        }
    }

    /// 稳定占用一批准入额度的普通规则。
    fn ordinary(count: usize) -> Vec<Rule> {
        (0..count)
            .map(|index| {
                rule(
                    Match::Domain,
                    &format!("h{index}.budget.test"),
                    Policy::Direct,
                )
            })
            .collect()
    }

    #[test]
    fn budget_lifecycle_releases_quota_and_rejects_overflow() {
        let baseline = memory_usage().reserved_bytes;

        // 持有快照时准入占用必须上升。
        let held = Snapshot::generate(settings(), vec![source(ordinary(4_000))], 1).unwrap();
        let with_snapshot = memory_usage().reserved_bytes;
        assert!(
            with_snapshot > baseline,
            "生成快照后必须计入准入占用：{with_snapshot} 未超过基线 {baseline}"
        );

        // 释放后必须回落到基线：租约随所有权归还，不依赖手工回滚。
        drop(held);
        assert_eq!(
            memory_usage().reserved_bytes,
            baseline,
            "快照释放后准入占用必须归还到基线"
        );

        // 超预算输入必须以资源错误拒绝，不能静默成功。
        let dense: Vec<Rule> = (0..20_000)
            .map(|_| {
                rule(
                    Match::DomainSuffix,
                    &("a.".repeat(125) + "com"),
                    Policy::Direct,
                )
            })
            .collect();
        assert!(
            matches!(
                Snapshot::generate(settings(), vec![source(dense)], 1),
                Err(Error::ResourceBudgetExceeded { .. })
            ),
            "超出准入池的输入必须以资源错误拒绝"
        );

        // 被拒绝后不得泄漏配额：仍可正常准入，且再次释放后回到基线。
        let retry = Snapshot::generate(settings(), vec![source(ordinary(4_000))], 2).unwrap();
        assert!(
            memory_usage().reserved_bytes > baseline,
            "被拒绝之后仍必须能正常准入"
        );
        drop(retry);
        assert_eq!(
            memory_usage().reserved_bytes,
            baseline,
            "拒绝与重试都不得泄漏配额"
        );

        let usage = memory_usage();
        assert!(
            usage.peak_reserved_bytes <= usage.limit_bytes,
            "历史峰值 {} 不得超过准入上限 {}",
            usage.peak_reserved_bytes,
            usage.limit_bytes
        );
    }

    #[test]
    fn budget_invariants_hold_at_any_time() {
        let usage = memory_usage();
        assert_eq!(
            usage.limit_bytes, LIMIT_BYTES,
            "准入上限必须是既定的 512 MiB"
        );
        assert!(
            usage.reserved_bytes <= usage.peak_reserved_bytes,
            "当前占用 {} 不得超过历史峰值 {}",
            usage.reserved_bytes,
            usage.peak_reserved_bytes
        );
        assert!(
            usage.peak_reserved_bytes <= usage.limit_bytes,
            "历史峰值 {} 不得超过准入上限 {}",
            usage.peak_reserved_bytes,
            usage.limit_bytes
        );
    }
}
