//! 测试文件：tests/test_geoip.rs
//! 测试类型：单元测试、迁移回归
//! 测试对象：src/geoip.rs
//! 测试内容：
//! - ready_database_binds_country_rules_and_source_priority：Ready来源验证、同来源显式 CIDR 优先 GeoIP、数据库生命周期与树/平铺一致。
//! - invalid_database_is_rejected：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - complete_examples_have_strict_yaml_and_disabled_geoip：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - bounded_diagnostic_retains_download_reason_and_correct_levels：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - offline_ipv4_ipv6_missing_invalid_and_file_limits：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - configured_url_is_strict_and_never_defaults：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - configured_https_download_integrity_budget_and_no_match：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - all_user_assigned_geoip_failure_codes_are_exact：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
//! - reachable_corruption_is_rejected_and_query_fault_is_not_no_match：保留原用例的独立预期和全部断言；具体输入及判定见同名函数。
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
    fn invalid_database_is_rejected() {
        let path = std::env::temp_dir().join(format!("fluxa-geoip-invalid-{}", std::process::id()));
        std::fs::write(&path, b"not an mmdb").unwrap();
        assert!(Database::open(&path).is_err());
        std::fs::remove_file(path).unwrap();
    }
}
#[cfg(test)]
mod classification_tests {
    use super::super::*;
    #[test]
    fn complete_examples_have_strict_yaml_and_disabled_geoip() {
        {
            let yaml = include_str!("../example_config.yaml");
            // 主配置已改为 geoip.enabled/url；必须经真实入口校验，不能把禁用时保留的 URL 当作启用。
            let document = crate::configuration::Document::from_yaml(yaml).unwrap();
            assert!(!document.summary().geoip_enabled);
            assert_eq!(document.geoip_url(), None);
            let config = Config {
                geoip_url: document.geoip_url().map(str::to_owned),
            };
            config.validate().unwrap();
            assert!(Service::new(config).unwrap().is_disabled());
        }
    }
    #[test]
    fn bounded_diagnostic_retains_download_reason_and_correct_levels() {
        assert_eq!(
            access_error(Error::NetworkTimeout),
            Error::GeoipDownloadTimeout
        );
        assert_eq!(access_error(Error::DnsTimeout), Error::GeoipDownloadFailed);
        assert_eq!(access_error(Error::Tls), Error::GeoipDownloadFailed);
        assert_eq!(
            access_error(Error::ResourceLimit),
            Error::GeoipResourceLimit
        );
        assert_eq!(
            ErrorCode::GeoipNotConfigured.level(),
            crate::error::LogLevel::Warn
        );
        assert_eq!(ErrorCode::GeoipReady.level(), crate::error::LogLevel::Info);
        assert_eq!(
            ErrorCode::GeoipNoMatch.level(),
            crate::error::LogLevel::Info
        );
        assert_eq!(
            ErrorCode::GeoipQueryFailed.level(),
            crate::error::LogLevel::Error
        );
    }
}

use crate::test_fixture_https as support;
// 以下保留迁移前的用例说明；历史执行状态不代表当前版本，当前结果仅见同名报告。
mod public_contract {
    use crate as libfluxa_core;
    use libfluxa_core::{error::Error, geoip::Database};
    use support::geoip_data as fixture;
    #[test]
    fn offline_ipv4_ipv6_missing_invalid_and_file_limits() {
        let path = std::env::temp_dir().join(format!("fluxa-geoip-test-{}", std::process::id()));
        std::fs::write(&path, fixture::fixture()).unwrap();
        let database = Database::open(&path).unwrap();
        let result = database.lookup("8.8.8.8", "zh-CN").unwrap().unwrap();
        assert_eq!(result.country_code, "US");
        assert_eq!(result.city, "Los Angeles");
        assert_eq!(result.region, "California");
        assert_eq!(
            database
                .lookup("2001:4860:4860::8888", "en")
                .unwrap()
                .unwrap()
                .country_code,
            "TW"
        );
        assert_eq!(database.lookup("192.0.2.1", "en").unwrap(), None);
        assert_eq!(
            database.lookup("bad..address", "en"),
            Err(Error::InvalidInput)
        );
        assert_eq!(
            database.lookup(&"1".repeat(65), "en"),
            Err(Error::InvalidInput)
        );
        assert_eq!(
            database.lookup("8.8.8.8", &"e".repeat(17)),
            Err(Error::InvalidInput)
        );
        drop(database);
        std::fs::write(&path, b"invalid").unwrap();
        assert!(Database::open(&path).is_err());
        let file = std::fs::File::create(&path).unwrap();
        file.set_len(256 * 1024 * 1024 + 1).unwrap();
        assert!(matches!(
            Database::open(&path),
            Err(Error::GeoipResourceLimit)
        ));
        std::fs::remove_file(&path).unwrap();
    }

    use super::support;
    use libfluxa_core::error::ErrorCode;
    use libfluxa_core::geoip::{Config, Service};

    #[test]
    fn configured_url_is_strict_and_never_defaults() {
        assert_eq!(Config::from_yaml("{}").unwrap().geoip_url, None);
        assert_eq!(
            Config::from_yaml("geoip_url: null").unwrap().geoip_url,
            None
        );
        for value in [
            "",
            "bad url",
            "https://bad..host/db",
            "https://user:password@example.com/db",
            "https://example.com/db#fragment",
            "https://example.com:0/db",
        ] {
            assert_eq!(
                Config {
                    geoip_url: Some(value.into())
                }
                .validate(),
                Err(Error::GeoipInvalidUrl)
            );
        }
        for value in [
            "http://example.com/db",
            "file:///tmp/db",
            "ftp://example.com/db",
        ] {
            assert_eq!(
                Config {
                    geoip_url: Some(value.into())
                }
                .validate(),
                Err(Error::GeoipUrlScheme)
            );
        }
        assert!(matches!(
            Config::from_yaml("geoip_url: https://example.com/a\ngeoip_url: https://example.com/b"),
            Err(Error::ConfigDuplicateField(_))
        ));
        assert!(matches!(
            Config::from_yaml("url: https://example.com/a"),
            Err(Error::ConfigUnknownField(_))
        ));
        let disabled = Service::default();
        let lookup = disabled.lookup("not-an-address", "en").unwrap();
        assert_eq!(lookup.status.state, "disabled");
        assert_eq!(lookup.status.code, ErrorCode::GeoipNotConfigured);
        assert!(lookup.location.is_none());
        assert_eq!(disabled.warning().unwrap().error.code.value(), 2410);
        let enabled = Service::new(Config {
            geoip_url: Some("https://example.com/geo.mmdb".into()),
        })
        .unwrap();
        assert!(enabled.warning().is_none());
        assert!(matches!(
            enabled.lookup("8.8.8.8", "en"),
            Err(Error::GeoipInitializationFailed)
        ));
    }

    #[tokio::test]
    async fn configured_https_download_integrity_budget_and_no_match() {
        let fixture = support::fixture().await;
        let mut roots = rustls::RootCertStore::empty();
        roots.add(fixture.root.clone()).unwrap();
        let client = libfluxa_core::https::HttpsClient::with_roots(roots)
            .unwrap()
            .with_network(fixture.network.clone());
        let url = |path: &str| Config {
            geoip_url: Some(format!("https://localhost:{}{path}", fixture.port)),
        };
        // 预期来自用户状态表：缺失关闭、合法无记录成功、故障必须 Err。
        assert_eq!(
            Service::default().refresh(&client).await.unwrap().state,
            "disabled"
        );
        let service = Service::new(url("/geoip")).unwrap();
        assert_eq!(service.refresh(&client).await.unwrap().code.value(), 2411);
        assert_eq!(
            service
                .lookup("8.8.8.8", "en")
                .unwrap()
                .location
                .unwrap()
                .country_code,
            "US"
        );
        let missing = service.lookup("192.0.2.1", "en").unwrap();
        assert_eq!(missing.status.code.value(), 2412);
        assert!(missing.location.is_none());
        assert_eq!(
            service.lookup("invalid", "en").err(),
            Some(Error::InvalidInput)
        );
        service.close().unwrap();
        assert_eq!(
            service.lookup("8.8.8.8", "en").err(),
            Some(Error::GeoipInitializationFailed)
        );
        drop(service);
        for (path, expected) in [
            ("/geoip-corrupt", Error::GeoipInvalidDatabase),
            ("/geoip-truncated", Error::GeoipIntegrityFailed),
            ("/geoip-limit", Error::GeoipResourceLimit),
            ("/redirect", Error::GeoipDownloadFailed),
            ("/subscription-error", Error::GeoipDownloadFailed),
        ] {
            let service = Service::new(url(path)).unwrap();
            assert_eq!(
                service.refresh(&client).await.err(),
                Some(expected),
                "{path}"
            );
            assert_eq!(
                service.lookup("8.8.8.8", "en").err(),
                Some(Error::GeoipInitializationFailed)
            );
        }
        // 真正超过普通 HTTP 的 8 MiB 限额，验证数据库专用入口没有扩大普通请求。
        let large = Service::new(url("/geoip-large")).unwrap();
        assert_eq!(large.refresh(&client).await.unwrap().state, "ready");
        assert_eq!(
            large
                .lookup("8.8.8.8", "en")
                .unwrap()
                .location
                .unwrap()
                .country_code,
            "US"
        );
        assert!(matches!(
            client
                .execute_limited(&url("/geoip-large").geoip_url.unwrap(), 1024)
                .await,
            Err(Error::ResourceLimit)
        ));
        drop(large);
        let delayed = Service::new(url("/wait")).unwrap();
        assert_eq!(
            delayed.refresh(&client).await.err(),
            Some(Error::GeoipDownloadTimeout)
        );
    }

    #[test]
    fn all_user_assigned_geoip_failure_codes_are_exact() {
        for (error, expected) in [
            (Error::GeoipInvalidUrl, 2401),
            (Error::GeoipUrlScheme, 2402),
            (Error::GeoipDownloadFailed, 2403),
            (Error::GeoipDownloadTimeout, 2404),
            (Error::GeoipInvalidDatabase, 2405),
            (Error::GeoipIntegrityFailed, 2406),
            (Error::GeoipResourceLimit, 2407),
            (Error::GeoipInitializationFailed, 2408),
            (Error::GeoipQueryFailed, 2409),
        ] {
            assert_eq!(error.code().value(), expected);
            let info = error.public_info();
            let bytes = serde_json::to_vec(&info).unwrap();
            assert_eq!(
                libfluxa_core::error::ErrorInfo::from_json(&bytes).unwrap(),
                info
            );
        }
    }

    #[test]
    fn reachable_corruption_is_rejected_and_query_fault_is_not_no_match() {
        let path =
            std::env::temp_dir().join(format!("fluxa-geoip-corruption-{}", std::process::id()));
        let mut damaged = fixture::fixture();
        damaged[..3].fill(0); // 左分支回到根节点：不能以合法 MMDB 元数据掩盖搜索树损坏。
        std::fs::write(&path, damaged).unwrap();
        assert!(matches!(
            Database::open(&path),
            Err(Error::GeoipIntegrityFailed)
        ));
        let mut valid_format = fixture::fixture();
        let offset = valid_format
            .windows(3)
            .position(|bytes| bytes == b"\x42US")
            .unwrap();
        valid_format[offset + 1] = b'u'; // MMDB 编码合法，但国家字段违反已确认返回规范。
        std::fs::write(&path, valid_format).unwrap();
        let database = Database::open(&path).unwrap();
        assert_eq!(
            database.lookup("8.8.8.8", "en"),
            Err(Error::GeoipQueryFailed)
        );
        std::fs::remove_file(path).unwrap();
    }
}

#[test]
fn ready_database_binds_country_rules_and_source_priority() {
    use super::*;
    use crate::routing::{
        Config as RoutingConfig, Match, Policy, Rule, Target, Transport,
        runtime::{Layer, Reference, Snapshot, Source},
    };
    use std::sync::Arc;
    let path = std::env::temp_dir().join(format!(
        "fluxa-country-rule-{}-{}",
        std::process::id(),
        crate::lifecycle::identifier().unwrap()
    ));
    std::fs::write(&path, crate::test_fixture_https::geoip_data::fixture()).unwrap();
    let database = Arc::new(Database::open(&path).unwrap());
    std::fs::remove_file(path).unwrap();
    let service = Service::new(super::Config {
        geoip_url: Some("https://example.com/ready.mmdb".into()),
    })
    .unwrap();
    let yaml = "geoip: {enabled: true, url: 'https://example.com/ready.mmdb'}\nrules: ['geoip,US,direct', 'final,reject']";
    assert!(matches!(
        crate::configuration::Document::from_yaml(yaml)
            .unwrap()
            .compile_local_rules_with_geoip(1, &service),
        Err(Error::GeoipInitializationFailed)
    ));
    *service.database.lock().unwrap() = Some(database.clone());
    let compiled = crate::configuration::Document::from_yaml(yaml)
        .unwrap()
        .compile_local_rules_with_geoip(2, &service)
        .unwrap();
    let wrong = yaml.replace("ready.mmdb", "other.mmdb");
    assert!(matches!(
        crate::configuration::Document::from_yaml(&wrong)
            .unwrap()
            .compile_local_rules_with_geoip(2, &service),
        Err(Error::GeoipInitializationFailed)
    ));
    let request = |ip: &str| Target {
        domain: None,
        address: Some(ip.into()),
        port: 443,
        transport: Transport::Tcp,
    };
    for (ip, expected) in [
        ("8.8.8.8", Policy::Direct),
        ("2001:4860:4860::8888", Policy::Reject),
        ("192.0.2.1", Policy::Reject),
    ] {
        assert_eq!(compiled.evaluate(&request(ip)).unwrap().policy, expected);
    }
    let row = |kind, value: &str, policy| Rule {
        protocol: None,
        r#type: kind,
        value: value.into(),
        policy,
        outbound: (policy == Policy::Proxy).then(|| "geo-proxy".into()),
    };
    let sources = vec![Source {
        id: "local".into(),
        layer: Layer::Local,
        enabled: true,
        rules: vec![
            row(Match::GeoIp, "US", Policy::Proxy),
            row(Match::IpCidr, "8.8.8.8/32", Policy::Direct),
        ],
    }];
    let reference =
        Reference::compile_with_geoip(RoutingConfig::default(), sources.clone(), 3, &service)
            .unwrap();
    let snapshot = Snapshot::generate(RoutingConfig::default(), sources, 3)
        .unwrap()
        .bind_geoip(database.clone());
    let assert_expected = |snapshot: &Snapshot| {
        let decision = snapshot.evaluate(&request("8.8.8.8")).unwrap();
        assert_eq!(
            decision.policy,
            Policy::Direct,
            "用户最新确认：同来源显式 IP-CIDR 优先 GeoIP"
        );
        assert_eq!(decision, reference.evaluate(&request("8.8.8.8")).unwrap());
    };
    assert_expected(&snapshot);
    let flat = snapshot.into_flat().unwrap();
    assert_expected(&flat);
    let loaded =
        Snapshot::from_flat_document(flat.document().clone(), flat.flat_bytes().unwrap().to_vec())
            .unwrap();
    assert!(
        matches!(
            loaded.evaluate(&request("8.8.8.8")),
            Err(Error::GeoipInitializationFailed)
        ),
        "恢复的结构化缓存不得伪造 Ready"
    );
    let loaded = loaded.bind_geoip(database.clone());
    assert_expected(&loaded);
    // 反转输入顺序仍由显式 CIDR 先命中；优先级来自独立业务类型顺序。
    let reversed = Snapshot::generate(
        RoutingConfig::default(),
        vec![Source {
            id: "local".into(),
            layer: Layer::Local,
            enabled: true,
            rules: vec![
                row(Match::IpCidr, "8.8.8.8/32", Policy::Direct),
                row(Match::GeoIp, "US", Policy::Proxy),
            ],
        }],
        4,
    )
    .unwrap()
    .bind_geoip(database.clone())
    .into_flat()
    .unwrap();
    assert_eq!(
        reversed.evaluate(&request("8.8.8.8")).unwrap().policy,
        Policy::Direct
    );

    // 来源先于 IP 同类原序；特殊 REJECT 则跨来源优先。预期直接来自用户约定。
    let source = |layer, id: &str, rules| Source {
        id: id.into(),
        layer,
        enabled: true,
        rules,
    };
    for (sources, expected) in [
        (
            vec![
                source(
                    Layer::Remote,
                    "remote",
                    vec![row(Match::IpCidr, "8.8.8.8/32", Policy::Direct)],
                ),
                source(
                    Layer::Local,
                    "local",
                    vec![row(Match::GeoIp, "US", Policy::Proxy)],
                ),
            ],
            Policy::Proxy,
        ),
        (
            vec![
                source(
                    Layer::Remote,
                    "remote",
                    vec![row(Match::IpCidr, "8.8.8.8/32", Policy::Direct)],
                ),
                source(
                    Layer::Plugin,
                    "plugin",
                    vec![row(Match::GeoIp, "US", Policy::Proxy)],
                ),
            ],
            Policy::Proxy,
        ),
        (
            vec![
                source(
                    Layer::Local,
                    "local",
                    vec![row(Match::GeoIp, "US", Policy::Proxy)],
                ),
                source(
                    Layer::Remote,
                    "remote",
                    vec![row(Match::IpCidr, "8.8.8.8/32", Policy::Reject)],
                ),
            ],
            Policy::Reject,
        ),
        (
            vec![source(
                Layer::Local,
                "local",
                vec![
                    row(Match::GeoIp, "US", Policy::Reject),
                    row(Match::IpCidr, "8.8.8.8/32", Policy::Reject),
                ],
            )],
            Policy::Reject,
        ),
    ] {
        let reference =
            Reference::compile_with_geoip(RoutingConfig::default(), sources.clone(), 5, &service)
                .unwrap();
        let tree = Snapshot::generate(RoutingConfig::default(), sources, 5)
            .unwrap()
            .bind_geoip(database.clone());
        let decision = tree.evaluate(&request("8.8.8.8")).unwrap();
        assert_eq!(decision.policy, expected);
        assert_eq!(decision, reference.evaluate(&request("8.8.8.8")).unwrap());
        let flat = tree.into_flat().unwrap();
        assert_eq!(flat.evaluate(&request("8.8.8.8")).unwrap(), decision);
    }
    for mode in [crate::routing::Mode::Global, crate::routing::Mode::Direct] {
        let settings = RoutingConfig {
            mode,
            fallback: Policy::Direct,
            ..RoutingConfig::default()
        };
        let source = |policy| Source {
            id: "local".into(),
            layer: Layer::Local,
            enabled: true,
            rules: vec![row(Match::GeoIp, "US", policy)],
        };
        // 普通规则被忽略时不应查询无关数据库，特殊 REJECT 仍须 Ready 且保留执行。
        let ignored = Snapshot::generate(settings.clone(), vec![source(Policy::Proxy)], 6)
            .unwrap()
            .into_flat()
            .unwrap();
        assert_eq!(
            ignored.evaluate(&request("8.8.8.8")).unwrap().policy,
            Policy::Direct
        );
        let blocked = Snapshot::generate(settings, vec![source(Policy::Reject)], 6).unwrap();
        assert!(matches!(
            blocked.evaluate(&request("8.8.8.8")),
            Err(Error::GeoipInitializationFailed)
        ));
        assert_eq!(
            blocked
                .bind_geoip(database.clone())
                .evaluate(&request("8.8.8.8"))
                .unwrap()
                .policy,
            Policy::Reject
        );
    }
    service.close().unwrap();
    assert_eq!(
        compiled.evaluate(&request("8.8.8.8")).unwrap().policy,
        Policy::Direct,
        "关闭来源服务不撤销已发布不可变数据库内容，实际内存仍有配额"
    );
}
