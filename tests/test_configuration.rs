//! 测试文件：tests/test_configuration.rs
//! 测试类型：单元测试、主配置边界与资源回归
//! 测试对象：src/configuration.rs
//! 测试内容：
//! - address_filters_fake_ip_switch_and_reserved_groups_are_bounded：按新配置契约独立验证正反边界。
//! - mitm_exclusions_precede_hosts_and_debug_options_are_not_configurable：按新配置契约独立验证正反边界。
//! - lan_authentication_is_strict_and_never_partially_disabled：按新配置契约独立验证正反边界。
//! - minimal_configuration_and_per_server_timeout：最小配置与每上游超时归一化。
//! - listener_ports_are_independent_and_disjoint：端口边界与非零互斥。
//! - strict_fields_and_types_reject_legacy_debug_and_duplicate_input：旧字段、Debug、重复字段与类型拒绝。
//! - node_identity_and_references_are_case_sensitive：节点唯一、保留名、引用区分大小写。
//! - provider_metadata_is_strict_and_cache_names_are_safe：提供者字段、时间、路径及重名拒绝。
//! - dns_bootstrap_routes_and_disabled_subsystem：数字引导、显式 system 上游与顺序、路由引用和关闭隔离。
//! - geoip_and_tun_fail_before_activation：启用时 URL/未实现栈拒绝。
//! - bounded_input_and_lifetime_accounting：长度限制、失败释放与常驻预算租约。
//! - complete_examples_are_accepted：当前完整示例的真实解析。
//!
//! 测试依据：用户 2026-10-16 已确认主配置约定；测试独立写明期望，不从生产结果生成预期。
//!
//! 前置条件：无网络、监听、系统代理或 VPN 副作用；预算测试在套件内串行执行。
use super::*;
use serde_json::{Value, json};
static GATE: std::sync::Mutex<()> = std::sync::Mutex::new(());
fn input() -> Value {
    json!({"rules":["final,direct"]})
}
fn parse(value: Value) -> Result<Document> {
    Document::from_yaml(&value.to_string())
}
fn proxy(name: &str) -> Value {
    json!({"name":name,"type":"vless","server":"example.com","port":443,
        "uuid":"00000000-0000-0000-0000-000000000000","flow":"xtls-rprx-vision",
        "security":"reality","sni":"example.com","pbk":"AQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQE","sid":"01"})
}
#[test]
fn minimal_configuration_and_per_server_timeout() {
    let _gate = GATE.lock().unwrap();
    let summary = parse(input()).unwrap().summary();
    assert_eq!(summary.mode, Mode::Rule);
    assert_eq!((summary.system_proxy_port, summary.lan_share_port), (0, 0));
    for (value, expected) in [(-1, 3), (0, 3), (1, 1), (3, 3), (30, 30), (31, 3)] {
        let mut config = input();
        config["dns"] = json!({"enable":true,"server":["1.1.1.1"],"timeout":value});
        assert_eq!(
            parse(config).unwrap().summary().dns_timeout_seconds,
            expected
        );
    }
}
#[test]
fn listener_ports_are_independent_and_disjoint() {
    let _gate = GATE.lock().unwrap();
    for (system, lan, accepted) in [
        (0, 0, true),
        (1054, 0, true),
        (0, 65535, true),
        (1054, 1055, true),
        (1, 0, false),
        (0, 1053, false),
        (1054, 1054, false),
        (65536, 0, false),
    ] {
        let mut config = input();
        config["system-proxy"] = json!({"port":system});
        config["lan-share"] = json!({"port":lan});
        assert_eq!(parse(config).is_ok(), accepted, "{system}/{lan}");
    }
}
#[test]
fn strict_fields_and_types_reject_legacy_debug_and_duplicate_input() {
    let _gate = GATE.lock().unwrap();
    for key in [
        "subscriptions",
        "system-proxy-port",
        "lan-share-port",
        "ip_strategy",
        "nodes",
        "routing",
        "geoip_url",
        "debug",
        "skip-cert-verify",
    ] {
        let mut config = input();
        config[key] = json!([]);
        assert!(
            matches!(parse(config), Err(Error::ConfigUnknownField(_))),
            "{key}"
        );
    }
    assert!(matches!(
        Document::from_yaml("rules: [final,direct]\nmode: rule\nmode: direct"),
        Err(Error::ConfigDuplicateField(_)) | Err(Error::InvalidInput)
    ));
    assert!(matches!(
        Document::from_yaml("rules: ['final,direct']\nmode: rule\nmode: direct"),
        Err(Error::ConfigDuplicateField(_))
    ));
    for value in [json!(true), json!("1054"), json!(-1)] {
        let mut config = input();
        config["system-proxy"] = json!({"port":value});
        assert!(matches!(parse(config), Err(Error::InvalidInput)));
    }
    let mut config = input();
    config["dns"] = json!({"enable":true,"server":["1.1.1.1"],"dns-hijack":true});
    assert!(matches!(parse(config), Err(Error::ConfigUnknownField(_))));
    assert!(Document::from_yaml("rules: &rules ['final,direct']").is_err());
}
#[test]
fn node_identity_and_references_are_case_sensitive() {
    let _gate = GATE.lock().unwrap();
    let mut config = input();
    config["proxies"] = json!([proxy("PROXY")]);
    config["rules"] = json!(["final,PROXY"]);
    assert!(parse(config.clone()).is_ok());
    config["rules"] = json!(["final,proxy"]);
    assert!(matches!(parse(config), Err(Error::InvalidInput)));
    for name in ["direct", "reject", "", "bad,node"] {
        let mut config = input();
        config["proxies"] = json!([proxy(name)]);
        assert!(matches!(parse(config), Err(Error::InvalidInput)));
    }
    let mut config = input();
    config["proxies"] = json!([proxy("A"), proxy("A")]);
    assert!(parse(config).is_err());
    let mut config = input();
    config["proxies"] = json!([proxy("A"), proxy("a")]);
    assert!(parse(config).is_ok());
    let mut config = input();
    let mut node = proxy("A");
    node["pbk"] = json!("<public key>");
    config["proxies"] = json!([node]);
    assert!(parse(config).is_err());
}
#[test]
fn provider_metadata_is_strict_and_cache_names_are_safe() {
    let _gate = GATE.lock().unwrap();
    let provider = json!({"name":"direct","url":"https://example.com/rules.yaml","policy":"direct","enable":false,"update-time":"12:00"});
    let mut config = input();
    config["rule-providers"] = json!([provider.clone()]);
    assert!(parse(config.clone()).is_ok());
    for name in ["..", "../other", "a/b", "a\\b", "trailing."] {
        let mut invalid = config.clone();
        invalid["rule-providers"][0]["name"] = json!(name);
        assert!(parse(invalid).is_err());
    }
    for time in ["24:00", "12:60", "1:00", "12:00:00"] {
        let mut invalid = config.clone();
        invalid["rule-providers"][0]["update-time"] = json!(time);
        assert!(parse(invalid).is_err());
    }
    config["rule-providers"] = json!([provider.clone(), provider]);
    assert!(parse(config).is_err());
    let mut config = input();
    config["rule-providers"] =
        json!([{"name":"x","url":"https://example.com/r.yaml","policy":"direct","enabled":true}]);
    assert!(matches!(parse(config), Err(Error::ConfigUnknownField(_))));
}
#[test]
fn dns_bootstrap_routes_and_disabled_subsystem() {
    let _gate = GATE.lock().unwrap();
    let mut config = input();
    config["dns"] = json!({"enable":true,"bootstrap":["1.1.1.1"],"server":["https://doh.example.com/dns-query,direct","2400:3200::1"]});
    assert!(parse(config.clone()).is_ok());
    config["dns"]["bootstrap"] = json!(["dns.example.com"]);
    assert!(parse(config.clone()).is_err());
    config["dns"]["bootstrap"] = json!(["1.1.1.1"]);
    config["dns"]["server"] = json!(["https://doh.example.com/dns-query,missing"]);
    assert!(parse(config.clone()).is_err());
    config["dns"]["server"] = json!(["https://doh.example.com/dns-query,reject"]);
    assert!(parse(config.clone()).is_err());
    // 独立约定：只允许字面量 system；显式位置保留，不附加隐式上游。
    let servers = json!([
        "https://doh.example.com/dns-query,direct",
        "system",
        "1.1.1.1"
    ]);
    config["dns"]["server"] = servers.clone();
    let document = parse(config.clone()).unwrap();
    assert_eq!(
        serde_json::to_value(&document.raw.dns.nameserver).unwrap(),
        servers
    );
    config["dns"]["server"] = json!(["1.1.1.1"]);
    assert_eq!(
        parse(config.clone()).unwrap().raw.dns.nameserver,
        vec!["1.1.1.1"]
    );
    config["dns"]["server"] = json!(["system"]);
    assert_eq!(
        parse(config.clone()).unwrap().raw.dns.nameserver,
        vec!["system"]
    );
    for invalid in [
        "SYSTEM",
        "system,direct",
        "system,missing",
        " system",
        "system ",
    ] {
        config["dns"]["server"] = json!([invalid]);
        assert!(
            matches!(parse(config.clone()), Err(Error::InvalidInput)),
            "{invalid}"
        );
    }
    config["dns"]["server"] = json!(["system"]);
    config["dns"]["bootstrap"] = json!(["system"]);
    assert!(
        matches!(parse(config.clone()), Err(Error::InvalidInput)),
        "bootstrap 不能使用 system"
    );
    config["dns"]["enable"] = json!(false);
    config["dns"]["hosts"] = json!({"example.com":"not-an-ip"});
    assert!(!parse(config).unwrap().summary().dns_enabled);
}
#[test]
fn geoip_and_tun_fail_before_activation() {
    let _gate = GATE.lock().unwrap();
    for (url, error) in [
        ("", Error::GeoipInvalidUrl),
        ("http://example.com/db.mmdb", Error::GeoipUrlScheme),
    ] {
        let mut config = input();
        config["geoip"] = json!({"enabled":true,"url":url});
        assert!(matches!(parse(config),Err(actual) if actual == error));
    }
    let mut config = input();
    config["geoip"] = json!({"enabled":true});
    assert!(matches!(parse(config), Err(Error::GeoipInvalidUrl)));
    for stack in ["system", "gvisor"] {
        let mut config = input();
        config["tun"] = json!({"enable":true,"stack":stack});
        assert!(matches!(parse(config), Err(Error::ConfigUnknownField(_))));
    }
}
#[test]
fn bounded_input_and_lifetime_accounting() {
    let _gate = GATE.lock().unwrap();
    static TEST_MEMORY: ByteBudget = ByteBudget::new(CONFIG_MEMORY_LIMIT);
    let before = TEST_MEMORY.usage().0;
    let text = input().to_string();
    let config = Document::parse_with_budget(&text, &TEST_MEMORY).unwrap();
    assert_eq!(
        TEST_MEMORY.usage().0,
        before + text.len() * PARSE_FACTOR + PARSE_BASE
    );
    assert!(Document::parse_with_budget("unknown: true", &TEST_MEMORY).is_err());
    assert_eq!(
        TEST_MEMORY.usage().0,
        before + text.len() * PARSE_FACTOR + PARSE_BASE
    );
    drop(config);
    assert_eq!(TEST_MEMORY.usage().0, before);
    let remaining = CONFIG_MEMORY_LIMIT - TEST_MEMORY.usage().0;
    let full = TEST_MEMORY.reserve(remaining).unwrap();
    assert!(matches!(
        Document::parse_with_budget(&text, &TEST_MEMORY),
        Err(Error::ResourceBudgetExceeded { .. })
    ));
    drop(full);
    assert!(Document::parse_with_budget(&text, &TEST_MEMORY).is_ok());
    assert!(matches!(
        Document::from_yaml(&" ".repeat(MAX_CONFIG_BYTES + 1)),
        Err(Error::ResourceLimit)
    ));
}
#[test]
fn complete_examples_are_accepted() {
    let _gate = GATE.lock().unwrap();
    {
        let text = include_str!("../example_config.yaml");
        // 用户示例显式启用加密并保留待替换公钥；占位符必须拒绝，不能放宽正式公钥校验。
        assert!(Document::from_yaml(text).is_err());
        use base64::Engine;
        use hpke::{Kem, Serializable};
        let (_, public) = hpke::kem::DhP256HkdfSha256::derive_keypair(
            b"independent public test key material for configuration fixture",
        );
        let key = base64::engine::general_purpose::STANDARD.encode(public.to_bytes());
        assert!(Document::from_yaml(&text.replace("BASE64_PUBLIC_KEY", &key)).is_ok());
        for default in [
            include_str!("../default_config.yaml"),
            include_str!("../default_config_en.yaml"),
        ] {
            assert!(Document::from_yaml(default).is_ok());
        }
    }
}

#[test]
fn lan_authentication_is_strict_and_never_partially_disabled() {
    for (account, password, enabled) in [("", "", false), ("user", "secret", true)] {
        let mut value = input();
        value["lan-share"] = json!({"account":account,"password":password});
        let document = parse(value).unwrap();
        assert_eq!(document.lan_authentication_enabled(), enabled);
        let summary = serde_json::to_string(&document.summary()).unwrap();
        assert!(!summary.contains("secret"));
    }
    assert!(!parse(input()).unwrap().lan_authentication_enabled());
    for (account, password) in [
        (json!("user"), json!("")),
        (json!(""), json!("secret")),
        (json!("user"), json!(12345678)),
        (json!("a:b"), json!("secret")),
        (json!("user"), json!("a\nb")),
        (json!("user"), json!("a".repeat(256))),
    ] {
        let mut value = input();
        value["lan-share"] = json!({"account":account,"password":password});
        assert!(parse(value).is_err());
    }
}
#[test]
fn mitm_exclusions_precede_hosts_and_debug_options_are_not_configurable() {
    let mut value = input();
    value["mitm"] = json!({"enabled":true,"hosts":["chatgpt.com","a.chatgpt.com","deep.a.chatgpt.com","exact.example.com","a.exact.example.com","other.com"],"bypass":["*.chatgpt.com","exact.example.com"],"capture-body":true,"skip-server-cert-verify":false});
    let all = parse(value.clone()).unwrap();
    // 精确清单包含被排除的域名，证明排除总先于普通 hosts；不伪造全量调试授权。
    for (host, expected) in [
        ("chatgpt.com", false),
        ("a.chatgpt.com", false),
        ("deep.a.chatgpt.com", false),
        ("exact.example.com", false),
        ("a.exact.example.com", true),
        ("other.com", true),
    ] {
        assert_eq!(all.mitm_matches(host).unwrap(), expected, "{host}");
    }
    for enabled in [true, false] {
        for all in [true, false] {
            let mut forbidden = value.clone();
            forbidden["mitm"]["enabled"] = json!(enabled);
            forbidden["mitm"]["decrypt-all"] = json!(all);
            // 既有公开参数 Schema 仅允许字母/数字/下划线；连字符字段安全省略为 null。
            assert!(
                matches!(parse(forbidden), Err(Error::ConfigUnknownField(None))),
                "正式配置不能声明调试字段，即使值为 false"
            );
        }
    }
    value["mitm"]["hosts"] = json!(["chatgpt.com", "exact.example.com"]);
    let exact = parse(value.clone()).unwrap();
    assert!(!exact.mitm_matches("chatgpt.com").unwrap());
    assert!(!exact.mitm_matches("a.chatgpt.com").unwrap());
    assert!(!exact.mitm_matches("exact.example.com").unwrap());
    value["mitm"]["bypass"] = json!([]);
    let exact_only = parse(value.clone()).unwrap();
    assert!(exact_only.mitm_matches("chatgpt.com").unwrap());
    assert!(!exact_only.mitm_matches("a.chatgpt.com").unwrap());
    value["mitm"]["hosts"] = json!(["*.chatgpt.com"]);
    let wildcard = parse(value.clone()).unwrap();
    for host in [
        "chatgpt.com",
        "a.chatgpt.com",
        "deep.a.chatgpt.com",
        "CHATGPT.COM.",
    ] {
        assert!(wildcard.mitm_matches(host).unwrap(), "{host}");
    }
    for host in ["badchatgpt.com", "chatgpt.com.evil", "chatgpt.net"] {
        assert!(!wildcard.mitm_matches(host).unwrap(), "{host}");
    }
    value["mitm"]["bypass"] = json!(["chatgpt.com"]);
    let exact_exclusion = parse(value.clone()).unwrap();
    assert!(!exact_exclusion.mitm_matches("chatgpt.com").unwrap());
    assert!(exact_exclusion.mitm_matches("a.chatgpt.com").unwrap());
    value["mitm"]["bypass"] = json!(["*.chatgpt.com"]);
    let wildcard_exclusion = parse(value.clone()).unwrap();
    for host in ["chatgpt.com", "a.chatgpt.com", "deep.a.chatgpt.com"] {
        assert!(!wildcard_exclusion.mitm_matches(host).unwrap());
    }
    value["mitm"]["hosts"] = json!(["*"]);
    value["mitm"]["bypass"] = json!([]);
    let inert_hosts = parse(value.clone()).unwrap();
    for host in ["chatgpt.com", "a.chatgpt.com", "unrelated.example"] {
        assert!(
            !inert_hosts.mitm_matches(host).unwrap(),
            "单独 * 不得开启全量解密"
        );
    }
    value["mitm"]["hosts"] = json!(["*.chatgpt.com"]);
    value["mitm"]["bypass"] = json!(["*"]);
    let inert_exclusion = parse(value.clone()).unwrap();
    for host in ["chatgpt.com", "a.chatgpt.com"] {
        assert!(
            inert_exclusion.mitm_matches(host).unwrap(),
            "单独 * 不得绕过任何域名"
        );
    }
    for broad in ["*", "*.com", "*.co.uk", "*.github.io"] {
        value["mitm"]["hosts"] = json!([broad]);
        value["mitm"]["bypass"] = json!([]);
        let inert = parse(value.clone()).unwrap();
        for host in [
            "chatgpt.com",
            "a.chatgpt.com",
            "example.co.uk",
            "tenant.github.io",
        ] {
            assert!(
                !inert.mitm_matches(host).unwrap(),
                "{broad} 不得解密 {host}"
            );
        }
        value["mitm"]["hosts"] = json!(["*.chatgpt.com"]);
        value["mitm"]["bypass"] = json!([broad]);
        let no_broad_bypass = parse(value.clone()).unwrap();
        for host in ["chatgpt.com", "a.chatgpt.com"] {
            assert!(
                no_broad_bypass.mitm_matches(host).unwrap(),
                "{broad} 不得绕过 {host}"
            );
        }
    }
    value["mitm"]["hosts"] = json!(["*", "chatgpt.com"]);
    let mixed = parse(value.clone()).unwrap();
    assert!(mixed.mitm_matches("chatgpt.com").unwrap());
    assert!(!mixed.mitm_matches("a.chatgpt.com").unwrap());
    for pattern in ["*.*", "**", "*.", "*chatgpt.com"] {
        value["mitm"]["hosts"] = json!([pattern]);
        assert!(parse(value.clone()).is_err());
        value["mitm"]["hosts"] = json!(["chatgpt.com"]);
        value["mitm"]["bypass"] = json!([pattern]);
        assert!(parse(value.clone()).is_err());
        value["mitm"]["bypass"] = json!([]);
    }
    for alias in [
        "decrypt_all",
        "decrypt-all",
        "capture-all",
        "global-decrypt",
    ] {
        let mut forbidden = value.clone();
        forbidden["mitm"][alias] = json!(true);
        assert!(
            matches!(parse(forbidden), Err(Error::ConfigUnknownField(_))),
            "正式配置不提供全量解密别名"
        );
    }
    value["mitm"]["hosts"] = json!([]);
    assert!(parse(value.clone()).is_err());
    value["mitm"]["enabled"] = json!(false);
    assert!(
        !parse(value.clone())
            .unwrap()
            .mitm_matches("chatgpt.com")
            .unwrap()
    );
    value["mitm"]["bypass"] = json!(["chat*gpt.com"]);
    assert!(parse(value).is_err());
}
#[test]
fn address_filters_fake_ip_switch_and_reserved_groups_are_bounded() {
    let mut value = input();
    value["system-proxy"] = json!({});
    value["system-proxy"]["bypass"] =
        json!(["127.0.0.1", "::1", "192.168.0.0/16", "localhost", "*.local"]);
    value["tun"] = json!({"bypass":["192.168.0.0/16","2001:db8::/32"]});
    value["proxy-groups"] = json!([]);
    assert!(parse(value.clone()).is_ok());
    value["tun"]["bypass"] = json!(["example.com"]);
    assert!(parse(value.clone()).is_err());
    value["tun"]["bypass"] = json!([]);
    value["proxy-groups"] = json!([{}]);
    assert!(matches!(parse(value.clone()), Err(Error::Unsupported)));
    value["proxy-groups"] = json!([]);
    value["dns"] = json!({"enable":true,"server":["1.1.1.1"],"fake-ip":{"enable":false,"ipv4-range":"invalid","ipv6-range":"invalid"}});
    assert!(parse(value.clone()).is_ok());
    value["dns"]["fake-ip"]["enable"] = json!(true);
    assert!(parse(value.clone()).is_err());
    value["dns"]["fake-ip"] =
        json!({"enable":true,"ipv4-range":"198.18.0.0/30","ipv6-range":"fd00::/120"});
    assert!(parse(value.clone()).is_ok());
    value["dns"]["fake-ip"]["capacity"] = json!(2);
    assert!(matches!(parse(value), Err(Error::ConfigUnknownField(_))));
}
