//! 测试文件：tests/test_configuration_rewrites_script_message.rs
//! 测试类型：单元测试、JS 输出协议安全回归。
//! 测试对象：src/configuration/rewrites/script_message.rs。
//! 测试内容：request_patch_cannot_change_origin_or_protected_headers、response_patch_removes_stale_body_metadata、response_patch_cannot_mutate_original_request、compressed_binary_and_oversized_input_are_rejected。
//! 测试依据：JS 仅变更有界消息数据，不能取得路由、认证、分帧或宿主资源控制权。
//! 前置条件：纯内存，无网络、共享容器、系统代理或 VPN 副作用。
use super::*;
fn request() -> Message<'static> {
    Message {
        method: Cow::Borrowed("GET"),
        url: Cow::Borrowed("https://example.com/a"),
        headers: Cow::Owned(vec![]),
        body: Cow::Borrowed(b"old"),
    }
}
fn response() -> Response {
    Response {
        status: 200,
        headers: vec![
            Header {
                name: "Content-Length".into(),
                value: "3".into(),
            },
            Header {
                name: "ETag".into(),
                value: "old".into(),
            },
        ],
        body: b"old".to_vec(),
        duration_ms: 0,
    }
}
#[test]
fn request_patch_cannot_change_origin_or_protected_headers() {
    for value in [
        json!({"url":"https://other.example/a"}),
        json!({"url":"https://user:secret@example.com/a"}),
        json!({"method":"CONNECT"}),
        json!({"headers":[{"name":"Host","value":"other.example"}]}),
        json!({"headers":[{"name":"Content-Length","value":"3"}]}),
        json!({"headers":[{"name":"X-Test","value":"a\r\nInjected: yes"}]}),
        json!({"route":"direct"}),
        json!({"status":200}),
    ] {
        assert!(apply_request(value, &mut request()).is_err());
    }
    let mut request = request();
    apply_request(
        json!({"method":"POST","url":"https://example.com/b?q=1","body":"new"}),
        &mut request,
    )
    .unwrap();
    assert_eq!(request.method, "POST");
    assert_eq!(request.url, "https://example.com/b?q=1");
    assert_eq!(request.body.as_ref(), b"new");
}
#[test]
fn response_patch_removes_stale_body_metadata() {
    let mut response = response();
    apply_response(
        json!({"status":201,"body":"new"}),
        &request(),
        &mut response,
    )
    .unwrap();
    assert_eq!(response.status, 201);
    assert_eq!(response.body, b"new");
    assert!(response.headers.is_empty());
    apply_response(json!({"status":204}), &request(), &mut response).unwrap();
    assert!(response.body.is_empty());
}
#[test]
fn response_patch_cannot_mutate_original_request() {
    for value in [
        json!({"request":{"method":"POST"}}),
        json!({"status":199}),
        json!({"method":"POST"}),
        json!({"url":"https://example.com/"}),
        json!({"unknown":true}),
    ] {
        assert!(apply_response(value, &request(), &mut response()).is_err());
    }
    let original = input(&request(), None).unwrap();
    apply_response(json!({"request":original}), &request(), &mut response()).unwrap();
}
#[test]
fn compressed_binary_and_oversized_input_are_rejected() {
    let mut request = request();
    request.headers = Cow::Owned(vec![Header {
        name: "Content-Encoding".into(),
        value: "gzip".into(),
    }]);
    assert_eq!(input(&request, None), Err(Error::Unsupported));
    request.headers = Cow::Borrowed(&[]);
    request.body = Cow::Borrowed(b"\xff");
    assert_eq!(input(&request, None), Err(Error::Unsupported));
    request.body = Cow::Owned(vec![b'x'; crate::script::MAX_DATA + 1]);
    assert_eq!(input(&request, None), Err(Error::ResourceLimit));
}
