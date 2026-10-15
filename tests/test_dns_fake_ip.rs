//! 测试文件：tests/test_dns_fake_ip.rs
//! 测试类型：单元测试、迁移回归
//! 测试对象：src/dns/fake_ip.rs
//! 测试内容：
//! - retired_pool_can_expand_without_reusing_either_address_family：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - either_family_history_prevents_reinterpretation_after_reconfiguration：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - unused_retired_namespaces_do_not_consume_lifetime_quota：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - shared_pool_remains_active_until_last_owner_and_allocations_are_unique：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - active_limit_is_reclaimed_and_concurrent_shared_allocations_are_unique：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - fragmented_history_limit_is_atomic_and_adjacent_history_merges：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - range_must_stay_inside_reserved_blocks：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - capacity_and_exclude_are_bounded：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - allocation_is_stable_reversible_and_does_not_rewind：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - excluded_literals_and_root_are_not_mapped：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - capacity_limit_is_enforced：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//!
//! 测试依据：保留原测试的业务/格式/授权/资源约束；《测试文件与报告统一管理原则》，独立语义审查不以实现输出反推预期。
//!
//! 前置条件：本地测试需文件/回环套接字权限；平台限定及忽略项以实际登记为准。
//!
//! 执行状态：由同名 Markdown 报告记录，不在源码中固化通过结论。

#[cfg(test)]
mod tests {
    use super::super::*;
    fn retire(owner: &Authority) {
        owner.revoke().unwrap();
        owner.complete(Ok(())).unwrap();
    }
    fn bind(pools: &mut Pools, ranges: &Ranges) -> Arc<Authority> {
        let owner = Authority::new().unwrap();
        pools.namespace(ranges, &owner).unwrap()
    }
    fn config(v4: &str, v6: &str) -> Ranges {
        Config {
            ipv4_range: v4.into(),
            ipv6_range: v6.into(),
            capacity: 2,
            exclude: vec![],
        }
        .ranges()
        .unwrap()
    }
    #[test]
    fn retired_pool_can_expand_without_reusing_either_address_family() {
        let old = config("198.18.0.0/16", "fd00:1::/64");
        let new = config("198.18.0.0/15", "fd00:1::/64");
        let mut pools = Pools::default();
        let owner = bind(&mut pools, &old);
        assert_eq!(pools.allocate(&old, 1, true).unwrap(), 1);
        assert!(matches!(
            pools.namespace(&new, &Authority::new().unwrap()),
            Err(Error::InvalidState)
        ));
        retire(&owner);
        drop(owner);
        let _new_owner = bind(&mut pools, &new);
        assert_eq!(pools.allocate(&new, 1, true).unwrap(), 2);
        assert_eq!(pools.active.len(), 1);
    }
    #[test]
    fn either_family_history_prevents_reinterpretation_after_reconfiguration() {
        let old = config("198.18.0.0/16", "fd00:2::/64");
        let changed_v4 = config("198.19.0.0/16", "fd00:2::/64");
        let changed_v6 = config("198.18.0.0/16", "fd00:3::/64");
        let mut pools = Pools::default();
        pools.allocate(&old, 1, true).unwrap();
        assert_eq!(pools.allocate(&changed_v4, 1, true).unwrap(), 2);
        assert_eq!(pools.allocate(&changed_v6, 1, true).unwrap(), 2);
    }
    #[test]
    fn unused_retired_namespaces_do_not_consume_lifetime_quota() {
        let mut pools = Pools::default();
        for index in 0..40 {
            let ranges = config("198.18.0.0/16", &format!("fd00:{index:x}::/64"));
            let owner = bind(&mut pools, &ranges);
            retire(&owner);
            drop(owner);
        }
        pools.prune().unwrap();
        assert!(pools.active.is_empty());
        assert!(pools.ipv6.is_empty());
        assert!(pools.history_lease.is_none());
    }
    #[test]
    fn shared_pool_remains_active_until_last_owner_and_allocations_are_unique() {
        let ranges = config("198.18.0.0/16", "fd00:4::/64");
        let larger = config("198.18.0.0/15", "fd00:4::/64");
        let mut pools = Pools::default();
        let first = bind(&mut pools, &ranges);
        let second = bind(&mut pools, &ranges);
        assert_ne!(first.identity(), second.identity());
        retire(&first);
        drop(first);
        assert!(matches!(
            pools.namespace(&larger, &Authority::new().unwrap()),
            Err(Error::InvalidState)
        ));
        assert_eq!(pools.allocate(&ranges, 1, true).unwrap(), 1);
        assert_eq!(pools.allocate(&ranges, 1, true).unwrap(), 2);
        retire(&second);
        drop(second);
        assert!(pools.namespace(&larger, &Authority::new().unwrap()).is_ok());
    }
    #[test]
    fn active_limit_is_reclaimed_and_concurrent_shared_allocations_are_unique() {
        let mut pools = Pools::default();
        let mut owners = Vec::new();
        for index in 0..MAX_ACTIVE_POOLS {
            let ranges = config(
                &format!("198.18.0.{}/30", index * 4),
                &format!("fd00:{index:x}::/64"),
            );
            owners.push(bind(&mut pools, &ranges));
        }
        let extra = config("198.18.0.64/30", "fd00:100::/64");
        assert!(matches!(
            pools.namespace(&extra, &Authority::new().unwrap()),
            Err(Error::ResourceLimit)
        ));
        let retiring = owners.pop().unwrap();
        retire(&retiring);
        assert!(pools.namespace(&extra, &Authority::new().unwrap()).is_ok());
        for owner in &owners {
            retire(owner);
        }
        drop(owners);
        let pools = Arc::new(Mutex::new(Pools::default()));
        let ranges = config("198.18.0.0/16", "fd00:6::/64");
        let threads: Vec<_> = (0..8)
            .map(|_| {
                let pools = pools.clone();
                let ranges = ranges.clone();
                std::thread::spawn(move || {
                    let own = Authority::new().unwrap();
                    let owner = pools.lock().unwrap().namespace(&ranges, &own).unwrap();
                    let id = pools.lock().unwrap().allocate(&ranges, 1, true).unwrap();
                    retire(&owner);
                    drop(owner);
                    id
                })
            })
            .collect();
        let mut ids: Vec<_> = threads
            .into_iter()
            .map(|thread| thread.join().unwrap())
            .collect();
        ids.sort_unstable();
        assert_eq!(ids, (1..=8).collect::<Vec<_>>());
        let mut pools = pools.lock().unwrap();
        pools.prune().unwrap();
        assert!(pools.active.is_empty());
    }
    #[test]
    fn fragmented_history_limit_is_atomic_and_adjacent_history_merges() {
        let ranges = config("198.18.0.0/16", "fd00:5::/64");
        let mut pools = Pools::default();
        for index in 0..MAX_IPV6_INTERVALS {
            pools.remember(&ranges, (index * 2 + 1) as u32).unwrap();
        }
        let rejected = (MAX_IPV6_INTERVALS * 2 + 1) as u32;
        assert!(matches!(
            pools.remember(&ranges, rejected),
            Err(Error::ResourceLimit)
        ));
        assert!(!pools.used(&ranges, rejected));
        pools.remember(&ranges, 2).unwrap();
        assert_eq!(pools.ipv6.len(), MAX_IPV6_INTERVALS - 1);
        pools.remember(&ranges, rejected).unwrap();
    }
}
// 以下保留迁移前的用例说明；历史执行状态不代表当前版本，当前结果仅见同名报告。
mod public_contract {
    use crate as libfluxa_core;
    // # Fake-IP 映射表测试
    //
    // ## 测试类型
    // 集成测试；安全测试（地址段强制与退役地址不复用）；回归测试（分配、反查与重置的既定行为）。
    //
    // ## 测试对象
    // 生产代码路径：`src/dns/fake_ip.rs`（`Config::validate` / `Config::ranges` 的地址段与容量校验、
    // `Table::new`、`Table::allocate`、`Table::contains`、`Table::reverse`、`Table::reset`、`Table::generation`）。
    //
    // 公开接口：`libfluxa_core::dns::fake_ip::{Config, Table}`（`src/dns/mod.rs:4` 的 `pub mod fake_ip;`）。
    // `Table::bind_tun` / `authority` / `ensure_active` 为 `pub(crate)`，属于内核会话绑定，按第八条
    // 未扩大其可见性，本文件不覆盖；`Pools` 亦为私有，其不变量由 `src/dns/fake_ip.rs:397-545` 的
    // 内联测试覆盖（见报告「覆盖边界」）。
    //
    // ## 测试内容
    // 1. `range_must_stay_inside_reserved_blocks`：IPv4 必须落在 `198.18.0.0/15`、IPv6 必须落在 `fd00::/8`，
    //    前缀长度区间与网络对齐都必须校验。
    // 2. `capacity_and_exclude_are_bounded`：容量 1–4096、不得超过可用地址数；排除项上限 128 且不得为根。
    // 3. `allocation_is_stable_reversible_and_does_not_rewind`：同名重复分配返回同一地址且标记为缓存命中；
    //    地址可反查；`reset` 清空映射但不回退游标，旧地址不再分配给其他域名。
    // 4. `excluded_literals_and_root_are_not_mapped`：排除项按标签边界匹配；字面 IP 与根域名不分配。
    // 5. `capacity_limit_is_enforced`：达到容量后继续分配返回 `ResourceLimit`。
    //
    // ## 断言依据
    // `src/dns/fake_ip.rs` 的模块契约「有界 Fake-IP 映射：分配后不自动复用地址，防止旧连接误指向新的域名」，
    // 以及 `Documentation/DNS.md:70`（服务复用客户端的 Fake-IP）与 `Documentation/TUN.md` 对虚拟 DNS 端点的约定。
    // 地址段 `198.18.0.0/15`、`fd00::/8` 与容量区间取自公开配置契约，不取自实现输出。
    //
    // ## 隔离边界
    // 只使用公开 API；未修改任何业务代码，也未扩大 `Table` 的私有成员可见性。
    // `Table::allocate` 会占用模块级 16 MiB 预算与进程级历史记录，因此本文件必须串行执行：
    // `cargo test --locked --test test_dns_fake_ip -- --test-threads=1`。
    //
    // ## 执行状态
    // 5 项已实现、含有效断言且实际执行，结果见同名报告 `tests/test_dns_fake_ip.md`。

    use libfluxa_core::dns::fake_ip::{Config, Table};
    use libfluxa_core::error::Error;
    use std::net::IpAddr;

    fn config(ipv4_range: &str, ipv6_range: &str, capacity: usize, exclude: &[&str]) -> Config {
        Config {
            ipv4_range: ipv4_range.into(),
            ipv6_range: ipv6_range.into(),
            capacity,
            exclude: exclude.iter().map(|value| (*value).into()).collect(),
        }
    }

    fn accepted(config: &Config) {
        let result = config.validate();
        assert!(result.is_ok(), "应当被接受，实际：{result:?}");
    }

    fn rejected(config: &Config) {
        let result = config.validate();
        assert!(
            matches!(result, Err(Error::InvalidInput)),
            "必须返回 InvalidInput，实际：{result:?}"
        );
    }

    #[test]
    fn range_must_stay_inside_reserved_blocks() {
        // 基准测试网段：198.18.0.0/15 与 fd00::/8。
        accepted(&config("198.18.0.0/16", "fd00:1::/64", 16, &[]));
        accepted(&config("198.19.0.0/16", "fd00:1::/64", 16, &[]));
        accepted(&config("198.18.0.0/15", "fd00::/64", 16, &[]));
        // 前缀长度区间：IPv4 15–30、IPv6 64–120。
        accepted(&config("198.18.0.0/30", "fd00::/120", 2, &[]));
        rejected(&config("198.18.0.0/14", "fd00:1::/64", 16, &[]));
        rejected(&config("198.18.0.0/31", "fd00:1::/64", 16, &[]));
        rejected(&config("198.18.0.0/16", "fd00::/63", 16, &[]));
        rejected(&config("198.18.0.0/16", "fd00::/121", 16, &[]));
        // 落在保留段之外。
        rejected(&config("10.0.0.0/16", "fd00:1::/64", 16, &[]));
        rejected(&config("198.20.0.0/16", "fd00:1::/64", 16, &[]));
        rejected(&config("198.18.0.0/16", "2001:db8::/64", 16, &[]));
        rejected(&config("198.18.0.0/16", "fe80::/64", 16, &[]));
        // 未按前缀对齐。
        rejected(&config("198.18.0.1/16", "fd00:1::/64", 16, &[]));
        rejected(&config("198.18.0.0/16", "fd00:1::1/64", 16, &[]));
        // 地址族写反。
        rejected(&config("fd00:1::/64", "198.18.0.0/16", 16, &[]));
        // 缺失前缀。
        rejected(&config("198.18.0.0", "fd00:1::/64", 16, &[]));
    }

    #[test]
    fn capacity_and_exclude_are_bounded() {
        accepted(&config("198.18.0.0/16", "fd00:1::/64", 1, &[]));
        accepted(&config("198.18.0.0/16", "fd00:1::/64", 4_096, &[]));
        rejected(&config("198.18.0.0/16", "fd00:1::/64", 0, &[]));
        rejected(&config("198.18.0.0/16", "fd00:1::/64", 4_097, &[]));
        // 容量不得超过可用地址数：/30 只有 2 个可用地址。
        rejected(&config("198.18.0.0/30", "fd00::/120", 3, &[]));
        accepted(&config("198.18.0.0/30", "fd00::/120", 2, &[]));
        // 排除项上限 128，且根域名不是合法排除项。
        let many: Vec<String> = (0..129).map(|index| format!("e{index}.example")).collect();
        let references: Vec<&str> = many.iter().map(String::as_str).collect();
        rejected(&config("198.18.0.0/16", "fd00:1::/64", 16, &references));
        rejected(&config("198.18.0.0/16", "fd00:1::/64", 16, &["."]));
        accepted(&config(
            "198.18.0.0/16",
            "fd00:1::/64",
            16,
            &["ads.example"],
        ));
    }

    #[test]
    fn allocation_is_stable_reversible_and_does_not_rewind() {
        let table = Table::new(config("198.18.0.0/16", "fd00:1::/64", 16, &[])).unwrap();
        assert_ne!(table.generation(), 0, "代次标识不得为零");

        let (first, cached) = table.allocate("a.example", false).unwrap().unwrap();
        assert!(!cached, "首次分配不得标记为缓存命中");
        let address: IpAddr = first.parse().unwrap();
        assert!(
            table.contains(address),
            "分配出的地址必须落在配置的地址段内"
        );

        // 同名重复分配必须返回同一地址，并标记为缓存命中。
        let (again, cached) = table.allocate("a.example", false).unwrap().unwrap();
        assert_eq!(again, first, "同名重复分配必须返回同一地址");
        assert!(cached, "重复分配必须标记为缓存命中");

        // 不同域名必须得到不同地址。
        let (second, cached) = table.allocate("b.example", false).unwrap().unwrap();
        assert_ne!(second, first);
        assert!(!cached);

        // 反查必须还原域名。
        assert_eq!(table.reverse(&first).unwrap().as_deref(), Some("a.example"));
        assert_eq!(
            table.reverse(&second).unwrap().as_deref(),
            Some("b.example")
        );

        // IPv6 分配同样落在配置段内。
        let (v6, _) = table.allocate("c.example", true).unwrap().unwrap();
        let v6: IpAddr = v6.parse().unwrap();
        assert!(table.contains(v6), "IPv6 分配必须落在配置的地址段内");

        // reset 只清空映射，不回退游标：旧地址不得再分配给其他域名。
        table.reset().unwrap();
        assert_eq!(
            table.reverse(&first).unwrap(),
            None,
            "reset 后不得再反查到旧映射"
        );
        let (after_reset, cached) = table.allocate("d.example", false).unwrap().unwrap();
        assert!(!cached);
        assert_ne!(after_reset, first, "reset 不得回退游标并复用旧地址");
        assert_ne!(after_reset, second);
        assert_eq!(
            table.reverse(&after_reset).unwrap().as_deref(),
            Some("d.example")
        );
    }

    #[test]
    fn excluded_literals_and_root_are_not_mapped() {
        let table =
            Table::new(config("198.18.0.0/16", "fd00:1::/64", 16, &["ads.example"])).unwrap();
        // 排除项本身与其子域都不分配（None 表示调用者应使用真实解析）。
        assert_eq!(table.allocate("ads.example", false).unwrap(), None);
        assert_eq!(table.allocate("sub.ads.example", false).unwrap(), None);
        assert_eq!(table.allocate("ADS.EXAMPLE", false).unwrap(), None);
        // 按标签边界匹配：仅后缀相同不算命中。
        assert!(table.allocate("notads.example", false).unwrap().is_some());
        // 字面 IP 与根域名不分配。
        assert_eq!(table.allocate("192.0.2.1", false).unwrap(), None);
        assert_eq!(table.allocate(".", false).unwrap(), None);
        // 普通域名仍然分配。
        assert!(table.allocate("plain.example", false).unwrap().is_some());
    }

    #[test]
    fn capacity_limit_is_enforced() {
        let table = Table::new(config("198.18.0.0/16", "fd00:1::/64", 2, &[])).unwrap();
        assert!(table.allocate("one.example", false).unwrap().is_some());
        assert!(table.allocate("two.example", false).unwrap().is_some());
        assert!(
            matches!(
                table.allocate("three.example", false),
                Err(Error::ResourceLimit)
            ),
            "达到容量后必须返回 ResourceLimit"
        );
        // 已存在的映射不受容量影响。
        assert!(table.allocate("one.example", false).unwrap().is_some());
    }
}
