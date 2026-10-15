//! 测试文件：tests/test_routing_flat.rs
//! 测试类型：单元测试、迁移回归
//! 测试对象：src/routing/flat.rs
//! 测试内容：
//! - scenario_01_sixty_five_domains_in_one_slot：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - scenario_02_ipv4_zero_prefix_must_be_normalized：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - scenario_03_ipv6_zero_prefix_must_be_normalized：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - scenario_04_child_prefix_must_belong_to_parent：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - scenario_05_child_must_be_on_the_right_branch：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - scenario_06_keyword_failures_return_every_candidate：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - scenario_07_tampered_keyword_failure_transition_is_rejected：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - scenario_08_candidate_must_correspond_to_its_node：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - scenario_09_same_class_candidate_order_is_preserved：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - scenario_10_domain_and_ip_priority：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - scenario_11_build_peak_stays_within_budget：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - scenario_12_retained_snapshot_is_still_counted：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - scenario_13_incompatible_format_version_is_reported：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - scenario_14_domain_normalization_never_misses：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - control_rewrite_with_recomputed_digest_still_loads：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - normalized_child_outside_parent_is_rejected_for_both_families：以独立构造的非法缓存及合法边界验证拒绝和匹配。
//! - prefix_address_bits_and_leaf_boundaries_are_validated：以独立构造的非法缓存及合法边界验证拒绝和匹配。
//! - previous_cache_layout_requires_explicit_rebuild：旧格式返回明确错误且不覆盖文件。
//! - collision_fallback_preserves_other_sections：碰撞后其他索引区段的字符串和来源不受影响。
//!
//! 测试依据：保留原测试的业务/格式/授权/资源约束；《测试文件与报告统一管理原则》，独立语义审查不以实现输出反推预期。
//!
//! 前置条件：本地测试需文件/回环套接字权限；平台限定及忽略项以实际登记为准。
//!
//! 执行状态：由同名 Markdown 报告记录，不在源码中固化通过结论。

// 以下保留迁移前的用例说明；历史执行状态不代表当前版本，当前结果仅见同名报告。
mod public_contract {
    use crate as libfluxa_core;
    // # 平铺索引与规则缓存测试
    //
    // ## 测试类型
    // 集成测试；安全测试（非法索引必须被拒绝）；回归测试（规则优先级、域名规范化与资源预算的既定行为）。
    //
    // ## 测试对象
    // 生产代码路径：
    // - `src/routing/flat.rs`：平铺索引的解析、语义校验（`FlatIndex::validate`）与查询路径。
    // - `src/routing/cache.rs`：缓存文件格式、版本字段与 SHA-256 校验。
    // - `src/routing/store.rs`：`Store::open` / `update` / `snapshot` / `close`。
    // - `src/routing/runtime.rs`：`Snapshot::generate` / `Snapshot::evaluate` / `memory_usage`。
    //
    // 公开接口：`libfluxa_core::routing::{Config, Match, Policy, Rule, Target, Transport}`、
    // `libfluxa_core::routing::runtime::{Layer, Snapshot, Source, memory_usage}`、
    // `libfluxa_core::routing::store::Store`、`libfluxa_core::error::Error`。
    //
    // ## 测试内容
    // 场景用例（函数名即断言目标）：
    // 1. `scenario_01_sixty_five_domains_in_one_slot`：65 个哈希同槽合法域名不得被报为资源耗尽；含 64 个的边界对照。
    // 2. `scenario_02_ipv4_zero_prefix_must_be_normalized`：IPv4 非规范化 `/0` 前缀必须被拒绝。
    // 3. `scenario_03_ipv6_zero_prefix_must_be_normalized`：IPv6 非规范化 `/0` 前缀必须被拒绝。
    // 4. `scenario_04_child_prefix_must_belong_to_parent`：子节点前缀不属于父节点时必须被拒绝。
    // 5. `scenario_05_child_must_be_on_the_right_branch`：子节点挂载到错误分支时必须被拒绝。
    // 6. `scenario_06_keyword_failures_return_every_candidate`：关键词 `ab`/`bc` 匹配 `abc` 时必须返回全部候选。
    // 7. `scenario_07_tampered_keyword_failure_transition_is_rejected`：篡改关键词失败转移必须被拒绝。
    // 8. `scenario_08_candidate_must_correspond_to_its_node`：候选编号与节点匹配条件不对应时必须被拒绝。
    // 9. `scenario_09_same_class_candidate_order_is_preserved`：同类别候选顺序被篡改不得改变既定规则顺序。
    // 10. `scenario_10_domain_and_ip_priority`：域名与 IP 同时命中时必须遵守正式规则优先级。
    // 11. `scenario_11_build_peak_stays_within_budget`：构建内存峰值不得越过准入预算。
    // 12. `scenario_12_retained_snapshot_is_still_counted`：旧索引快照仍被引用时不得漏计内存。
    // 13. `scenario_13_incompatible_format_version_is_reported`：不兼容的文件格式版本必须返回明确错误。
    // 14. `scenario_14_domain_normalization_never_misses`：域名规范化差异不得造成漏匹配。
    //
    // 自检用例：
    // 15. `control_rewrite_with_recomputed_digest_still_loads`：原样重写并重算摘要后必须仍能加载；摘要错误必须被拒绝。
    //
    // ## 断言依据
    // 14 条场景用例的预期结果取自用户给出的「测试场景 / 预期结果」表，属独立业务规范；
    // 预期不由当前实现输出反向推导。用例失败即表示实现未满足该规范，不得通过放宽断言使其通过。
    // 场景措辞存在多种解读时，本文件采用的解读写在相应用例自身的文档注释里（见用例 09）。
    //
    // ## 隔离边界
    // 只使用公开 API 与缓存文件字节；未为测试扩大任何生产 API 可见性，也未向外部暴露内核内部资源管理能力。
    // `FlatIndex`、`Snapshot::into_flat` 等为 crate 私有，因此篡改用例经缓存文件触达 `FlatIndex::validate`：
    // `Store::update` 写出 → 就地改写索引区 → 重算 SHA-256 → `Store::open` 重载，全程不修改产品代码。
    // 每个篡改用例先做「空改写仍能加载」自检，另有用例独立证明「摘要错误必被拒绝」，
    // 避免把校验和不匹配误判成语义校验生效。
    //
    // ## 运行要求
    // 用例 11、12 依赖进程级 512 MiB 准入池，必须串行执行：
    // `python3 Tools/test_reports.py --run --suite test_routing_flat`。
    // 默认并行时其他用例占用同一内存池，`cache::load` 会返回 `ResourceBudgetExceeded` 而污染结论。
    //
    // 原六项失败的历史证据保留在 Git 历史和报告中；本次结果仅见同名报告。

    use libfluxa_core::error::Error;
    use libfluxa_core::routing::{
        Config, Match, Policy, Rule, Target, Transport,
        runtime::{Layer, Snapshot, Source, memory_usage},
        store::Store,
    };
    use std::collections::HashMap;
    use std::path::{Path, PathBuf};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU64, Ordering};

    // ---- 缓存文件格式（对应 src/routing/cache.rs 的常量）----
    const CACHE_FILE: &str = "runtime-rules.bin";
    /// 本次升级前的已知旧格式，用于明确迁移拒绝而非猜测未知版本。
    const PREVIOUS_CACHE_VERSION: u32 = 2;
    const DOCUMENT_LENGTH_OFFSET: usize = 12;
    const DIGEST_OFFSET: usize = 24;
    const DIGEST_BYTES: usize = 32;
    const CACHE_HEADER_BYTES: usize = DIGEST_OFFSET + DIGEST_BYTES; // 56

    // ---- 平铺索引格式（对应 src/routing/flat.rs 的常量）----
    const SECTION_V4: usize = 3;
    const SECTION_V6: usize = 4;
    const SECTION_KEYWORD_NODES: usize = 6;
    /// 精确域名行：字符串偏移(4) + 长度(4) + 21 个候选槽(84)。
    const EXACT_BYTES: usize = 2 * 4 + CANDIDATE_BYTES;
    /// 前缀行：key(16) + length(4) + 左右子(8) + 21 个候选槽(84)。
    const PREFIX_BYTES: usize = 16 + 4 + 2 * 4 + CANDIDATE_BYTES;
    const PREFIX_LENGTH_OFFSET: usize = 16;
    const PREFIX_CHILDREN_OFFSET: usize = 20;
    /// 关键词节点：start(4) + count(4) + fail(4) + depth(4) + 21 个候选槽(84)。
    const KEYWORD_NODE_BYTES: usize = 4 * 4 + CANDIDATE_BYTES;
    const KEYWORD_FAIL_OFFSET: usize = 2 * 4;
    const CANDIDATE_BYTES: usize = 21 * 4;
    const NONE: u32 = u32::MAX;
    /// 候选槽总数（`RuleClass::COUNT`），仅用于按槽遍历。
    const CLASSES: usize = 21;

    // ---------------------------------------------------------------- 基础构造

    fn rule(kind: Match, value: &str, policy: Policy) -> Rule {
        Rule {
            protocol: None,
            r#type: kind,
            value: value.into(),
            policy,
            outbound: None,
        }
    }

    fn source(id: &str, rules: Vec<Rule>) -> Source {
        Source {
            id: id.into(),
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

    fn target(domain: Option<&str>, address: Option<&str>) -> Target {
        Target {
            domain: domain.map(Into::into),
            address: address.map(Into::into),
            port: 443,
            transport: Transport::Tcp,
        }
    }

    /// 临时目录独占所有权；成功或断言失败时都清理，清理失败不冒充测试成功。
    struct TestDirectory(PathBuf);
    impl std::ops::Deref for TestDirectory {
        type Target = Path;
        fn deref(&self) -> &Path {
            &self.0
        }
    }
    impl AsRef<Path> for TestDirectory {
        fn as_ref(&self) -> &Path {
            &self.0
        }
    }
    impl Drop for TestDirectory {
        fn drop(&mut self) {
            if let Err(error) = std::fs::remove_dir_all(&self.0)
                && error.kind() != std::io::ErrorKind::NotFound
            {
                if std::thread::panicking() {
                    eprintln!("临时规则目录清理失败：{error}");
                } else {
                    panic!("临时规则目录清理失败：{error}");
                }
            }
        }
    }
    fn temp_dir(tag: &str) -> TestDirectory {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!(
            "fluxa-flat-{tag}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        TestDirectory(dir)
    }

    /// 写出缓存并返回可继续读取的目录；`Store` 关闭后跨进程锁释放，允许再次 `open`。
    fn write_cache(dir: &Path, rules: Vec<Rule>) {
        let store = Store::open(dir).unwrap();
        store
            .update(settings(), vec![source("local", rules)])
            .expect("合法规则必须能写出缓存");
        store.close().unwrap();
    }

    /// 用当前缓存重新打开一次；只关心能否加载。
    fn reload(dir: &Path) -> Result<(), Error> {
        match Store::open(dir) {
            Ok(store) => {
                let _ = store.close();
                Ok(())
            }
            Err(error) => Err(error),
        }
    }

    /// 打开缓存并取回已平铺的快照（`into_flat` 为 crate 私有，公开路径只有 `Store`）。
    fn flat_snapshot(dir: &Path, rules: Vec<Rule>) -> Arc<Snapshot> {
        let store = Store::open(dir).unwrap();
        store
            .update(settings(), vec![source("local", rules)])
            .unwrap();
        let snapshot = store.snapshot().unwrap();
        let _ = store.close();
        snapshot
    }

    // ---------------------------------------------------------------- 字节工具

    fn u32_at(bytes: &[u8], offset: usize) -> u32 {
        u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
    }

    fn put_u32(bytes: &mut [u8], offset: usize, value: u32) {
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }

    fn u128_at(bytes: &[u8], offset: usize) -> u128 {
        u128::from_le_bytes(bytes[offset..offset + 16].try_into().unwrap())
    }

    fn put_u128(bytes: &mut [u8], offset: usize, value: u128) {
        bytes[offset..offset + 16].copy_from_slice(&value.to_le_bytes());
    }

    /// 平铺索引区在缓存文件中的绝对起点。
    fn index_start(bytes: &[u8]) -> usize {
        CACHE_HEADER_BYTES + u32_at(bytes, DOCUMENT_LENGTH_OFFSET) as usize
    }

    /// 返回区段的 `(绝对起点, 记录数)`；目录里存的是相对平铺缓冲区起点的绝对偏移。
    fn section(bytes: &[u8], slot: usize) -> (usize, usize) {
        let start = index_start(bytes);
        let offset = u32_at(bytes, start + slot * 8) as usize;
        let count = u32_at(bytes, start + slot * 8 + 4) as usize;
        (start + offset, count)
    }

    fn sha256(data: &[u8]) -> [u8; DIGEST_BYTES] {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "fluxa-sha-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::write(&path, data).unwrap();
        let output = std::process::Command::new("shasum")
            .args(["-a", "256"])
            .arg(&path)
            .output()
            .expect("需要系统 shasum 计算 SHA-256；不可用时应记录为未测而不是伪造通过");
        let _ = std::fs::remove_file(&path);
        let text = String::from_utf8(output.stdout).expect("shasum 输出应为 UTF-8");
        let hex = text
            .split_whitespace()
            .next()
            .expect("shasum 应输出摘要")
            .to_owned();
        assert_eq!(hex.len(), DIGEST_BYTES * 2, "shasum 摘要长度异常：{text}");
        let mut digest = [0u8; DIGEST_BYTES];
        for (slot, byte) in digest.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&hex[slot * 2..slot * 2 + 2], 16).unwrap();
        }
        digest
    }

    /// 按 `cache::write` 的顺序把摘要写在 (文档 ‖ 索引) 之上后落盘。
    fn write_with_digest(dir: &Path, bytes: &[u8]) {
        let digest = sha256(&bytes[CACHE_HEADER_BYTES..]);
        let mut output = bytes.to_vec();
        output[DIGEST_OFFSET..DIGEST_OFFSET + DIGEST_BYTES].copy_from_slice(&digest);
        std::fs::write(dir.join(CACHE_FILE), &output).unwrap();
    }

    /// 就地改写索引区并重载，返回重载结果。
    ///
    /// 先做「空改写必须仍能加载」自检，证明摘要算法与头部偏移正确；否则后续得到的
    /// `InvalidInput` 可能只是校验和不匹配，而不是索引语义校验生效。
    fn tamper_and_reload(dir: &Path, mutate: impl FnOnce(&mut [u8])) -> Result<(), Error> {
        let original = std::fs::read(dir.join(CACHE_FILE)).unwrap();
        write_with_digest(dir, &original);
        assert!(
            reload(dir).is_ok(),
            "空改写后必须仍能加载：校验和算法或头部偏移与实现不一致"
        );
        let mut tampered = original;
        mutate(&mut tampered);
        write_with_digest(dir, &tampered);
        reload(dir)
    }

    /// 前缀区段每个节点的 `(记录起点, key, length, [左子, 右子])`。
    fn prefix_nodes(bytes: &[u8], slot: usize) -> Vec<(usize, u128, u32, [u32; 2])> {
        let (start, count) = section(bytes, slot);
        (0..count)
            .map(|index| {
                let at = start + index * PREFIX_BYTES;
                (
                    at,
                    u128_at(bytes, at),
                    u32_at(bytes, at + PREFIX_LENGTH_OFFSET),
                    [
                        u32_at(bytes, at + PREFIX_CHILDREN_OFFSET),
                        u32_at(bytes, at + PREFIX_CHILDREN_OFFSET + 4),
                    ],
                )
            })
            .collect()
    }

    fn fnv1a(value: &str) -> u64 {
        value.bytes().fold(0xcbf2_9ce4_8422_2325u64, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3)
        })
    }

    // ---------------------------------------------------------------- 场景 01

    fn collision_names() -> Vec<String> {
        let mut buckets: HashMap<usize, Vec<String>> = HashMap::new();
        let mut chosen = None;
        for index in 0..2_000_000u32 {
            let name = format!("h{index}.slot.test");
            let slot = (fnv1a(&name) as usize) & 0xff;
            let bucket = buckets.entry(slot).or_default();
            bucket.push(name);
            if bucket.len() == 65 {
                chosen = Some(std::mem::take(bucket));
                break;
            }
        }
        chosen.expect("应能构造出 65 个同槽合法域名")
    }

    /// 65 个同槽合法域名不应被错误识别为真实内存资源耗尽。
    ///
    /// 精确域名表按 `next_power_of_two(2 × 域名数)` 取槽数；65 个域名给出 256 槽。
    /// 构造 65 个在低 8 位哈希上同槽的合法域名，第 65 次插入会走到探测链上限。
    #[test]
    fn scenario_01_sixty_five_domains_in_one_slot() {
        let names = collision_names();
        assert_eq!(names.len(), 65);
        // 对照：取其中 64 个，槽数为 128，同槽 64 个正好等于探测链上限，必须成功。
        // 这一步证明「同槽构造」有效，失败只可能来自 65 越过上限。
        let dir = temp_dir("slot64");
        let store = Store::open(&dir).unwrap();
        let sixty_four: Vec<Rule> = names[..64]
            .iter()
            .map(|name| rule(Match::Domain, name, Policy::Direct))
            .collect();
        assert!(
            store
                .update(settings(), vec![source("local", sixty_four)])
                .is_ok(),
            "同槽 64 个域名应当可以构建"
        );
        let _ = store.close();

        // 同时满足另一种读法：65 个域名同属一个规则类别槽（LocalDomain）。
        let rules: Vec<Rule> = names
            .iter()
            .map(|name| rule(Match::Domain, name, Policy::Direct))
            .collect();
        let dir = temp_dir("slot65");
        let store = Store::open(&dir).unwrap();
        let result = store.update(settings(), vec![source("local", rules)]);
        // 注意错误类别：内存池不足返回的是 `ResourceBudgetExceeded`，这里拿到的是
        // `ResourceLimit`，说明失败来自精确表探测链上限而不是真实内存耗尽。
        assert!(
            !matches!(result, Err(Error::ResourceLimit)),
            "65 个同槽合法域名被误报为资源耗尽（探测链上限，非真实内存不足）"
        );
        assert!(
            result.is_ok(),
            "65 个同槽合法域名应当可以构建：{:?}",
            result.err()
        );
        let snapshot = store.snapshot().unwrap();
        for (index, name) in names.iter().enumerate() {
            let decision = snapshot.evaluate(&target(Some(name), None)).unwrap();
            assert_eq!(decision.policy, Policy::Direct);
            assert_eq!(
                decision.rule_index,
                Some(index),
                "合法碰撞不能漏匹配或错报规则来源"
            );
        }
        assert_eq!(
            snapshot
                .evaluate(&target(Some("absent.example"), None))
                .unwrap()
                .rule_index,
            None
        );
        store.close().unwrap();
        let recovered = Store::open(&dir).unwrap();
        for (index, name) in names.iter().enumerate() {
            let decision = recovered
                .snapshot()
                .unwrap()
                .evaluate(&target(Some(name), None))
                .unwrap();
            assert_eq!(
                decision.rule_index,
                Some(index),
                "缓存恢复必须保留碰撞表对应关系"
            );
        }
        recovered.close().unwrap();
    }

    // ---------------------------------------------------------------- 场景 02 / 03

    fn non_normalized_zero_prefix(slot: usize, tag: &str, rules: Vec<Rule>, expect_width: u32) {
        let dir = temp_dir(tag);
        write_cache(&dir, rules);
        let result = tamper_and_reload(&dir, |bytes| {
            let nodes = prefix_nodes(bytes, slot);
            assert!(!nodes.is_empty(), "该形态应当产生前缀节点");
            // 找到 length 为 0 的根节点：/0 的掩码为空，任何非零 key 都是非规范化前缀。
            let (at, _, length, _) = *nodes
                .iter()
                .find(|(_, _, length, _)| *length == 0)
                .expect("两个 /1 规则应当产生 /0 根节点");
            assert_eq!(length, 0);
            assert!(expect_width == 32 || expect_width == 128);
            assert_eq!(u128_at(bytes, at), 0, "根节点原本应当是规范化的全零 key");
            put_u128(bytes, at, 1);
        });
        assert!(
            matches!(result, Err(Error::InvalidInput)),
            "非规范化的 /0 前缀必须被拒绝，实际：{result:?}"
        );
    }

    #[test]
    fn scenario_02_ipv4_zero_prefix_must_be_normalized() {
        non_normalized_zero_prefix(
            SECTION_V4,
            "v4zero",
            vec![
                rule(Match::IpCidr, "0.0.0.0/1", Policy::Direct),
                rule(Match::IpCidr, "128.0.0.0/1", Policy::Direct),
            ],
            32,
        );
    }

    #[test]
    fn scenario_03_ipv6_zero_prefix_must_be_normalized() {
        non_normalized_zero_prefix(
            SECTION_V6,
            "v6zero",
            vec![
                rule(Match::IpCidr, "::/1", Policy::Direct),
                rule(Match::IpCidr, "8000::/1", Policy::Direct),
            ],
            128,
        );
    }

    // ---------------------------------------------------------------- 场景 04 / 05

    #[test]
    fn scenario_04_child_prefix_must_belong_to_parent() {
        let dir = temp_dir("childprefix");
        write_cache(
            &dir,
            vec![
                rule(Match::IpCidr, "0.0.0.0/1", Policy::Direct),
                rule(Match::IpCidr, "128.0.0.0/1", Policy::Direct),
            ],
        );
        let result = tamper_and_reload(&dir, |bytes| {
            let nodes = prefix_nodes(bytes, SECTION_V4);
            let (_, _, root_length, children) = *nodes
                .iter()
                .find(|(_, _, length, _)| *length == 0)
                .expect("应有 /0 根节点");
            let child = children[0];
            assert_ne!(child, NONE);
            let (child_at, _, child_length, _) = nodes[child as usize];
            assert!(child_length > root_length);
            // 把子节点的 key 换成与父节点完全无关的值：长度关系不变，父子前缀关系被破坏。
            assert_ne!(u128_at(bytes, child_at), u128::MAX >> 1, "改动必须真的生效");
            put_u128(bytes, child_at, u128::MAX >> 1);
        });
        assert!(
            matches!(result, Err(Error::InvalidInput)),
            "子节点前缀不属于父节点时必须被拒绝，实际：{result:?}"
        );
    }

    #[test]
    fn scenario_05_child_must_be_on_the_right_branch() {
        let dir = temp_dir("childbranch");
        write_cache(
            &dir,
            vec![
                rule(Match::IpCidr, "0.0.0.0/1", Policy::Direct),
                rule(Match::IpCidr, "128.0.0.0/1", Policy::Direct),
            ],
        );
        let result = tamper_and_reload(&dir, |bytes| {
            let nodes = prefix_nodes(bytes, SECTION_V4);
            let (root, _, _, children) = *nodes
                .iter()
                .find(|(_, _, length, _)| *length == 0)
                .expect("应有 /0 根节点");
            assert_ne!(children[0], NONE);
            assert_ne!(children[1], NONE);
            assert_ne!(children[0], children[1], "两个子节点必须不同，交换才有意义");
            // 交换左右子：两个子节点都挂到了错误分支，但长度关系与候选仍然合法。
            put_u32(bytes, root + PREFIX_CHILDREN_OFFSET, children[1]);
            put_u32(bytes, root + PREFIX_CHILDREN_OFFSET + 4, children[0]);
        });
        assert!(
            matches!(result, Err(Error::InvalidInput)),
            "子节点挂载到错误分支时必须被拒绝，实际：{result:?}"
        );
    }

    /// 父节点不是 /0，子地址本身规范合法，单独验证父子包含关系。
    #[test]
    fn normalized_child_outside_parent_is_rejected_for_both_families() {
        for (section_id, networks, replacement) in [
            (
                SECTION_V4,
                ["10.0.0.0/9", "10.128.0.0/9", "11.0.0.0/9", "11.128.0.0/9"],
                u128::from(0x0c00_0000u32),
            ),
            (
                SECTION_V6,
                [
                    "2001:db8::/33",
                    "2001:db8:8000::/33",
                    "2001:db9::/33",
                    "2001:db9:8000::/33",
                ],
                u128::from("2001:dba::".parse::<std::net::Ipv6Addr>().unwrap()),
            ),
        ] {
            let dir = temp_dir("parentcontainment");
            write_cache(
                &dir,
                networks
                    .into_iter()
                    .map(|cidr| rule(Match::IpCidr, cidr, Policy::Direct))
                    .collect(),
            );
            let result = tamper_and_reload(&dir, |bytes| {
                let nodes = prefix_nodes(bytes, section_id);
                let (_, _, _, children) = nodes
                    .iter()
                    .filter(|(_, _, _, children)| children.iter().all(|id| *id != NONE))
                    .min_by_key(|(_, _, length, _)| *length)
                    .unwrap();
                let (at, _, _, grandchildren) = nodes[children[0] as usize];
                assert!(grandchildren.iter().all(|id| *id != NONE));
                assert!(
                    (0..CLASSES).all(|slot| u32_at(
                        bytes,
                        at + PREFIX_CHILDREN_OFFSET + 2 * 4 + slot * 4
                    ) == NONE),
                    "必须独立测试无候选中间节点"
                );
                put_u128(bytes, at, replacement);
            });
            assert!(
                matches!(result, Err(Error::InvalidInput)),
                "规范合法的子网也必须属于父网段：{result:?}"
            );
        }
    }

    /// 直接覆盖不同于父子关系的地址主机位、IPv4 位宽及最长合法前缀边界。
    #[test]
    fn prefix_address_bits_and_leaf_boundaries_are_validated() {
        for (section_id, cidr, replacement) in [
            (SECTION_V4, "10.0.0.0/8", u128::from(0x0a00_0001u32)),
            (
                SECTION_V4,
                "10.0.0.0/8",
                (1u128 << 32) | u128::from(0x0a00_0000u32),
            ),
            (
                SECTION_V6,
                "2001:db8::/32",
                u128::from("2001:db8::1".parse::<std::net::Ipv6Addr>().unwrap()),
            ),
        ] {
            let dir = temp_dir("addressbits");
            write_cache(&dir, vec![rule(Match::IpCidr, cidr, Policy::Reject)]);
            let result = tamper_and_reload(&dir, |bytes| {
                let at = prefix_nodes(bytes, section_id)[0].0;
                put_u128(bytes, at, replacement);
            });
            assert!(matches!(result, Err(Error::InvalidInput)));
        }
        for (cidr, address) in [
            ("192.0.2.1/32", "192.0.2.1"),
            ("2001:db8::1/128", "2001:db8::1"),
        ] {
            let dir = temp_dir("leafboundary");
            let snapshot = flat_snapshot(&dir, vec![rule(Match::IpCidr, cidr, Policy::Reject)]);
            assert_eq!(
                snapshot
                    .evaluate(&target(None, Some(address)))
                    .unwrap()
                    .policy,
                Policy::Reject
            );
        }
    }

    // ---------------------------------------------------------------- 场景 06

    fn keyword_decision(rules: Vec<Rule>, domain: &str) -> libfluxa_core::routing::Decision {
        let dir = temp_dir("keyword");
        let snapshot = flat_snapshot(&dir, rules);
        snapshot
            .evaluate(&target(Some(domain), Some("192.0.2.1")))
            .unwrap()
    }

    /// 关键词 `ab`、`bc` 匹配 `abc`：AC 自动机必须通过失败转移把两个候选都返回。
    ///
    /// 两轮只在「哪个关键词是 Reject」上互换：若任一轮的 Reject 不是对应关键词，
    /// 结果就会是 Direct。两轮都得到 Reject 即证明两个关键词都参与了同一次判定。
    #[test]
    fn scenario_06_keyword_failures_return_every_candidate() {
        let ab_reject = keyword_decision(
            vec![
                rule(Match::DomainKeyword, "ab", Policy::Reject),
                rule(Match::DomainKeyword, "bc", Policy::Direct),
            ],
            "abc",
        );
        let bc_reject = keyword_decision(
            vec![
                rule(Match::DomainKeyword, "ab", Policy::Direct),
                rule(Match::DomainKeyword, "bc", Policy::Reject),
            ],
            "abc",
        );
        assert_eq!(
            ab_reject.policy,
            Policy::Reject,
            "关键词 ab 未在 abc 上返回候选"
        );
        assert_eq!(
            bc_reject.policy,
            Policy::Reject,
            "关键词 bc 未在 abc 上返回候选"
        );
        // 对照：只有一个关键词命中时不得互相牵连。
        let only_ab = keyword_decision(
            vec![
                rule(Match::DomainKeyword, "ab", Policy::Reject),
                rule(Match::DomainKeyword, "bc", Policy::Direct),
            ],
            "ab",
        );
        assert_eq!(only_ab.policy, Policy::Reject);
        let neither = keyword_decision(
            vec![
                rule(Match::DomainKeyword, "ab", Policy::Reject),
                rule(Match::DomainKeyword, "bc", Policy::Reject),
            ],
            "xyz",
        );
        assert_eq!(neither.policy, Policy::Direct, "未命中关键词时不得返回候选");
    }

    // ---------------------------------------------------------------- 场景 07

    #[test]
    fn scenario_07_tampered_keyword_failure_transition_is_rejected() {
        let dir = temp_dir("keywordfail");
        write_cache(
            &dir,
            vec![
                rule(Match::DomainKeyword, "abc", Policy::Direct),
                rule(Match::DomainKeyword, "abd", Policy::Direct),
            ],
        );
        let result = tamper_and_reload(&dir, |bytes| {
            let (start, count) = section(bytes, SECTION_KEYWORD_NODES);
            assert!(
                count >= 4,
                "两个关键词应当产生至少 4 个自动机节点，实际 {count}"
            );
            // 把某个非根节点的失败转移指向它自己：失败边必须严格缩短深度。
            let node = 2usize;
            let at = start + node * KEYWORD_NODE_BYTES;
            put_u32(bytes, at + KEYWORD_FAIL_OFFSET, node as u32);
        });
        assert!(
            matches!(result, Err(Error::InvalidInput)),
            "篡改关键词失败转移后必须被拒绝，实际：{result:?}"
        );
    }

    // ---------------------------------------------------------------- 场景 08

    #[test]
    fn scenario_08_candidate_must_correspond_to_its_node() {
        let dir = temp_dir("candidatecorr");
        write_cache(
            &dir,
            vec![
                rule(Match::Domain, "one.example", Policy::Reject),
                rule(Match::Domain, "two.example", Policy::Reject),
            ],
        );
        let result = tamper_and_reload(&dir, |bytes| {
            let (start, count) = section(bytes, 0);
            assert!(count >= 2, "应当有两条精确域名记录");
            let mut rows = Vec::new();
            for index in 0..count {
                let at = start + index * EXACT_BYTES;
                if u32_at(bytes, at + 4) != 0 {
                    rows.push(at);
                }
            }
            assert!(rows.len() >= 2, "应当有两条非空精确域名记录");
            let (first, second) = (rows[0], rows[1]);
            // 只交换候选编号：两个编号都合法、类别槽也对得上，但不再对应各自的匹配条件。
            let mut changed = 0usize;
            for slot in 0..CLASSES {
                let offset = 2 * 4 + slot * 4;
                let left = u32_at(bytes, first + offset);
                let right = u32_at(bytes, second + offset);
                if left != right {
                    changed += 1;
                }
                put_u32(bytes, first + offset, right);
                put_u32(bytes, second + offset, left);
            }
            assert!(changed > 0, "交换必须真的改变候选编号");
        });
        assert!(
            matches!(result, Err(Error::InvalidInput)),
            "候选编号与匹配条件不对应时必须被拒绝，实际：{result:?}"
        );
    }

    // ---------------------------------------------------------------- 场景 09

    /// 同类别候选顺序被篡改时不得改变既定规则顺序。
    ///
    /// 平铺格式里每个类别只有一个槽，槽内存的是该类别最早的规则编号；因此
    /// 「同类别顺序」体现为「最早规则胜出」。分两部分验证：
    /// 一是索引自身保持文档顺序，二是打乱候选槽后决策不得静默改变。
    #[test]
    fn scenario_09_same_class_candidate_order_is_preserved() {
        let dir = temp_dir("classorder");
        let snapshot = flat_snapshot(
            &dir,
            vec![
                rule(Match::Domain, "same.example", Policy::Reject),
                rule(Match::Domain, "same.example", Policy::Direct),
            ],
        );
        let decision = snapshot
            .evaluate(&target(Some("same.example"), Some("192.0.2.1")))
            .unwrap();
        assert_eq!(
            decision.policy,
            Policy::Reject,
            "同类别候选必须按文档顺序取最早规则"
        );
        assert_eq!(decision.rule_index, Some(0));

        // 打乱候选类别槽；类别编号不是优先级，非法类别必须被拒绝且不能改变决策。
        let before = decision.policy;
        let result = tamper_and_reload(&dir, |bytes| {
            let (start, count) = section(bytes, 0);
            for index in 0..count {
                let at = start + index * EXACT_BYTES;
                if u32_at(bytes, at + 4) == 0 {
                    continue;
                }
                let mut slots: Vec<u32> = (0..CLASSES)
                    .map(|s| u32_at(bytes, at + 8 + s * 4))
                    .collect();
                slots.rotate_left(1);
                for (slot, value) in slots.into_iter().enumerate() {
                    put_u32(bytes, at + 8 + slot * 4, value);
                }
            }
        });
        if result.is_ok() {
            let after = reload_decision(&dir, "same.example");
            assert_eq!(
                after, before,
                "打乱候选槽后决策发生了变化，既定规则顺序未被保护"
            );
        }
    }

    fn reload_decision(dir: &Path, domain: &str) -> Policy {
        let store = Store::open(dir).unwrap();
        let snapshot = store.snapshot().unwrap();
        let decision = snapshot
            .evaluate(&target(Some(domain), Some("192.0.2.1")))
            .unwrap();
        let _ = store.close();
        decision.policy
    }

    // ---------------------------------------------------------------- 场景 10

    #[test]
    fn scenario_10_domain_and_ip_priority() {
        let dir = temp_dir("domainip");
        let snapshot = flat_snapshot(
            &dir,
            vec![
                rule(Match::Domain, "example.com", Policy::Direct),
                rule(Match::IpCidr, "192.0.2.0/24", Policy::Reject),
                rule(Match::Domain, "blocked.example", Policy::Reject),
            ],
        );
        // 域名命中：按既有优先级跳过 IP 规则。
        let hit = snapshot
            .evaluate(&target(Some("example.com"), Some("192.0.2.1")))
            .unwrap();
        assert_eq!(hit.policy, Policy::Direct, "域名命中时必须跳过 IP 候选");
        // 域名未命中：IP 规则生效。
        let miss = snapshot
            .evaluate(&target(Some("other.example"), Some("192.0.2.1")))
            .unwrap();
        assert_eq!(miss.policy, Policy::Reject, "域名未命中时 IP 候选必须生效");
        // 无域名时 IP 规则同样生效。
        let address_only = snapshot.evaluate(&target(None, Some("192.0.2.1"))).unwrap();
        assert_eq!(address_only.policy, Policy::Reject);
        // Reject 始终最先，不能被直连域名规则覆盖。
        let reject = snapshot
            .evaluate(&target(Some("blocked.example"), Some("192.0.2.1")))
            .unwrap();
        assert_eq!(reject.policy, Policy::Reject);
        let outside = snapshot
            .evaluate(&target(Some("example.com"), Some("198.51.100.1")))
            .unwrap();
        assert_eq!(outside.policy, Policy::Direct);
    }

    // ---------------------------------------------------------------- 场景 11

    #[test]
    fn scenario_11_build_peak_stays_within_budget() {
        let usage = memory_usage();
        assert!(
            usage.peak_reserved_bytes <= usage.limit_bytes,
            "构建峰值 {} 已超过准入上限 {}",
            usage.peak_reserved_bytes,
            usage.limit_bytes
        );
        // 高标签后缀在达到规则数量上限前就会被准入拒绝，且拒绝后峰值仍不得越界。
        let dense: Vec<Rule> = (0..20_000)
            .map(|_| {
                rule(
                    Match::DomainSuffix,
                    &("a.".repeat(125) + "com"),
                    Policy::Direct,
                )
            })
            .collect();
        let result = Snapshot::generate(settings(), vec![source("dense", dense)], 1);
        assert!(
            matches!(result, Err(Error::ResourceBudgetExceeded { .. })),
            "超出预算的输入必须以资源错误拒绝，实际：{:?}",
            result.err()
        );
        let usage = memory_usage();
        assert!(
            usage.peak_reserved_bytes <= usage.limit_bytes,
            "被拒绝之后峰值 {} 仍不得超过上限 {}",
            usage.peak_reserved_bytes,
            usage.limit_bytes
        );
    }

    // ---------------------------------------------------------------- 场景 12

    #[test]
    fn scenario_12_retained_snapshot_is_still_counted() {
        let dir = temp_dir("retain");
        let rules = || -> Vec<Rule> {
            (0..2_000)
                .map(|index| {
                    rule(
                        Match::Domain,
                        &format!("h{index}.retain.test"),
                        Policy::Direct,
                    )
                })
                .collect()
        };
        let store = Store::open(&dir).unwrap();
        store
            .update(settings(), vec![source("local", rules())])
            .unwrap();
        let first = store.snapshot().unwrap();
        let with_first = memory_usage().reserved_bytes;

        store
            .update(settings(), vec![source("local", rules())])
            .unwrap();
        let _second = store.snapshot().unwrap();
        let with_both = memory_usage().reserved_bytes;
        assert!(
            with_both > with_first,
            "旧索引快照仍被引用时必须叠加计入：{with_both} 未超过 {with_first}"
        );

        drop(first);
        let after_release = memory_usage().reserved_bytes;
        assert!(
            after_release < with_both,
            "释放旧快照引用后占用必须下降：{after_release} 未低于 {with_both}"
        );
        let _ = store.close();
    }

    // ---------------------------------------------------------------- 场景 13

    #[test]
    fn scenario_13_incompatible_format_version_is_reported() {
        let dir = temp_dir("version");
        write_cache(
            &dir,
            vec![rule(Match::Domain, "example.com", Policy::Direct)],
        );
        let result = tamper_and_reload(&dir, |bytes| {
            // 版本字段位于缓存头部 8..12；改成实现不认识的版本。
            bytes[8..12].copy_from_slice(&99u32.to_le_bytes());
        });
        assert!(
            matches!(result, Err(Error::Unsupported)),
            "不兼容的文件格式版本必须返回明确错误，实际：{result:?}"
        );
    }

    // ---------------------------------------------------------------- 场景 14

    #[test]
    fn scenario_14_domain_normalization_never_misses() {
        let dir = temp_dir("normalize");
        let snapshot = flat_snapshot(
            &dir,
            vec![
                rule(Match::Domain, "Example.COM", Policy::Reject),
                rule(Match::DomainSuffix, "Sub.Example.COM", Policy::Reject),
            ],
        );
        for query in ["example.com", "EXAMPLE.COM", "Example.Com", "example.com."] {
            let decision = snapshot
                .evaluate(&target(Some(query), Some("192.0.2.1")))
                .unwrap();
            assert_eq!(
                decision.policy,
                Policy::Reject,
                "精确域名在规范化差异下漏匹配：{query}"
            );
        }
        for query in [
            "x.sub.example.com",
            "X.SUB.EXAMPLE.COM",
            "x.sub.example.com.",
        ] {
            let decision = snapshot
                .evaluate(&target(Some(query), Some("192.0.2.1")))
                .unwrap();
            assert_eq!(
                decision.policy,
                Policy::Reject,
                "后缀规则在规范化差异下漏匹配：{query}"
            );
        }
        // 反例：不同域名不得被规范化命中。
        let decision = snapshot
            .evaluate(&target(Some("example.org"), Some("192.0.2.1")))
            .unwrap();
        assert_eq!(decision.policy, Policy::Direct, "不相关域名不得命中");
        // 结尾多一个点与大小写混合同时出现时同样不得漏匹配。
        let decision = snapshot
            .evaluate(&target(Some("X.Sub.Example.COM."), Some("192.0.2.1")))
            .unwrap();
        assert_eq!(decision.policy, Policy::Reject);
    }

    // ---------------------------------------------------------------- 自检

    /// 篡改路径的自检：把缓存原样重写（只重算摘要）后必须仍能加载。
    ///
    /// 这条不通过时，上面所有「拒绝非法索引」的结论都不成立——它们可能只是
    /// 摘要不匹配造成的 `InvalidInput`。
    #[test]
    fn control_rewrite_with_recomputed_digest_still_loads() {
        let dir = temp_dir("control");
        write_cache(
            &dir,
            vec![
                rule(Match::Domain, "one.example", Policy::Direct),
                rule(Match::IpCidr, "192.0.2.0/24", Policy::Reject),
                rule(Match::DomainKeyword, "abc", Policy::Direct),
            ],
        );
        let original = std::fs::read(dir.join(CACHE_FILE)).unwrap();
        write_with_digest(&dir, &original);
        assert!(
            reload(&dir).is_ok(),
            "原样重写并重算摘要后必须仍能加载，否则篡改用例的拒绝结论无效"
        );
        // 摘要写错时必须被校验和拦下，证明校验确实生效。
        let mut broken = original.clone();
        broken[DIGEST_OFFSET] ^= 0xff;
        std::fs::write(dir.join(CACHE_FILE), &broken).unwrap();
        let result = reload(&dir);
        assert!(
            matches!(result, Err(Error::InvalidInput)),
            "摘要错误必须被拒绝，实际：{result:?}"
        );
    }
    #[test]
    fn previous_cache_layout_requires_explicit_rebuild() {
        let dir = temp_dir("previousformat");
        write_cache(
            &dir,
            vec![rule(Match::Domain, "blocked.example", Policy::Reject)],
        );
        let file = dir.join(CACHE_FILE);
        let mut bytes = std::fs::read(&file).unwrap();
        bytes[8..12].copy_from_slice(&PREVIOUS_CACHE_VERSION.to_le_bytes());
        std::fs::write(&file, &bytes).unwrap();
        assert!(matches!(reload(&dir), Err(Error::Unsupported)));
        assert_eq!(
            std::fs::read(&file).unwrap(),
            bytes,
            "不支持的旧格式不能被静默覆盖或当成空规则接受"
        );
    }
    /// 碰撞布局复用字符串池后，其他索引区段和缓存恢复不得指向旧偏移。
    #[test]
    fn collision_fallback_preserves_other_sections() {
        let names = collision_names();
        let mut rules: Vec<_> = names
            .iter()
            .map(|name| rule(Match::Domain, name, Policy::Direct))
            .collect();
        rules.extend([
            rule(Match::DomainSuffix, "suffix.example", Policy::Reject),
            rule(Match::DomainKeyword, "needle", Policy::Reject),
            rule(Match::IpCidr, "192.0.2.0/24", Policy::Reject),
            rule(Match::IpCidr, "2001:db8::/32", Policy::Reject),
        ]);
        let dir = temp_dir("collision-mixed");
        write_cache(&dir, rules);
        let store = Store::open(&dir).unwrap();
        let snapshot = store.snapshot().unwrap();
        for (id, name) in names.iter().enumerate() {
            assert_eq!(
                snapshot
                    .evaluate(&target(Some(name), None))
                    .unwrap()
                    .rule_index,
                Some(id)
            );
        }
        for (domain, address, expected_id) in [
            (Some("x.suffix.example"), None, 65),
            (Some("x.needle.example"), None, 66),
            (None, Some("192.0.2.1"), 67),
            (None, Some("2001:db8::1"), 68),
        ] {
            let decision = snapshot.evaluate(&target(domain, address)).unwrap();
            assert_eq!(decision.policy, Policy::Reject);
            assert_eq!(decision.rule_index, Some(expected_id));
        }
        store.close().unwrap();
    }
}
