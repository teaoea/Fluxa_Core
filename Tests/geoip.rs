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

mod support;
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
    let path = std::env::temp_dir().join(format!("fluxa-geoip-corruption-{}", std::process::id()));
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
