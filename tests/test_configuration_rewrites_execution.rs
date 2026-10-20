//! 测试文件：tests/test_configuration_rewrites_execution.rs
//! 测试类型：单元测试、条件/动作与安全边界回归。
//! 测试对象：src/configuration/rewrites.rs、src/configuration/rewrites/conditions.rs、src/configuration/rewrites/execution.rs、src/configuration/rewrites/presets.rs。
//! 测试内容：request_conditions_are_and、headers_and_query_have_distinct_case_rules、response_waits_for_original_status_and_mime、length_uses_actual_body、body_conditions_are_explicit、disabled_rules_and_unbound_proxy_are_skipped、phase_and_unknown_fields_are_rejected、request_modification_preserves_origin、response_modification_clears_stale_metadata、redirect_and_abort_are_distinct、first_match_has_priority、script_condition_does_not_fake_execution、media_presets_are_real_bytes、proxy_reference_is_checked、direct_preserves_debug_precedence、mp4_declares_zero_milliseconds_without_audio。
//! 测试依据：用户 2026-10-20 新配置注释、when 图片及 proxy/proxy(节点名称) 补充。
//! 前置条件：纯内存，无系统 VPN、文件缓存、JS 工作线程或外网操作。
use super::*;
use serde_json::{Value, json};
fn document(rules: Value) -> Document {
    Document::from_yaml(&json!({"rules":["final,reject"],"rewrites":rules}).to_string()).unwrap()
}
fn message<'a>(
    method: &'a str,
    url: &'a str,
    headers: &'a [Header],
    body: &'a [u8],
) -> Message<'a> {
    Message {
        method: method.into(),
        url: url.into(),
        headers: headers.into(),
        body: body.into(),
    }
}
fn h(name: &str, value: &str) -> Header {
    Header {
        name: name.into(),
        value: value.into(),
    }
}
fn rejection(when: Value) -> Document {
    document(
        json!([{"url":"https://example.com/api/**","when":when,"action":"reject","status":404}]),
    )
}
fn rejected(doc: &Document, req: &Message<'_>) -> bool {
    matches!(
        doc.rewrite_request(req, &mut req.clone()).unwrap(),
        Outcome::Response(Response { status: 404, .. })
    )
}
#[test]
fn request_conditions_are_and() {
    let doc = rejection(
        json!({"method":"GET","request-headers":{"exists":["User-Agent","X-Client-ID"],"equals":{"X-Client-Type":"mobile"}},"query":{"exists":["ad_id"],"equals":{"source":"app"}}}),
    );
    let headers = [
        h("user-agent", "x"),
        h("X-Client-ID", "1"),
        h("X-Client-Type", "mobile"),
    ];
    let url = "https://example.com/api/a?ad_id=&source=app";
    assert!(rejected(&doc, &message("GET", url, &headers, &[])));
    assert!(!rejected(&doc, &message("POST", url, &headers, &[])));
    assert!(!rejected(&doc, &message("GET", url, &headers[..2], &[])));
    assert!(!rejected(
        &doc,
        &message("GET", "https://example.com/api/a?source=app", &headers, &[])
    ));
    assert!(!rejected(
        &doc,
        &message(
            "GET",
            "https://example.com/api/a?ad_id=1&source=web",
            &headers,
            &[]
        )
    ));
}
#[test]
fn headers_and_query_have_distinct_case_rules() {
    let doc = rejection(
        json!({"request-headers":{"equals":{"X-Test":"Mobile"}},"query":{"equals":{"source":"a b"}}}),
    );
    assert!(rejected(
        &doc,
        &message(
            "GET",
            "https://example.com/api/a?source=a+b",
            &[h("x-test", "Mobile")],
            &[]
        )
    ));
    assert!(!rejected(
        &doc,
        &message(
            "GET",
            "https://example.com/api/a?Source=a+b",
            &[h("x-test", "Mobile")],
            &[]
        )
    ));
    assert!(!rejected(
        &doc,
        &message(
            "GET",
            "https://example.com/api/a?source=a+b",
            &[h("x-test", "mobile")],
            &[]
        )
    ));
    assert!(rejected(
        &doc,
        &message(
            "GET",
            "https://example.com/api/a?source=bad&source=a%20b",
            &[h("X-Test", "Mobile")],
            &[]
        )
    ));
}
#[test]
fn response_waits_for_original_status_and_mime() {
    let doc = document(
        json!([{"url":"https://example.com/images/**","phase":"response","when":{"status":200,"response-headers":{"exists":["Content-Type"],"equals":{"X-Resource-Type":"image"}},"content-type":"image/png"},"action":"modify-response","response":{"status":201,"body":{"preset":"transparent-png"}}}]),
    );
    let req = message("GET", "https://example.com/images/a", &[], &[]);
    assert!(matches!(
        doc.rewrite_request(&req, &mut req.clone()).unwrap(),
        Outcome::Continue
    ));
    for (status, mime, changed) in [
        (200, "IMAGE/PNG; charset=binary", true),
        (403, "image/png", false),
        (200, "image/jpeg", false),
    ] {
        let mut value = response(
            status,
            vec![h("Content-Type", mime), h("X-Resource-Type", "image")],
            b"original".to_vec(),
        );
        doc.rewrite_response(&req, &mut value).unwrap();
        assert_eq!(value.status, if changed { 201 } else { status });
        assert_eq!(
            value.body.as_slice(),
            if changed {
                super::super::presets::PNG
            } else {
                b"original"
            }
        );
    }
}
#[test]
fn length_uses_actual_body() {
    let doc = rejection(json!({"content-length":{"min":2,"max":3}}));
    assert!(rejected(
        &doc,
        &message(
            "POST",
            "https://example.com/api/a",
            &[h("Content-Length", "900")],
            b"ok"
        )
    ));
    assert!(!rejected(
        &doc,
        &message(
            "POST",
            "https://example.com/api/a",
            &[h("Content-Length", "2")],
            b"long"
        )
    ));
}
#[test]
fn body_conditions_are_explicit() {
    let doc = rejection(json!({"request-body":{"contains":"ad","equals":"ad=yes"}}));
    assert!(rejected(
        &doc,
        &message("POST", "https://example.com/api/a", &[], b"ad=yes")
    ));
    assert!(!rejected(
        &doc,
        &message("POST", "https://example.com/api/a", &[], b"ad=no")
    ));
    assert!(!rejected(
        &doc,
        &message("POST", "https://example.com/api/a", &[], &[255])
    ));
    let encoded_headers = [h("Content-Encoding", "gzip")];
    let req = message(
        "POST",
        "https://example.com/api/a",
        &encoded_headers,
        b"ad=yes",
    );
    assert!(matches!(
        doc.rewrite_request(&req, &mut req.clone()),
        Err(Error::Unsupported)
    ));
}
#[test]
fn disabled_rules_and_unbound_proxy_are_skipped() {
    for action in ["proxy", "proxy()"] {
        let doc = document(
            json!([{"url":"https://example.com/**","action":"reject","status":500,"enable":false},{"url":"https://example.com/**","action":action},{"url":"https://example.com/**","action":"reject","status":404}]),
        );
        assert!(rejected(
            &doc,
            &message("GET", "https://example.com/api/a", &[], &[])
        ));
    }
}
#[test]
fn phase_and_unknown_fields_are_rejected() {
    for rule in [
        json!({"url":"https://example.com/","phase":"response","action":"reject","status":404}),
        json!({"url":"https://example.com/","phase":"request","action":"modify-response","response":{"status":200}}),
        json!({"url":"https://example.com/","action":"reject","status":404,"when":{"status":200}}),
        json!({"url":"https://example.com/","action":"reject","status":404,"when":{}}),
        json!({"url":"https://example.com/","action":"reject","status":404,"when":{"unknown":true}}),
        json!({"url":"https://example.com/","action":"reject","status":404,"when":{"method":null}}),
        json!({"url":"https://example.com/","action":"reject","status":404,"when":{"content-length":{"min":9,"max":1}}}),
        json!({"url":"https://example.com/","action":"reject","status":404,"when":{"request-headers":{"exists":["X","x"]}}}),
        json!({"url":"https://example.com/","action":"response","response":{"body":{"text":"x"}}}),
    ] {
        assert!(
            Document::from_yaml(&json!({"rules":["final,reject"],"rewrites":[rule]}).to_string())
                .is_err()
        );
    }
}
#[test]
fn request_modification_preserves_origin() {
    let doc = document(
        json!([{"url":"https://example.com/api/**","action":"modify-request","request":{"method":"POST","headers":{"set":{"X-New":"ok"},"remove":["X-Old"]},"query":{"set":{"source":"a b"},"remove":["ad"]},"body":{"text":"new"}}}]),
    );
    let headers = [h("X-Old", "yes"), h("Content-Encoding", "gzip")];
    let req = message(
        "GET",
        "https://example.com/api/a?ad=1&keep=2",
        &headers,
        b"old",
    );
    let mut out = req.clone();
    doc.rewrite_request(&req, &mut out).unwrap();
    assert_eq!(out.method, "POST");
    assert_eq!(out.url, "https://example.com/api/a?keep=2&source=a+b");
    assert_eq!(out.body.as_ref(), b"new");
    assert_eq!(out.headers.len(), 1);
    assert_eq!(out.headers[0].name, "X-New");
    assert_eq!(req.method, "GET");
    assert_eq!(req.body.as_ref(), b"old");
}
#[test]
fn response_modification_clears_stale_metadata() {
    let doc = document(
        json!([{"url":"https://example.com/**","action":"modify-response","response":{"body":{"text":"new"}}}]),
    );
    let mut value = response(
        206,
        vec![
            h("Content-Length", "999"),
            h("Content-Encoding", "gzip"),
            h("ETag", "old"),
            h("Content-Range", "bytes 1-9/99"),
            h("X-Keep", "yes"),
        ],
        b"old".to_vec(),
    );
    doc.rewrite_response(
        &message("GET", "https://example.com/", &[], &[]),
        &mut value,
    )
    .unwrap();
    assert_eq!(value.body, b"new");
    assert_eq!(value.headers.len(), 1);
    assert_eq!(value.headers[0].name, "X-Keep");
}
#[test]
fn redirect_and_abort_are_distinct() {
    let req = message("GET", "https://example.com/", &[], &[]);
    let doc = document(
        json!([{"url":"https://example.com/","action":"redirect","status":307,"location":"https://example.org/new"}]),
    );
    match doc.rewrite_request(&req, &mut req.clone()).unwrap() {
        Outcome::Response(r) => {
            assert_eq!(r.status, 307);
            assert_eq!(r.headers[0].value, "https://example.org/new");
        }
        _ => panic!("必须构造重定向"),
    }
    let doc = document(json!([{"url":"https://example.com/","action":"abort"}]));
    assert!(matches!(
        doc.rewrite_request(&req, &mut req.clone()),
        Err(Error::Cancelled)
    ));
}
#[test]
fn first_match_has_priority() {
    let doc = document(
        json!([{"url":"https://example.com/**","action":"reject","status":404},{"url":"https://example.com/**","action":"reject","status":500}]),
    );
    assert!(rejected(
        &doc,
        &message("GET", "https://example.com/api/a", &[], &[])
    ));
}
#[test]
fn script_condition_does_not_fake_execution() {
    let doc = document(
        json!([{"url":"https://example.com/**","phase":"response","when":{"status":403},"action":"script","js":"response.js"}]),
    );
    let req = message("GET", "https://example.com/", &[], &[]);
    assert!(matches!(
        doc.rewrite_request(&req, &mut req.clone()).unwrap(),
        Outcome::Continue
    ));
    let mut ok = response(200, vec![], vec![]);
    doc.rewrite_response(&req, &mut ok).unwrap();
    let mut forbidden = response(403, vec![], vec![]);
    assert!(matches!(
        doc.rewrite_response(&req, &mut forbidden),
        Err(Error::Unsupported)
    ));
}
#[test]
fn media_presets_are_real_bytes() {
    for (preset, mime, signature) in [
        (
            "transparent-png",
            "image/png",
            b"\x89PNG\r\n\x1a\n".as_slice(),
        ),
        ("minimal-mp4", "video/mp4", b"ftyp".as_slice()),
    ] {
        let doc = document(
            json!([{"url":"https://example.com/**","action":"respond","response":{"status":200,"body":{"preset":preset}}}]),
        );
        match doc
            .rewrite_request(
                &message("GET", "https://example.com/", &[], &[]),
                &mut message("GET", "https://example.com/", &[], &[]),
            )
            .unwrap()
        {
            Outcome::Response(r) => {
                assert!(r.body.windows(signature.len()).any(|w| w == signature));
                assert_eq!(r.headers[0].value, mime);
                assert!(r.body.len() < 2048);
            }
            _ => panic!("预置响应"),
        }
    }
}
#[test]
fn proxy_reference_is_checked() {
    let mut config = json!({"rules":["final,reject"],"rewrites":[{"url":"https://example.com/**","action":"proxy(Missing)"}]});
    assert!(matches!(
        Document::from_yaml(&config.to_string()),
        Err(Error::InvalidInput)
    ));
    config["rewrites"][0]["action"] = json!("proxy");
    assert!(Document::from_yaml(&config.to_string()).is_ok());
}
#[test]
fn direct_preserves_debug_precedence() {
    use crate::routing::debug::{DebugRule, Session};
    use crate::routing::{Config, Match, Policy, Router, Rule as RoutingRule, Target, Transport};
    let session = Session::new().unwrap();
    session
        .replace(
            vec![DebugRule {
                rule: RoutingRule {
                    protocol: None,
                    r#type: Match::Domain,
                    value: "example.com".into(),
                    policy: Policy::Reject,
                    outbound: None,
                },
                protocol: None,
                lifetime_seconds: Some(60),
            }],
            None,
        )
        .unwrap();
    let router = Router::compile(Config::default())
        .unwrap()
        .with_debug(&session)
        .unwrap()
        .for_http_rewrite(Policy::Direct, None)
        .unwrap();
    let target = Target {
        domain: Some("example.com".into()),
        address: None,
        port: 443,
        transport: Transport::Tcp,
    };
    assert_eq!(
        router.evaluate_flow(target, None, None).unwrap().policy,
        Policy::Reject
    );
}

#[test]
fn mp4_declares_zero_milliseconds_without_audio() {
    fn boxes(bytes: &[u8], found: &mut Vec<([u8; 4], Vec<u8>)>) {
        let mut offset = 0;
        while offset < bytes.len() {
            assert!(bytes.len() - offset >= 8);
            let size = u32::from_be_bytes(bytes[offset..offset + 4].try_into().unwrap()) as usize;
            let kind: [u8; 4] = bytes[offset + 4..offset + 8].try_into().unwrap();
            let (size, header) = if size == 1 {
                (
                    u64::from_be_bytes(bytes[offset + 8..offset + 16].try_into().unwrap()) as usize,
                    16,
                )
            } else {
                (size, 8)
            };
            assert!(size >= header && offset + size <= bytes.len());
            let body = &bytes[offset + header..offset + size];
            if [*b"moov", *b"trak", *b"mdia", *b"minf", *b"stbl"].contains(&kind) {
                boxes(body, found);
            }
            found.push((kind, body.to_vec()));
            offset += size;
        }
    }
    let mut found = Vec::new();
    boxes(super::super::presets::MP4, &mut found);
    for kind in [*b"mvhd", *b"mdhd"] {
        let body = &found.iter().find(|(k, _)| *k == kind).unwrap().1;
        assert_eq!(body[0], 0);
        assert_eq!(u32::from_be_bytes(body[16..20].try_into().unwrap()), 0);
    }
    let headers: Vec<_> = found.iter().filter(|(k, _)| k == b"hdlr").collect();
    assert_eq!(headers.len(), 1);
    assert_eq!(&headers[0].1[8..12], b"vide");
    let timing = &found.iter().find(|(k, _)| k == b"stts").unwrap().1;
    assert_eq!(u32::from_be_bytes(timing[12..16].try_into().unwrap()), 0);
}
