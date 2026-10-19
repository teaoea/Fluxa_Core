//! 测试文件：tests/test_storage.rs
//! 测试类型：单元测试、安全回归、并发测试。
//! 测试对象：src/storage.rs、src/platform/storage.rs、src/configuration/defaults.rs。
//! 测试内容：cross_process_initialization_is_exclusive 验证多个独立进程只能创建一次；default_is_complete_localized_and_fail_closed 验证双语完整默认字段及拒绝兜底；
//! create_once_preserves_existing_yaml 验证幂等与保留自定义文件；
//! extension_reads_but_cannot_write 验证扩展只读；invalid_input_and_links_are_rejected 验证越界、损坏和链接；
//! concurrent_creation_never_overwrites 验证并发不覆盖；save_validates_before_replacing 验证失败保留。
//! 测试依据：用户 2026-10-19 example_config.yaml；无节点不得静默 DIRECT；共享容器目录和扩展只读约束。
//! 前置条件：Unix 隔离临时目录；不访问真实共享容器、不改变系统网络。
use super::*;
use crate::platform::storage::Context;
use std::path::PathBuf;
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "fluxa-storage-{}-{}",
            std::process::id(),
            crate::lifecycle::identifier().unwrap()
        ));
        std::fs::create_dir(&root).unwrap();
        Self(root)
    }
    fn context(&self, read_only: bool, language: &str) -> Context {
        Context {
            root: self.0.clone(),
            read_only_configuration: read_only,
            language: language.into(),
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}
#[test]
fn default_is_complete_localized_and_fail_closed() {
    for language in ["zh-Hans", "en-US", "fr-FR"] {
        let fixture = Fixture::new();
        assert!(initialize_in(fixture.context(false, language)).unwrap());
        for child in ["config", "log", "rule", "plugin"] {
            assert!(fixture.0.join(child).is_dir());
        }
        let text = read_in(fixture.context(true, language), "config.yaml").unwrap();
        let yaml = text.as_str();
        assert!(!yaml.contains("proxies:"));
        assert!(!yaml.contains("proxy-groups:"));
        assert!(!yaml.contains(fixture.0.to_str().unwrap()));
        assert!(!yaml.contains("App Group"));
        assert!(yaml.contains("final,reject"));
        assert!(yaml.contains("https://github.com/teaoea/fluxa-app/issues"));
        for key in [
            "log-level:",
            "system-proxy:",
            "lan-share:",
            "mode:",
            "geoip:",
            "ip-strategy:",
            "rule-providers:",
            "plugin-providers:",
            "rules:",
            "outbound-limits:",
            "dns:",
            "tun:",
            "mitm:",
            "rewrites:",
        ] {
            assert!(yaml.contains(key), "missing {key}");
        }
        for (previous, line) in yaml.lines().zip(yaml.lines().skip(1)) {
            if !line.trim_start().starts_with('#')
                && (line.trim_start().contains(": ") || line.trim_end().ends_with(':'))
            {
                assert!(previous.trim_start().starts_with('#'), "uncommented {line}");
            }
        }
        assert_eq!(yaml.contains("日志等级，"), language.starts_with("zh"));
        assert!(if language.starts_with("zh") {
            yaml.contains("GitHub：")
        } else {
            yaml.contains("GitHub:")
        });
        let summary = crate::configuration::Document::from_yaml(yaml)
            .unwrap()
            .summary();
        assert_eq!(summary.node_count, 0);
        assert_eq!(summary.rule_count, 1);
        assert!(!summary.dns_enabled && !summary.tun_requested && !summary.geoip_enabled);
        assert_eq!(summary.system_proxy_port, 0);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(fixture.0.join("config/config.yaml"))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
    }
}
#[test]
fn create_once_preserves_existing_yaml() {
    let fixture = Fixture::new();
    assert!(initialize_in(fixture.context(false, "en")).unwrap());
    let before = std::fs::read(fixture.0.join("config/config.yaml")).unwrap();
    assert!(!initialize_in(fixture.context(false, "zh")).unwrap());
    assert_eq!(
        std::fs::read(fixture.0.join("config/config.yaml")).unwrap(),
        before
    );
    std::fs::rename(
        fixture.0.join("config/config.yaml"),
        fixture.0.join("config/custom.yaml"),
    )
    .unwrap();
    assert!(!initialize_in(fixture.context(false, "en")).unwrap());
    assert!(!fixture.0.join("config/config.yaml").exists());
}
#[test]
fn extension_reads_but_cannot_write() {
    let fixture = Fixture::new();
    assert!(matches!(
        initialize_in(fixture.context(true, "en")),
        Err(Error::InvalidState)
    ));
    assert!(!fixture.0.join("config").exists());
    initialize_in(fixture.context(false, "en")).unwrap();
    let before = std::fs::read(fixture.0.join("config/config.yaml")).unwrap();
    assert!(read_in(fixture.context(true, "en"), "config.yaml").is_ok());
    assert!(matches!(
        save_in(
            fixture.context(true, "en"),
            "config.yaml",
            "rules: ['final,direct']"
        ),
        Err(Error::InvalidState)
    ));
    assert_eq!(
        std::fs::read(fixture.0.join("config/config.yaml")).unwrap(),
        before
    );
}
#[test]
fn invalid_input_and_links_are_rejected() {
    let fixture = Fixture::new();
    initialize_in(fixture.context(false, "en")).unwrap();
    for name in [
        "../config.yaml",
        "/tmp/config.yaml",
        ".hidden.yaml",
        "a\\b.yaml",
        "config.yaml\0",
    ] {
        assert!(read_in(fixture.context(true, "en"), name).is_err());
    }
    std::fs::write(fixture.0.join("config/bad.yaml"), "unknown: true").unwrap();
    assert!(read_in(fixture.context(true, "en"), "bad.yaml").is_err());
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(
            fixture.0.join("config/config.yaml"),
            fixture.0.join("config/link.yaml"),
        )
        .unwrap();
        assert!(read_in(fixture.context(true, "en"), "link.yaml").is_err());
        assert!(
            save_in(
                fixture.context(false, "en"),
                "link.yaml",
                "rules: ['final,reject']"
            )
            .is_err()
        );
    }
}
#[test]
fn concurrent_creation_never_overwrites() {
    for _ in 0..24 {
        let fixture = Fixture::new();
        std::thread::scope(|scope| {
            let tasks: Vec<_> = (0..6)
                .map(|_| scope.spawn(|| initialize_in(fixture.context(false, "en"))))
                .collect();
            let results: Vec<_> = tasks.into_iter().map(|task| task.join().unwrap()).collect();
            assert_eq!(results.iter().filter(|v| matches!(v, Ok(true))).count(), 1);
            for value in results {
                assert!(
                    matches!(value, Ok(_) | Err(Error::InvalidState)),
                    "unexpected {value:?}, diagnostics {:?}",
                    crate::error::diagnostics::snapshot()
                );
            }
        });
        assert!(read_in(fixture.context(true, "en"), "config.yaml").is_ok());
        assert!(
            std::fs::read_dir(fixture.0.join("config"))
                .unwrap()
                .all(|entry| !entry
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .ends_with(".tmp"))
        );
    }
}

#[test]
fn save_validates_before_replacing() {
    let fixture = Fixture::new();
    initialize_in(fixture.context(false, "en")).unwrap();
    let before = std::fs::read(fixture.0.join("config/config.yaml")).unwrap();
    assert!(save_in(fixture.context(false, "en"), "config.yaml", "rules: [bad]").is_err());
    assert_eq!(
        std::fs::read(fixture.0.join("config/config.yaml")).unwrap(),
        before
    );
    save_in(
        fixture.context(false, "en"),
        "config.yaml",
        "rules: ['final,reject']",
    )
    .unwrap();
    assert_eq!(
        read_in(fixture.context(true, "en"), "config.yaml")
            .unwrap()
            .as_str(),
        "rules: ['final,reject']"
    );
    let large = format!("# {}\nrules: ['final,reject']", "a".repeat(300 * 1024));
    save_in(fixture.context(false, "en"), "large.yaml", &large).unwrap();
    let text = read_in(fixture.context(true, "en"), "large.yaml").unwrap();
    assert_eq!(text.as_str(), large);
    let serialized = serde_json::to_vec(&text).unwrap();
    assert!(serialized.len() > 256 * 1024);
    assert_eq!(
        serde_json::from_slice::<String>(&serialized).unwrap(),
        large
    );
}

#[test]
fn cross_process_initialization_is_exclusive() {
    const CHILD: &str = "FLUXA_STORAGE_TEST_CHILD_ROOT";
    if let Some(root) = std::env::var_os(CHILD) {
        let result = initialize_in(Context {
            root: root.into(),
            read_only_configuration: false,
            language: "en".into(),
        });
        match result {
            Ok(true) => println!("CORE_CREATED"),
            Ok(false) | Err(Error::InvalidState) => (),
            Err(error) => panic!("unexpected child result {error:?}"),
        }
        return;
    }
    let fixture = Fixture::new();
    let children: Vec<_> = (0..6)
        .map(|_| {
            std::process::Command::new(std::env::current_exe().unwrap())
                .arg("storage::test_storage::cross_process_initialization_is_exclusive")
                .arg("--exact")
                .arg("--nocapture")
                .env(CHILD, &fixture.0)
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .unwrap()
        })
        .collect();
    let mut created = 0;
    for child in children {
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "{} {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        created += String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter(|line| *line == "CORE_CREATED")
            .count();
    }
    assert_eq!(created, 1);
    assert!(read_in(fixture.context(true, "en"), "config.yaml").is_ok());
}
