//! 测试文件：tests/test_configuration_check.rs
//! 测试类型：单元测试、只读字段定位与错误协议。
//! 测试对象：src/configuration/check.rs、src/configuration.rs、src/routing/yaml.rs。
//! 测试内容：mixed_provider_defaults_and_error_position、semantic_fields_have_exact_lines、strict_decode_reports_nested_and_duplicate_fields、unicode_and_crlf_offsets_match_editor_units、file_errors_never_have_fake_lines_or_repair、checking_does_not_apply_or_change_error_codes。
//! 测试依据：字段错误、固定代码、源文件逻辑行与前端 UTF-16 自动换行定位；Core 不写入。
//! 前置条件：临时文件由测试宿主准备，无网络。
use super::*;
#[test]
fn semantic_fields_have_exact_lines() {
    for (yaml, field, line) in [
        (
            "# header\n\nsystem-proxy:\n  port: 80\nrules: ['final,direct']",
            "system-proxy.port",
            4,
        ),
        (
            "rules:\n  - domain,good.example,direct\n  - final,missing-node",
            "rules[1]",
            3,
        ),
        (
            "geoip:\n  enabled: true\n  url: http://example.com/a\nrules: ['final,direct']",
            "geoip.url",
            3,
        ),
        (
            "rule-providers:\n  - name: one\n    url: https://example.com/a\n    enable: true\n    policy: missing-node\nrules: ['final,direct']",
            "rule-providers[0].policy",
            5,
        ),
    ] {
        let issue = check_yaml(yaml).unwrap_err();
        assert_eq!(issue.field, field);
        assert_eq!(issue.position.unwrap().line, line);
        assert!(Document::from_yaml(yaml).is_err());
    }
}
#[test]
fn strict_decode_reports_nested_and_duplicate_fields() {
    for (yaml, field, line, code) in [
        (
            "system-proxy:\n  port: wrong\nrules: ['final,direct']",
            "system-proxy.port",
            2,
            1001,
        ),
        (
            "system-proxy:\n  unexpected: true\nrules: ['final,direct']",
            "system-proxy.unexpected",
            2,
            2002,
        ),
        (
            "system-proxy:\n  port: 0\n  port: 0\nrules: ['final,direct']",
            "system-proxy.port",
            3,
            2003,
        ),
        ("mode: unknown\nrules: ['final,direct']", "mode", 1, 1001),
    ] {
        let issue = check_yaml(yaml).unwrap_err();
        assert_eq!(issue.field, field);
        assert_eq!(issue.error_info.code.value(), code);
        assert_eq!(issue.position.unwrap().line, line);
    }
    let issue = check_yaml("rules: [\n").unwrap_err();
    assert_eq!(issue.error_info.code.value(), 2001);
    assert!(issue.position.unwrap().line >= 1);
}
#[test]
fn unicode_and_crlf_offsets_match_editor_units() {
    let yaml = "# 中文 😀\r\n\r\nsystem-proxy: {port: wrong}\r\nrules: ['final,direct']\r\n";
    let issue = check_yaml(yaml).unwrap_err();
    let position = issue.position.unwrap();
    assert_eq!(position.line, 3);
    let expected = yaml.find("wrong").unwrap();
    assert_eq!(
        position.utf16_offset,
        yaml[..expected].encode_utf16().count()
    );
    assert_eq!(position.column, "system-proxy: {port: ".chars().count() + 1);
    // 逻辑行中的长内容不会因窗口宽度产生新的文件行；前端依偏移映射实际显示行。
    let same_line = "system-proxy: {port: wrong}";
    assert_eq!(check_yaml(same_line).unwrap_err().position.unwrap().line, 1);
}
#[test]
fn file_errors_never_have_fake_lines_or_repair() {
    let path = std::env::temp_dir().join(format!(
        "nodus-check-{}-{}.yaml",
        std::process::id(),
        crate::lifecycle::identifier().unwrap()
    ));
    let missing = check_file(path.to_str().unwrap()).unwrap_err();
    assert_eq!(missing.field, "$file");
    assert!(missing.position.is_none());
    assert!(!path.exists());
    let yaml = "rules: ['final,missing']";
    std::fs::write(&path, yaml).unwrap();
    let issue = check_file(path.to_str().unwrap()).unwrap_err();
    assert_eq!(issue.field, "rules[0]");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), yaml);
    std::fs::remove_file(path).unwrap();
}
#[test]
fn checking_does_not_apply_or_change_error_codes() {
    let manager = crate::outbound::Manager::default();
    let before = manager.network_snapshot().unwrap().revision;
    assert_eq!(check_yaml("rules: ['final,direct']").unwrap().rule_count, 1);
    assert_eq!(manager.network_snapshot().unwrap().revision, before);
    let yaml = "lan-share: {password: secret, account: ''}\nrules: ['final,direct']";
    let issue = check_yaml(yaml).unwrap_err();
    assert_eq!(
        issue.error_info.code,
        Document::from_yaml(yaml).err().unwrap().code()
    );
    let json = serde_json::to_string(&issue).unwrap();
    assert!(!json.contains("secret"));
}

#[test]
fn mixed_provider_defaults_and_error_position() {
    let yaml = "rule-providers:\n  - name: remote\n    url: https://example.com/rules.yaml\n    policy: direct,reject\nrules: ['final,direct']";
    let document = Document::from_yaml(yaml).unwrap();
    assert!(document.raw.rule_providers[0].enable);
    assert!(check_yaml(yaml).is_ok());
    let wrong = yaml.replace("direct,reject", "missing,reject");
    let error = check_yaml(&wrong).unwrap_err();
    assert_eq!(error.field, "rule-providers[0].policy");
    assert_eq!(error.position.unwrap().line, 4);
    let local = "# 中文 😀\nrules:\n  - final,(missing,reject)";
    let error = check_yaml(local).unwrap_err();
    assert_eq!(error.field, "rules[0]");
    assert_eq!(error.position.unwrap().line, 3);
}
