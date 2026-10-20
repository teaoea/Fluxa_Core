//! 原生复写执行：每阶段第一条完整命中生效；请求/响应条件读取未被本阶段修改的输入。
use super::{Body, Changes, Phase, Preset, Request, Rule, Wire};
use crate::{
    configuration::Document,
    error::{Error, Result},
    http::{Header, MAX_BODY, MAX_HEADER},
    https::Response,
};
use std::borrow::Cow;

#[derive(Clone)]
pub(crate) struct Message<'a> {
    pub method: Cow<'a, str>,
    pub url: Cow<'a, str>,
    pub headers: Cow<'a, [Header]>,
    pub body: Cow<'a, [u8]>,
}
pub(crate) enum Outcome {
    Continue,
    Script(std::sync::Arc<crate::script::worker::Binding>),
    Response(Response),
    Route(crate::routing::Policy, Option<String>),
}
impl Document {
    pub(crate) fn has_rewrites(&self) -> bool {
        self.raw.rewrites.iter().any(|r| {
            r.wire.common().enable
                && !(matches!(r.wire, Wire::Proxy { .. }) && r.proxy_node.is_none())
        })
    }
    #[cfg(test)]
    pub(crate) fn rewrite_request(
        &self,
        original: &Message<'_>,
        output: &mut Message<'_>,
    ) -> Result<Outcome> {
        self.rewrite_request_bound(
            original,
            output,
            None,
            &super::super::plugins::Bundle::default(),
        )
    }
    pub(crate) fn rewrite_request_bound(
        &self,
        original: &Message<'_>,
        output: &mut Message<'_>,
        bindings: Option<&crate::script::worker::Bindings>,
        plugins: &super::super::plugins::Bundle,
    ) -> Result<Outcome> {
        for (rule, bindings) in self.raw.rewrites.iter().map(|r| (r, bindings)).chain(
            plugins
                .plugins
                .iter()
                .flat_map(|p| p.rules.iter().map(|r| (r, Some(p.bindings.as_ref())))),
        ) {
            if let Wire::Script { js, ttl, .. } = &rule.wire
                && bindings.is_some_and(|b| !b.contains(js, *ttl))
            {
                continue;
            }
            if !rule.matches(original, None, Phase::Request)? {
                continue;
            }
            return match &rule.wire {
                Wire::Reject { status, .. } => {
                    Ok(Outcome::Response(response(*status, Vec::new(), Vec::new())))
                }
                Wire::Respond {
                    response: change, ..
                } => {
                    let mut value = response(change.status.unwrap_or(200), Vec::new(), Vec::new());
                    apply_response(change, &mut value)?;
                    Ok(Outcome::Response(value))
                }
                Wire::Redirect {
                    status, location, ..
                } => Ok(Outcome::Response(response(
                    *status,
                    vec![Header {
                        name: "Location".into(),
                        value: location.clone(),
                    }],
                    Vec::new(),
                ))),
                Wire::Abort { .. } => Err(Error::Cancelled),
                Wire::ModifyRequest { request, .. } => {
                    apply_request(request, output)?;
                    Ok(Outcome::Continue)
                }
                Wire::Direct { .. } => Ok(Outcome::Route(crate::routing::Policy::Direct, None)),
                Wire::Proxy { .. } => Ok(Outcome::Route(
                    crate::routing::Policy::Proxy,
                    rule.proxy_node.clone(),
                )),
                // 只允许已成功绑定的命名资源进入 JS 工作线程。
                Wire::Script { js, ttl, .. } => match bindings {
                    Some(b) => Ok(Outcome::Script(b.get(js, *ttl)?)),
                    None => Err(Error::Unsupported),
                },
                Wire::ModifyResponse { .. } => Err(Error::Internal),
            };
        }
        Ok(Outcome::Continue)
    }
    pub(crate) async fn rewrite_response_bound(
        &self,
        request: &Message<'_>,
        value: &mut Response,
        bindings: &crate::script::worker::Bindings,
        cache: &std::sync::Arc<crate::script::worker::Cache>,
        plugins: &super::super::plugins::Bundle,
    ) -> Result<()> {
        let selected = self.response_rule(request, value, bindings, plugins)?;
        let Some((rule, bindings)) = selected else {
            return Ok(());
        };
        match &rule.wire {
            Wire::ModifyResponse { response, .. } => apply_response(response, value),
            Wire::Script { js, ttl, .. } => {
                super::script_message::execute(
                    bindings.get(js, *ttl)?,
                    cache,
                    &mut request.clone(),
                    Some(value),
                )
                .await
            }
            _ => Err(Error::Internal),
        }
    }
    fn response_rule<'a>(
        &'a self,
        request: &Message<'_>,
        value: &Response,
        bindings: &'a crate::script::worker::Bindings,
        plugins: &'a super::super::plugins::Bundle,
    ) -> Result<Option<(&'a Rule, &'a crate::script::worker::Bindings)>> {
        for (rule, bindings) in self.raw.rewrites.iter().map(|r| (r, bindings)).chain(
            plugins
                .plugins
                .iter()
                .flat_map(|p| p.rules.iter().map(|r| (r, p.bindings.as_ref()))),
        ) {
            if let Wire::Script { js, ttl, .. } = &rule.wire
                && !bindings.contains(js, *ttl)
            {
                continue;
            }
            if rule.matches(request, Some(value), Phase::Response)? {
                return Ok(Some((rule, bindings)));
            }
        }
        Ok(None)
    }
    #[cfg(test)]
    pub(crate) fn rewrite_response(
        &self,
        request: &Message<'_>,
        value: &mut Response,
    ) -> Result<()> {
        for rule in &self.raw.rewrites {
            if !rule.matches(request, Some(value), Phase::Response)? {
                continue;
            }
            return match &rule.wire {
                Wire::ModifyResponse { response, .. } => apply_response(response, value),
                Wire::Script { .. } => Err(Error::Unsupported),
                _ => Err(Error::Internal),
            };
        }
        Ok(())
    }
}
impl Rule {
    fn matches(
        &self,
        request: &Message<'_>,
        response: Option<&Response>,
        phase: Phase,
    ) -> Result<bool> {
        let common = self.wire.common();
        if !common.enable
            || self.wire.phase() != phase
            || matches!(self.wire, Wire::Proxy { .. }) && self.proxy_node.is_none()
        {
            return Ok(false);
        }
        if !common
            .compiled_url
            .as_ref()
            .ok_or(Error::Internal)?
            .matches(&request.url)?
        {
            return Ok(false);
        }
        match &common.when {
            Some(when) => when.matches(request, response, phase),
            None => Ok(true),
        }
    }
}
fn response(status: u16, headers: Vec<Header>, body: Vec<u8>) -> Response {
    Response {
        status,
        headers,
        body,
        duration_ms: 0,
    }
}
pub(super) fn headers_size(headers: &[Header]) -> Result<()> {
    if headers.len() > 128
        || headers
            .iter()
            .map(|h| h.name.len() + h.value.len() + 4)
            .sum::<usize>()
            > MAX_HEADER
    {
        return Err(Error::ResourceLimit);
    }
    Ok(())
}
fn change_headers(changes: &Changes, headers: &mut Vec<Header>) -> Result<()> {
    headers.retain(|h| {
        !changes
            .remove
            .iter()
            .any(|n| n.eq_ignore_ascii_case(&h.name))
            && !changes.set.keys().any(|n| n.eq_ignore_ascii_case(&h.name))
    });
    headers.extend(changes.set.iter().map(|(name, value)| Header {
        name: name.clone(),
        value: value.clone(),
    }));
    headers_size(headers)
}
fn apply_request(change: &Request, output: &mut Message<'_>) -> Result<()> {
    if let Some(headers) = &change.headers {
        change_headers(headers, output.headers.to_mut())?;
    }
    if let Some(method) = &change.method {
        output.method = Cow::Owned(method.clone());
    }
    if let Some(query) = &change.query {
        // 只替换查询部分，保留原始来源/路径字节，避免 URL 标准化改变字面匹配范围。
        let (base, raw) = output.url.split_once('?').unwrap_or((&output.url, ""));
        let mut serializer = url::form_urlencoded::Serializer::new(String::new());
        for (name, value) in url::form_urlencoded::parse(raw.as_bytes()) {
            if !query.remove.contains(&name.to_string()) && !query.set.contains_key(name.as_ref()) {
                serializer.append_pair(&name, &value);
            }
        }
        for (name, value) in &query.set {
            serializer.append_pair(name, value);
        }
        let query = serializer.finish();
        let updated = if query.is_empty() {
            base.to_owned()
        } else {
            format!("{base}?{query}")
        };
        if updated.len() > 4096 {
            return Err(Error::ResourceLimit);
        }
        output.url = Cow::Owned(updated);
    }
    if let Some(body) = &change.body {
        if let Some(text) = &body.text {
            output.body = Cow::Owned(text.as_bytes().to_vec());
        }
        // 替换为明确的正文后，旧压缩标识及完整性校验已不再有效。
        output.headers.to_mut().retain(|h| !body_header(&h.name));
    }
    Ok(())
}
pub(super) fn body_header(name: &str) -> bool {
    [
        "etag",
        "content-md5",
        "digest",
        "content-length",
        "content-encoding",
        "content-range",
        "accept-ranges",
        "transfer-encoding",
    ]
    .iter()
    .any(|n| name.eq_ignore_ascii_case(n))
}
fn materialize(body: &Body) -> Result<(Vec<u8>, Option<&'static str>)> {
    match (&body.text, &body.preset) {
        (Some(text), None) => Ok((text.as_bytes().to_vec(), None)),
        (None, Some(Preset::TransparentPng)) => {
            Ok((super::presets::PNG.to_vec(), Some("image/png")))
        }
        (None, Some(Preset::MinimalMp4)) => Ok((super::presets::MP4.to_vec(), Some("video/mp4"))),
        _ => Err(Error::Internal),
    }
}
fn apply_response(change: &super::Response, value: &mut Response) -> Result<()> {
    if let Some(status) = change.status {
        value.status = status;
    }
    if let Some(body) = &change.body {
        let (bytes, mime) = materialize(body)?;
        if bytes.len() > MAX_BODY {
            return Err(Error::ResourceLimit);
        }
        value.body = bytes;
        value.headers.retain(|h| !body_header(&h.name));
        if let Some(mime) = mime {
            value
                .headers
                .retain(|h| !h.name.eq_ignore_ascii_case("content-type"));
            value.headers.push(Header {
                name: "Content-Type".into(),
                value: mime.into(),
            });
        }
    }
    if let Some(headers) = &change.headers {
        change_headers(headers, &mut value.headers)?;
    }
    if matches!(value.status, 204 | 205 | 304) {
        value.body.clear();
        value.headers.retain(|h| !body_header(&h.name));
    }
    // 写出层重新计算 Content-Length，不信任旧分帧信息。
    headers_size(&value.headers)
}
#[cfg(test)]
#[path = "../../../tests/test_configuration_rewrites_execution.rs"]
mod test_configuration_rewrites_execution;
