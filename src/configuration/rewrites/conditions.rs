//! 原生 when 条件：字段间及 exists/equals 内部均为 AND，不引入表达式或正则解释器。
use super::{Phase, present};
use crate::{
    error::{Error, Result},
    http::{Header, MAX_BODY, MAX_HEADER},
};
use serde::Deserialize;
use std::collections::{BTreeMap, HashSet};

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub(super) struct When {
    #[serde(default, deserialize_with = "present")]
    method: Option<String>,
    #[serde(default, deserialize_with = "present")]
    request_headers: Option<Fields>,
    #[serde(default, deserialize_with = "present")]
    query: Option<Fields>,
    #[serde(default, deserialize_with = "present")]
    status: Option<u16>,
    #[serde(default, deserialize_with = "present")]
    response_headers: Option<Fields>,
    #[serde(default, deserialize_with = "present")]
    content_type: Option<String>,
    #[serde(default, deserialize_with = "present")]
    content_length: Option<Length>,
    #[serde(default, deserialize_with = "present")]
    request_body: Option<Text>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Fields {
    #[serde(default)]
    exists: Vec<String>,
    #[serde(default)]
    equals: BTreeMap<String, String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Length {
    #[serde(default, deserialize_with = "present")]
    min: Option<usize>,
    #[serde(default, deserialize_with = "present")]
    max: Option<usize>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Text {
    #[serde(default, deserialize_with = "present")]
    equals: Option<String>,
    #[serde(default, deserialize_with = "present")]
    contains: Option<String>,
}
impl When {
    pub(super) fn validate(&self, phase: Phase) -> Result<()> {
        if self.method.is_none()
            && self.request_headers.is_none()
            && self.query.is_none()
            && self.status.is_none()
            && self.response_headers.is_none()
            && self.content_type.is_none()
            && self.content_length.is_none()
            && self.request_body.is_none()
        {
            return Err(Error::InvalidInput);
        }
        if phase == Phase::Request && (self.status.is_some() || self.response_headers.is_some()) {
            return Err(Error::InvalidInput);
        }
        if let Some(method) = &self.method
            && (method.is_empty()
                || method.len() > 32
                || !method.bytes().all(crate::https::is_token))
        {
            return Err(Error::InvalidInput);
        }
        for fields in [&self.request_headers, &self.response_headers]
            .into_iter()
            .flatten()
        {
            fields.validate(true)?;
        }
        if let Some(fields) = &self.query {
            fields.validate(false)?;
        }
        if let Some(status) = self.status {
            super::final_status(status)?;
        }
        if let Some(mime) = &self.content_type {
            let Some((kind, subtype)) = mime.split_once('/') else {
                return Err(Error::InvalidInput);
            };
            if mime.len() > 256
                || kind.is_empty()
                || subtype.is_empty()
                || !kind
                    .bytes()
                    .chain(subtype.bytes())
                    .all(crate::https::is_token)
                || mime.contains('*')
            {
                return Err(Error::InvalidInput);
            }
        }
        if let Some(length) = &self.content_length
            && (length.min.is_none() && length.max.is_none()
                || length.min.unwrap_or(0) > length.max.unwrap_or(MAX_BODY)
                || length.max.unwrap_or(0) > MAX_BODY)
        {
            return Err(Error::InvalidInput);
        }
        if let Some(text) = &self.request_body {
            if text.equals.is_none() && text.contains.is_none()
                || text.contains.as_ref().is_some_and(|s| s.is_empty())
            {
                return Err(Error::InvalidInput);
            }
            if text.equals.as_ref().is_some_and(|s| s.len() > 8192)
                || text.contains.as_ref().is_some_and(|s| s.len() > 8192)
            {
                return Err(Error::ResourceLimit);
            }
        }
        Ok(())
    }
    pub(super) fn matches(
        &self,
        request: &super::execution::Message<'_>,
        response: Option<&crate::https::Response>,
        phase: Phase,
    ) -> Result<bool> {
        if self
            .method
            .as_ref()
            .is_some_and(|m| m.as_str() != request.method.as_ref())
        {
            return Ok(false);
        }
        if self
            .request_headers
            .as_ref()
            .is_some_and(|f| !f.matches_headers(&request.headers))
        {
            return Ok(false);
        }
        if let Some(fields) = &self.query {
            // 查询按 application/x-www-form-urlencoded 解码；重复参数保留，equals 任意一个同名值成立即可。
            let query = request.url.split_once('?').map(|(_, q)| q).unwrap_or("");
            let pairs: Vec<_> = url::form_urlencoded::parse(query.as_bytes()).collect();
            if !fields
                .exists
                .iter()
                .all(|name| pairs.iter().any(|(n, _)| n == name))
                || !fields
                    .equals
                    .iter()
                    .all(|(name, value)| pairs.iter().any(|(n, v)| n == name && v == value))
            {
                return Ok(false);
            }
        }
        if let Some(status) = self.status
            && response.is_none_or(|r| r.status != status)
        {
            return Ok(false);
        }
        if let Some(fields) = &self.response_headers
            && response.is_none_or(|r| !fields.matches_headers(&r.headers))
        {
            return Ok(false);
        }
        let (headers, length): (&[Header], usize) = if phase == Phase::Response {
            let Some(response) = response else {
                return Ok(false);
            };
            (&response.headers, response.body.len())
        } else {
            (&request.headers, request.body.len())
        };
        if let Some(mime) = &self.content_type
            && !headers.iter().any(|h| {
                h.name.eq_ignore_ascii_case("content-type")
                    && h.value
                        .split(';')
                        .next()
                        .is_some_and(|s| s.trim().eq_ignore_ascii_case(mime))
            })
        {
            return Ok(false);
        }
        if let Some(range) = &self.content_length
            && (length < range.min.unwrap_or(0) || length > range.max.unwrap_or(MAX_BODY))
        {
            return Ok(false);
        }
        if let Some(condition) = &self.request_body {
            if request.headers.iter().any(|h| {
                h.name.eq_ignore_ascii_case("content-encoding")
                    && !h.value.eq_ignore_ascii_case("identity")
            }) {
                return Err(Error::Unsupported);
            }
            let Ok(body) = std::str::from_utf8(&request.body) else {
                return Ok(false);
            };
            if condition.equals.as_ref().is_some_and(|s| s != body)
                || condition
                    .contains
                    .as_ref()
                    .is_some_and(|s| !body.contains(s))
            {
                return Ok(false);
            }
        }
        Ok(true)
    }
}
impl Fields {
    fn validate(&self, headers: bool) -> Result<()> {
        if self.exists.is_empty() && self.equals.is_empty() {
            return Err(Error::InvalidInput);
        }
        if self.exists.len() + self.equals.len() > 128 {
            return Err(Error::ResourceLimit);
        }
        let mut total = 0;
        for group in [
            self.exists.iter().collect::<Vec<_>>(),
            self.equals.keys().collect(),
        ] {
            let mut seen = HashSet::new();
            for name in group {
                if name.is_empty()
                    || name.len() > 256
                    || name.chars().any(char::is_control)
                    || headers && !name.bytes().all(crate::https::is_token)
                {
                    return Err(Error::InvalidInput);
                }
                if !seen.insert(if headers {
                    name.to_ascii_lowercase()
                } else {
                    name.clone()
                }) {
                    return Err(Error::InvalidInput);
                }
                total += name.len();
            }
        }
        for value in self.equals.values() {
            if value.len() > 8192 || headers && value.bytes().any(|b| b < 32 || b == 127) {
                return Err(Error::InvalidInput);
            }
            total += value.len();
        }
        if total > MAX_HEADER {
            return Err(Error::ResourceLimit);
        }
        Ok(())
    }
    fn matches_headers(&self, headers: &[Header]) -> bool {
        self.exists
            .iter()
            .all(|name| headers.iter().any(|h| h.name.eq_ignore_ascii_case(name)))
            && self.equals.iter().all(|(name, value)| {
                headers
                    .iter()
                    .any(|h| h.name.eq_ignore_ascii_case(name) && &h.value == value)
            })
    }
}
