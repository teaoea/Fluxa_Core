//! 测试文件：tests/test_flat_cache_round_trip.rs
//! 测试类型：集成测试、持久化和独立策略回归
//! 测试对象：src/routing/flat.rs、src/routing/cache.rs、src/routing/store.rs
//! 测试内容：cache_round_trip_preserves_independent_policy_matrix：双栈互补网段、精确域名、后缀代理和未命中在缓存恢复前后保持明确策略与出口。
//! 测试依据：合法前缀树及域名规则必须可持久化；DIRECT、PROXY、REJECT 与出口不得因加载校验或布局切换改变。
//! 前置条件：仅独占临时目录，无系统 VPN、网络监听或外部服务。
use libfluxa_core::routing::{
    Config, Match, Policy, Rule, Target, Transport,
    runtime::{Layer, Source},
    store::Store,
};
use std::path::PathBuf;
struct Directory(PathBuf);
impl Drop for Directory {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_dir_all(&self.0) {
            eprintln!("集成测试目录清理失败：{error}");
        }
    }
}
#[test]
fn cache_round_trip_preserves_independent_policy_matrix() {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "fluxa-flat-contract-{}-{unique}",
        std::process::id()
    ));
    std::fs::create_dir(&path).unwrap();
    let directory = Directory(path);
    let rules = [
        (Match::IpCidr, "0.0.0.0/1", Policy::Direct),
        (Match::IpCidr, "128.0.0.0/1", Policy::Reject),
        (Match::IpCidr, "::/1", Policy::Direct),
        (Match::IpCidr, "8000::/1", Policy::Reject),
        (Match::Domain, "allow.example", Policy::Direct),
        (Match::Domain, "blocked.example", Policy::Reject),
        (Match::DomainSuffix, "lab.example", Policy::Proxy),
    ]
    .into_iter()
    .map(|(r#type, value, policy)| Rule {
        r#type,
        value: value.into(),
        policy,
        protocol: None,
        outbound: (policy == Policy::Proxy).then(|| "contract-proxy".into()),
    })
    .collect();
    let store = Store::open(&directory.0).unwrap();
    store
        .update(
            Config::default(),
            vec![Source {
                id: "contract".into(),
                layer: Layer::Local,
                enabled: true,
                rules,
            }],
        )
        .unwrap();
    let assert_expected = |store: &Store| {
        // 预期来自具体规则文本，而不是调用另一个实现推导。
        for (domain, address, policy, outbound) in [
            (None, Some("1.2.3.4"), Policy::Direct, None),
            (None, Some("192.0.2.1"), Policy::Reject, None),
            (None, Some("2001:db8::1"), Policy::Direct, None),
            (None, Some("fd00::1"), Policy::Reject, None),
            (Some("allow.example"), None, Policy::Direct, None),
            (Some("blocked.example"), None, Policy::Reject, None),
            (
                Some("api.lab.example"),
                None,
                Policy::Proxy,
                Some("contract-proxy"),
            ),
            (Some("unmatched.example"), None, Policy::Reject, None),
        ] {
            let decision = store
                .snapshot()
                .unwrap()
                .evaluate(&Target {
                    domain: domain.map(Into::into),
                    address: address.map(Into::into),
                    port: 443,
                    transport: Transport::Tcp,
                })
                .unwrap();
            assert_eq!(decision.policy, policy);
            assert_eq!(decision.outbound.as_deref(), outbound);
        }
    };
    assert_expected(&store);
    store.close().unwrap();
    let recovered = Store::open(&directory.0).unwrap();
    assert_expected(&recovered);
    recovered.close().unwrap();
}
