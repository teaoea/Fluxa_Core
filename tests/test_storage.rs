//! 测试文件：tests/test_storage.rs
//! 测试类型：单元测试、只读文件安全回归测试。
//! 测试对象：src/storage.rs、src/configuration/defaults.rs。
//! 测试内容：default_matches_host_templates_and_starts_direct、existing_file_is_read_without_mutation、missing_and_invalid_files_are_never_repaired、nonregular_and_unauthorized_paths_are_rejected、manifest_is_strict_and_bounded。
//! 测试依据：Core 只读消费者；模板仅返回内容；文件创建和持久化由宿主承担。
//! 前置条件：独立临时目录由测试宿主创建和清理，不访问共享容器或系统网络。
use super::*;
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "nodus-readonly-{}",
            crate::lifecycle::identifier().unwrap()
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}
#[test]
fn default_matches_host_templates_and_starts_direct() {
    for language in ["zh-Hans", "en-US", "fr-FR"] {
        let fixture = Fixture::new();
        let yaml = default_configuration(language).unwrap();
        assert!(!yaml.contains("proxies:"));
        assert!(!yaml.contains("proxy-groups:"));
        assert!(!yaml.contains(fixture.0.to_str().unwrap()));
        assert!(!yaml.contains("App Group"));
        assert!(yaml.contains("final,direct"));
        assert!(yaml.contains("https://github.com/teaoea/Nodus-Core/issues"));
        for key in [
            "log:",
            "system-proxy:",
            "lan-share:",
            "mode:",
            "geoip:",
            "ip-strategy:",
            "rule-providers:",
            "plugin-providers:",
            "rules:",
        ] {
            assert!(yaml.contains(key), "missing {key}");
        }
        // 主字段块之间恰好空一行；字段注释必须紧邻对应字段，不与上一块粘连。
        let lines: Vec<_> = yaml.lines().collect();
        let mut first = true;
        for (index, line) in lines.iter().enumerate() {
            if line.starts_with('#') || line.starts_with(' ') || line.is_empty() {
                continue;
            }
            assert!(line.contains(':'));
            let mut start = index;
            while start > 0 && lines[start - 1].starts_with('#') {
                start -= 1;
            }
            if !first {
                assert!(start >= 2, "missing block separator: {line}");
                assert_eq!(lines[start - 1], "", "missing blank before {line}");
                assert!(!lines[start - 2].is_empty(), "extra blank before {line}");
            }
            first = false;
        }
        assert_eq!(yaml.contains("默认配置文件"), language.starts_with("zh"));
        let summary = crate::configuration::Document::from_yaml(yaml)
            .unwrap()
            .summary();
        assert_eq!(summary.node_count, 0);
        assert_eq!(summary.rule_count, 1);
        assert!(!summary.dns_enabled && !summary.tun_requested && !summary.geoip_enabled);
        assert_eq!(summary.system_proxy_port, 0);

        assert_eq!(std::fs::read_dir(&fixture.0).unwrap().count(), 0);
    }
    assert!(default_configuration("en\0").is_err());
}
#[test]
fn existing_file_is_read_without_mutation() {
    let fixture = Fixture::new();
    let path = fixture.path("user.yaml");
    let expected = "rules: ['final,reject']\n";
    std::fs::write(&path, expected).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o400)).unwrap();
    }
    let metadata = std::fs::metadata(&path).unwrap();
    std::thread::scope(|scope| {
        let readers: Vec<_> = (0..8)
            .map(|_| {
                scope.spawn(|| {
                    assert_eq!(
                        read_configuration(path.to_str().unwrap()).unwrap().as_str(),
                        expected
                    );
                })
            })
            .collect();
        for reader in readers {
            reader.join().unwrap();
        }
    });
    assert_eq!(std::fs::read_to_string(&path).unwrap(), expected);
    assert_eq!(
        metadata.modified().unwrap(),
        std::fs::metadata(&path).unwrap().modified().unwrap()
    );
    assert_eq!(std::fs::read_dir(&fixture.0).unwrap().count(), 1);
}
#[test]
fn missing_and_invalid_files_are_never_repaired() {
    let fixture = Fixture::new();
    assert!(read_configuration(fixture.path("missing.yaml").to_str().unwrap()).is_err());
    assert_eq!(std::fs::read_dir(&fixture.0).unwrap().count(), 0);
    let path = fixture.path("invalid.yaml");
    let text = "unknown: true\n";
    std::fs::write(&path, text).unwrap();
    assert!(read_configuration(path.to_str().unwrap()).is_err());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), text);
    assert_eq!(std::fs::read_dir(&fixture.0).unwrap().count(), 1);
}
#[test]
fn nonregular_and_unauthorized_paths_are_rejected() {
    let fixture = Fixture::new();
    for path in ["config.yaml", "../config.yaml", "/tmp/../bad", "/tmp/bad\0"] {
        assert!(read_configuration(path).is_err());
    }
    assert!(read_configuration(fixture.0.to_str().unwrap()).is_err());
    #[cfg(unix)]
    {
        let path = fixture.path("config.yaml");
        std::fs::write(&path, "rules: ['final,reject']").unwrap();
        let link = fixture.path("link.yaml");
        std::os::unix::fs::symlink(&path, &link).unwrap();
        assert!(read_configuration(link.to_str().unwrap()).is_err());
    }
}
#[test]
fn manifest_is_strict_and_bounded() {
    assert!(serde_json::from_str::<Resources>(r#"{"directory":"/tmp"}"#).is_err());
    let files = Resources {
        rules: vec![NamedFile {
            name: "one".into(),
            path: "relative".into(),
        }],
        ..Default::default()
    };
    assert!(files.validate().is_err());
    let file = NamedFile {
        name: "one".into(),
        path: "/tmp/one".into(),
    };
    let files = Resources {
        rules: vec![file.clone(), file],
        ..Default::default()
    };
    assert!(files.validate().is_err());
    let files = Resources {
        plugins: (0..129)
            .map(|n| PluginFiles {
                name: format!("p{n}"),
                id: n + 1,
                path: "/tmp/p".into(),
                script: None,
            })
            .collect(),
        ..Default::default()
    };
    assert!(matches!(files.validate(), Err(Error::ResourceLimit)));
}
