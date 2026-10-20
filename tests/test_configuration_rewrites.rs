//! - script_mapping_and_extensionless_reference_are_valid：验证本轮已登记契约。
//! - ttl_belongs_to_rewrite_and_defaults_to_one_hour：验证本轮已登记契约。
//!
//! 测试文件：tests/test_configuration_rewrites.rs
//! 测试类型：单元测试、主配置字段契约与安全回归。
//! 测试对象：src/configuration/rewrites.rs、src/configuration/models.rs、src/configuration.rs。
//! 测试内容：example_actions_are_accepted 验证新示例九类声明；action_fields_are_exclusive 验证跨动作字段拒绝；
//! required_fields_and_types_are_strict 验证缺失/错误类型；nested_unknown_and_duplicate_fields_are_rejected 验证嵌套严格性；
//! header_method_and_query_changes_are_validated 验证头、方法及查询冲突；body_presets_and_bounds_are_strict 验证正文组合及预算；
//! script_references_are_filenames_not_source_or_paths 验证脚本引用隔离；url_and_status_boundaries_are_explicit 验证地址与状态码；
//! validation_never_activates_unimplemented_rewrites 验证只接受声明、不伪造执行；complete_user_example_is_accepted 验证用户完整示例。
//! loading_compiles_url_patterns 验证加载时保存编译结果并拒绝非法通配符。
//! 测试依据：用户 2026-10-20 example_config.yaml 及本轮任务 1–3 的字段与加载编译要求。
//! 前置条件：无网络、真实共享容器、JS 读取、系统代理或 VPN 副作用。
use super::*;
use crate::configuration::Document;
use serde_json::{Value, json};
fn parse(rule: Value) -> Result<Document> {
    Document::from_yaml(&json!({"rules":["final,reject"],"rewrites":[rule]}).to_string())
}
#[test]
fn example_actions_are_accepted() {
    for rule in [
        json!({"url":"https://example.com/ads/*","action":"reject","status":404}),
        json!({"url":"https://example.com/images/**","action":"modify-response","response":{"body":{"preset":"transparent-png"}}}),
        json!({"url":"https://example.com/video/**","action":"modify-response","response":{"body":{"preset":"minimal-mp4"}}}),
        json!({"url":"https://example.com/api/*","action":"modify-request","request":{"headers":{"set":{"X-Custom-Header":"AAA"},"remove":["X-Tracking-ID"]}}}),
        json!({"url":"https://example.com/api/submit","action":"modify-request","request":{"method":"POST"}}),
        json!({"url":"https://example.com/api/**","action":"modify-request","request":{"query":{"set":{"source":"fluxa"},"remove":["tracking_id"]}}}),
        json!({"url":"https://example.com/api/data","action":"modify-request","request":{"body":{"text":"{\"enabled\":true}"},"headers":{"set":{"Content-Type":"application/json"}}}}),
        json!({"url":"https://example.com/api/request","action":"script","phase":"request","js":"request.js"}),
        json!({"url":"https://example.com/api/user","action":"script","phase":"response","js":"response.js"}),
    ] {
        let document = parse(rule).unwrap();
        assert_eq!(document.summary().rule_count, 1);
        assert_eq!(document.raw.rewrites.len(), 1);
    }
}
#[test]
fn action_fields_are_exclusive() {
    for (action, extra) in [
        (
            json!({"url":"https://example.com/","action":"reject","status":404}),
            json!({"phase":"response"}),
        ),
        (
            json!({"url":"https://example.com/","action":"modify-response","response":{"body":{"preset":"transparent-png"}}}),
            json!({"request":{"method":"POST"}}),
        ),
        (
            json!({"url":"https://example.com/","action":"modify-request","request":{"method":"GET"}}),
            json!({"status":404}),
        ),
        (
            json!({"url":"https://example.com/","action":"script","phase":"request","js":"request.js"}),
            json!({"response":{"body":{"text":"x"}}}),
        ),
    ] {
        let mut action = action;
        action
            .as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        assert!(parse(action).is_err());
    }
    // 主配置不再把旧的 host/path_prefix 解释为新 url 规则。
    assert!(parse(json!({"host":"example.com","path_prefix":"/","action":"reject"})).is_err());
}
#[test]
fn required_fields_and_types_are_strict() {
    for rule in [
        json!({"url":"https://example.com/","action":"reject"}),
        json!({"url":"https://example.com/","action":"reject","status":"404"}),
        json!({"action":"reject","status":404}),
        json!({"url":"https://example.com/","action":"modify-response"}),
        json!({"url":"https://example.com/","action":"modify-request","request":{}}),
        json!({"url":"https://example.com/","action":"modify-request","request":{"method":123}}),
        json!({"url":"https://example.com/","action":"modify-request","request":{"method":"POST","headers":null}}),
        json!({"url":"https://example.com/","action":"script","js":"x.js"}),
        json!({"url":"https://example.com/","action":"script","phase":"both","js":"x.js"}),
        json!({"url":"https://example.com/","action":"script","phase":"request","js":null}),
    ] {
        assert!(parse(rule.clone()).is_err(), "accepted {rule}");
    }
}
#[test]
fn nested_unknown_and_duplicate_fields_are_rejected() {
    for rule in [
        json!({"url":"https://example.com/","action":"modify-request","request":{"method":"GET","unknown":true}}),
        json!({"url":"https://example.com/","action":"modify-request","request":{"headers":{"set":{"X-Test":"a"},"append":[]}}}),
        json!({"url":"https://example.com/","action":"modify-response","response":{"body":{"preset":"transparent-png","source":"secret"}}}),
    ] {
        assert!(matches!(parse(rule), Err(Error::ConfigUnknownField(_))));
    }
    for rule in [
        "{url: 'https://example.com/', action: reject, status: 404, status: 500}",
        "{url: 'https://example.com/', action: modify-request, request: {body: {text: a, text: b}}}",
        "{url: 'https://example.com/', action: modify-request, request: {headers: {set: {X-Test: a, X-Test: b}}}}",
    ] {
        assert!(matches!(
            Document::from_yaml(&format!("rules: ['final,reject']\nrewrites: [{rule}]")),
            Err(Error::ConfigDuplicateField(_))
        ));
    }
}
#[test]
fn header_method_and_query_changes_are_validated() {
    for request in [
        json!({"method":""}),
        json!({"method":"GE T"}),
        json!({"headers":{"set":{"Host":"other.example"}}}),
        json!({"headers":{"remove":["Content-Length"]}}),
        json!({"headers":{"set":{"X-Test":"ok\r\nInjected: value"}}}),
        json!({"headers":{"set":{"X-Test":"a","x-test":"b"}}}),
        json!({"headers":{"set":{"X-Test":"a"},"remove":["x-test"]}}),
        json!({"query":{"set":{"source":"a"},"remove":["source"]}}),
        json!({"query":{"remove":["x","x"]}}),
        json!({"headers":{}}),
        json!({"query":{}}),
    ] {
        assert!(
            parse(
                json!({"url":"https://example.com/","action":"modify-request","request":request})
            )
            .is_err()
        );
    }
    // 查询参数区分大小写；空正文和空参数值均有明确意义。
    assert!(parse(json!({"url":"https://example.com/","action":"modify-request","request":{"method":"POST","body":{"text":""},"query":{"set":{"Source":"","source":"x"}}}})).is_ok());
}
#[test]
fn body_presets_and_bounds_are_strict() {
    for body in [
        json!({}),
        json!({"preset":"unknown"}),
        json!({"text":"x","preset":"transparent-png"}),
        json!({"text":null,"preset":"transparent-png"}),
    ] {
        assert!(
            parse(
                json!({"url":"https://example.com/","action":"modify-response","response":{"body":body}})
            )
            .is_err()
        );
    }
    assert!(parse(json!({"url":"https://example.com/","action":"modify-request","request":{"body":{"preset":"minimal-mp4"}}})).is_err());
    let mut oversized = json!({"url":"https://example.com/","action":"modify-response","response":{"body":{"text":"x"}}});
    oversized["response"]["body"]["text"] = json!("x".repeat(1024 * 1024 + 1));
    assert!(matches!(parse(oversized), Err(Error::ResourceLimit)));
}
#[test]
fn script_references_are_filenames_not_source_or_paths() {
    for name in [
        "",
        "../request.js",
        "/tmp/request.js",
        "plugin/1/a.js",
        "https://example.com/a.js",
        "file:///a.js",
        "a\\b.js",
        ".hidden.js",
        "a.js\0",
        "input => input",
        ".js",
    ] {
        assert!(
            parse(
                json!({"url":"https://example.com/","action":"script","phase":"request","js":name})
            )
            .is_err(),
            "accepted {name:?}"
        );
    }
    assert!(parse(json!({"url":"https://example.com/","action":"script","phase":"response","js":"response-v2.test.js"})).is_ok());
}
#[test]
fn url_and_status_boundaries_are_explicit() {
    for url in [
        "",
        "/api/*",
        "ftp://example.com/",
        "https://u:p@example.com/",
        "https://example.com/#fragment",
        "https://example.com/with space",
        "https://example.com/\n",
    ] {
        assert!(parse(json!({"url":url,"action":"reject","status":404})).is_err());
    }
    for (status, accepted) in [
        (199, false),
        (200, true),
        (404, true),
        (599, true),
        (600, false),
    ] {
        assert_eq!(
            parse(json!({"url":"http://example.com/path/**","action":"reject","status":status}))
                .is_ok(),
            accepted
        );
    }
}
#[test]
fn validation_never_activates_unimplemented_rewrites() {
    let document = parse(
        json!({"url":"https://example.com/","action":"script","phase":"request","js":"missing.js"}),
    )
    .unwrap();
    // 未注册资源按错误排除；不能执行脚本，也不能遮挡下一条有效规则。
    let prepared = document.prepare().unwrap();
    assert!(!prepared.scripts.contains("missing.js", 3600));
}
#[test]
fn complete_user_example_is_accepted() {
    assert!(Document::from_yaml(include_str!("../example_config.yaml")).is_ok());
}

#[test]
fn loading_compiles_url_patterns() {
    let document =
        parse(json!({"url":"https://example.com/api/*","action":"reject","status":404})).unwrap();
    let compiled = document.raw.rewrites[0]
        .wire
        .common()
        .compiled_url
        .as_ref()
        .expect("加载必须完成编译");
    assert!(compiled.matches("https://example.com/api/a").unwrap());
    assert!(!compiled.matches("https://example.com/api/a/b").unwrap());
    assert!(
        parse(json!({"url":"https://example.com/api/a*b","action":"reject","status":404})).is_err()
    );
    assert!(parse(json!({"url":"https://example.com/api/*","action":"reject","status":404,"compiled_url":{}})).is_err());
}

#[test]
fn script_mapping_and_extensionless_reference_are_valid() {
    let yaml = "rules: ['final,reject']\nscripts:\n  name: response\n  url: https://example.com/response.js\nrewrites:\n  - url: https://example.com/api/**\n    phase: response\n    action: script\n    js: response\n    js-ttl: 3600\n";
    let document = Document::from_yaml(yaml).unwrap();
    assert_eq!(document.raw.scripts.as_ref().unwrap().name, "response");
    assert_eq!(
        document.raw.rewrites[0].script_reference(),
        Some(("response".into(), 3600))
    );
    for invalid in [
        "{}",
        "{url: 'https://example.com/response.js'}",
        "[{name: response}]",
        "null",
        "{name: response, name: other}",
        "{name: '../response'}",
    ] {
        assert!(
            Document::from_yaml(&format!("rules: ['final,reject']\nscripts: {invalid}\n")).is_err(),
            "accepted {invalid}"
        );
    }
    // URL 的扩展名不约束引用键；不再把任意名称强制解释成 .js 文件。
    for name in ["response", "response.js", "a.json"] {
        assert!(parse(json!({"url":"https://example.com/", "phase":"request", "action":"script", "js":name})).is_ok());
    }
}
#[test]
fn ttl_belongs_to_rewrite_and_defaults_to_one_hour() {
    let rule = json!({"url":"https://example.com/", "phase":"response", "action":"script", "js":"response"});
    let doc = parse(rule.clone()).unwrap();
    assert_eq!(
        doc.raw.rewrites[0].script_reference(),
        Some(("response".into(), 3600))
    );
    for ttl in [json!(0), json!(-1), json!("3600"), json!(null)] {
        let mut value = rule.clone();
        value["js-ttl"] = ttl;
        assert!(parse(value).is_err());
    }
    assert!(
        parse(json!({"url":"https://example.com/", "action":"reject", "status":403, "js-ttl":1}))
            .is_err()
    );
}
