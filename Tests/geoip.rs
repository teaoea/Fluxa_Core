use libfluxa_core::{error::Error, geoip::Database};
#[path = "../Tests/geoip_fixture.rs"]
mod fixture;
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
    assert!(matches!(Database::open(&path), Err(Error::ResourceLimit)));
    std::fs::remove_file(&path).unwrap();
}
