//! JS 只处理有界 JSON 消息；输出重新经过协议校验，不接受脚本取得路由/文件/线程控制权。
use super::execution::{Message, body_header, headers_size};
use crate::{
    error::{Error, Result},
    http::Header,
    https::Response,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::borrow::Cow;
fn body_text<'a>(body: &'a [u8], headers: &[Header]) -> Result<&'a str> {
    if headers.iter().any(|h| {
        h.name.eq_ignore_ascii_case("content-encoding") && !h.value.eq_ignore_ascii_case("identity")
    }) {
        return Err(Error::Unsupported);
    }
    if body.len() > crate::script::MAX_DATA {
        return Err(Error::ResourceLimit);
    }
    std::str::from_utf8(body).map_err(|_| Error::Unsupported)
}
pub(super) fn input(request: &Message<'_>, response: Option<&Response>) -> Result<Value> {
    let original = json!({"method":request.method,"url":request.url,"headers":request.headers,"body":body_text(&request.body,&request.headers)?});
    match response {
        Some(value) => Ok(
            json!({"status":value.status,"headers":value.headers,"body":body_text(&value.body,&value.headers)?,"request":original}),
        ),
        None => Ok(original),
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Output {
    #[serde(default)]
    method: Option<String>,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    status: Option<u16>,
    #[serde(default)]
    headers: Option<Vec<Header>>,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    request: Option<Value>,
}
fn parse(value: Value) -> Result<Output> {
    crate::routing::runtime::yaml::decode(&value.to_string())
}
fn checked_headers(old: &[Header], new: &[Header]) -> Result<()> {
    headers_size(new)?;
    for header in new {
        if header.name.is_empty()
            || !header.name.bytes().all(crate::https::is_token)
            || header.value.bytes().any(|b| b < 32 || b == 127)
        {
            return Err(Error::InvalidInput);
        }
    }
    // 分帧、认证和逐跳头只能由内核管理；保留原值可回传，不能篡改或注入。
    let protected = |headers: &[Header]| {
        headers
            .iter()
            .filter(|h| crate::rewrite::protected_header(&h.name))
            .map(|h| (h.name.to_ascii_lowercase(), h.value.clone()))
            .collect::<Vec<_>>()
    };
    if protected(old) != protected(new) {
        return Err(Error::InvalidInput);
    }
    Ok(())
}
pub(super) fn apply_request(value: Value, request: &mut Message<'_>) -> Result<()> {
    let output = parse(value)?;
    if output.status.is_some() || output.request.is_some() {
        return Err(Error::InvalidInput);
    }
    if let Some(method) = output.method {
        if method.is_empty()
            || method.len() > 32
            || !method.bytes().all(crate::https::is_token)
            || method.eq_ignore_ascii_case("CONNECT")
        {
            return Err(Error::InvalidInput);
        }
        request.method = Cow::Owned(method);
    }
    if let Some(target) = output.url {
        if target.len() > 4096 {
            return Err(Error::ResourceLimit);
        }
        let old = url::Url::parse(&request.url).map_err(|_| Error::InvalidInput)?;
        let new = url::Url::parse(&target).map_err(|_| Error::InvalidInput)?;
        if old.origin() != new.origin()
            || new.fragment().is_some()
            || !new.username().is_empty()
            || new.password().is_some()
            || target.bytes().any(|b| b <= 32 || b == 127 || b == b'\\')
        {
            return Err(Error::InvalidInput);
        }
        request.url = Cow::Owned(target);
    }
    if let Some(headers) = output.headers {
        checked_headers(&request.headers, &headers)?;
        request.headers = Cow::Owned(headers);
    }
    if let Some(body) = output.body {
        if body.as_bytes() != request.body.as_ref() {
            request.headers.to_mut().retain(|h| !body_header(&h.name));
        }
        request.body = Cow::Owned(body.into_bytes());
    }
    Ok(())
}
pub(super) fn apply_response(
    value: Value,
    request: &Message<'_>,
    response: &mut Response,
) -> Result<()> {
    let output = parse(value)?;
    if output.method.is_some() || output.url.is_some() {
        return Err(Error::InvalidInput);
    }
    if let Some(original) = output.request
        && original != input(request, None)?
    {
        return Err(Error::InvalidInput);
    }
    if let Some(status) = output.status {
        super::final_status(status)?;
        response.status = status;
    }
    if let Some(headers) = output.headers {
        checked_headers(&response.headers, &headers)?;
        response.headers = headers;
    }
    if let Some(body) = output.body {
        if body.as_bytes() != response.body.as_slice() {
            response.headers.retain(|h| !body_header(&h.name));
        }
        response.body = body.into_bytes();
    }
    if matches!(response.status, 204 | 205 | 304) {
        response.body.clear();
        response.headers.retain(|h| !body_header(&h.name));
    }
    Ok(())
}
pub(crate) async fn execute(
    binding: std::sync::Arc<crate::script::worker::Binding>,
    cache: &std::sync::Arc<crate::script::worker::Cache>,
    request: &mut Message<'_>,
    response: Option<&mut Response>,
) -> Result<()> {
    let value = cache
        .execute(binding, input(request, response.as_deref())?)
        .await?;
    match response {
        Some(response) => apply_response(value, request, response),
        None => apply_request(value, request),
    }
}

#[cfg(test)]
#[path = "../../../tests/test_configuration_rewrites_script_message.rs"]
mod test_configuration_rewrites_script_message;
