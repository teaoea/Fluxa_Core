//! 迁移现有声明式改写；精确域名和路径前缀，不访问 Swift 或系统接口。
use crate::{
    error::{Error, Result},
    http::{Header, MAX_BODY, MAX_HEADER},
    https::{Response, is_token, normalize_host},
};
use serde::Deserialize;
use url::Url;

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rewrite {
    pub host: String,
    pub path_prefix: String,
    pub action: Action,
    pub header: Option<String>,
    pub value: Option<String>,
    pub r#match: Option<String>,
}
#[derive(Clone, Copy, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum Action {
    Reject,
    RequestHeader,
    ResponseHeader,
    ReplaceResponseText,
}
impl Rewrite {
    pub fn validate(&self) -> Result<()> {
        normalize_host(&self.host)?;
        if !self.path_prefix.starts_with('/') || self.path_prefix.len() > 2048 {
            return Err(Error::InvalidInput);
        }
        match self.action {
            Action::RequestHeader | Action::ResponseHeader => {
                let name = self.header.as_ref().ok_or(Error::InvalidInput)?;
                let value = self.value.as_ref().ok_or(Error::InvalidInput)?;
                if name.is_empty()
                    || name.len() > 128
                    || !name.bytes().all(is_token)
                    || value.len() > 8192
                    || value.bytes().any(|b| b < 32 || b == 127)
                    || [
                        "host",
                        "content-length",
                        "transfer-encoding",
                        "connection",
                        "content-encoding",
                        "proxy-authorization",
                        "proxy-authenticate",
                        "proxy-connection",
                        "keep-alive",
                        "upgrade",
                        "trailer",
                        "expect",
                    ]
                    .contains(&name.to_ascii_lowercase().as_str())
                {
                    return Err(Error::InvalidInput);
                }
            }
            Action::ReplaceResponseText => {
                let pattern = self.r#match.as_ref().ok_or(Error::InvalidInput)?;
                let value = self.value.as_ref().ok_or(Error::InvalidInput)?;
                if pattern.is_empty() || pattern.len() > 8192 || value.len() > 8192 {
                    return Err(Error::InvalidInput);
                }
            }
            Action::Reject => {}
        }
        Ok(())
    }
    fn matches(&self, url: &Url) -> bool {
        url.host_str().and_then(|h| normalize_host(h).ok()) == normalize_host(&self.host).ok()
            && url.path().starts_with(&self.path_prefix)
    }
}
fn replace_header(headers: &mut Vec<Header>, rule: &Rewrite) -> Result<()> {
    if let (Some(name), Some(value)) = (&rule.header, &rule.value) {
        headers.retain(|h| !h.name.eq_ignore_ascii_case(name));
        let bytes: usize = headers
            .iter()
            .map(|h| h.name.len() + h.value.len() + 4)
            .sum();
        if headers.len() >= 128 || bytes + name.len() + value.len() + 4 > MAX_HEADER {
            return Err(Error::ResourceLimit);
        }
        headers.push(Header {
            name: name.clone(),
            value: value.clone(),
        });
    }
    Ok(())
}
pub fn request(url: &str, headers: &mut Vec<Header>, rules: &[Rewrite]) -> Result<()> {
    let url = Url::parse(url).map_err(|_| Error::InvalidInput)?;
    for rule in rules.iter().filter(|r| r.matches(&url)) {
        match rule.action {
            Action::Reject => return Err(Error::Rejected),
            Action::RequestHeader => replace_header(headers, rule)?,
            _ => {}
        }
    }
    Ok(())
}
pub fn response(url: &str, response: &mut Response, rules: &[Rewrite]) -> Result<()> {
    let url = Url::parse(url).map_err(|_| Error::InvalidInput)?;
    for rule in rules.iter().filter(|r| r.matches(&url)) {
        if rule.action == Action::ResponseHeader {
            replace_header(&mut response.headers, rule)?;
        }
        if rule.action != Action::ReplaceResponseText || response.body.len() > 1024 * 1024 {
            continue;
        }
        let mime = response
            .headers
            .iter()
            .find(|h| h.name.eq_ignore_ascii_case("content-type"));
        let encoded = response.headers.iter().any(|h| {
            h.name.eq_ignore_ascii_case("content-encoding")
                && !h.value.eq_ignore_ascii_case("identity")
        });
        if encoded
            || !mime.is_some_and(|h| h.value.starts_with("text/") || h.value.contains("json"))
        {
            continue;
        }
        let Ok(text) = std::str::from_utf8(&response.body) else {
            continue;
        };
        let (Some(pattern), Some(value)) = (&rule.r#match, &rule.value) else {
            return Err(Error::InvalidInput);
        };
        // 先计算替换后长度，避免短匹配/长替换造成巨大临时分配。
        let count = text.match_indices(pattern).count();
        let length = text
            .len()
            .checked_sub(
                count
                    .checked_mul(pattern.len())
                    .ok_or(Error::ResourceLimit)?,
            )
            .and_then(|n| n.checked_add(count.checked_mul(value.len())?))
            .ok_or(Error::ResourceLimit)?;
        if length > MAX_BODY {
            return Err(Error::ResourceLimit);
        }
        if count != 0 {
            response.body = text.replace(pattern, value).into_bytes();
            response.headers.retain(|h| {
                !["etag", "content-md5", "digest", "content-length"]
                    .contains(&h.name.to_ascii_lowercase().as_str())
            });
        }
    }
    Ok(())
}
