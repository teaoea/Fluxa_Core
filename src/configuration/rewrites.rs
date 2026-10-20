//! 主配置复写声明：严格动作/阶段校验、原生条件和加载期 URL 编译。
mod conditions;
pub(crate) mod execution;
mod presets;
pub(crate) mod script_message;
use crate::error::{Error, Result};
use conditions::When;
use serde::Deserialize;
use std::collections::{BTreeMap, HashSet};

/// 节点由 action: proxy(名称) 引用；不接受配置注入额外内部字段。
pub(super) struct Rule {
    wire: Wire,
    proxy_node: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Declaration {
    #[serde(flatten)]
    common: Common,
    action: String,
    #[serde(default, rename = "js-ttl", deserialize_with = "present")]
    js_ttl: Option<u32>,
    #[serde(default, deserialize_with = "present")]
    status: Option<u16>,
    #[serde(default, deserialize_with = "present")]
    response: Option<Response>,
    #[serde(default, deserialize_with = "present")]
    request: Option<Request>,
    #[serde(default, deserialize_with = "present")]
    location: Option<String>,
    #[serde(default, deserialize_with = "present")]
    js: Option<String>,
}
impl<'de> Deserialize<'de> for Rule {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        // 直接使用原事件反序列化器，保留未知字段/重复键的结构化错误，不能转成字符串再猜类型。
        let input = Declaration::deserialize(deserializer)?;
        if input.action != "script" && input.js_ttl.is_some() || input.js_ttl == Some(0) {
            return Err(serde::de::Error::custom("js-ttl 仅用于脚本，必须大于零"));
        }
        let ttl = input.js_ttl.unwrap_or(3600);
        let fields = u8::from(input.status.is_some())
            | (u8::from(input.response.is_some()) << 1)
            | (u8::from(input.request.is_some()) << 2)
            | (u8::from(input.location.is_some()) << 3)
            | (u8::from(input.js.is_some()) << 4);
        let expected = match input.action.as_str() {
            "reject" => 1,
            "respond" | "modify-response" => 2,
            "modify-request" => 4,
            "redirect" => 9,
            "script" => 16,
            _ => 0,
        };
        if fields != expected {
            return Err(serde::de::Error::custom("动作字段不匹配"));
        }
        let mut proxy_node = None;
        let common = input.common;
        let missing = || serde::de::Error::custom("缺少动作字段");
        let wire = match input.action.as_str() {
            "reject" => Wire::Reject {
                common,
                status: input.status.ok_or_else(missing)?,
            },
            "respond" => Wire::Respond {
                common,
                response: input.response.ok_or_else(missing)?,
            },
            "modify-response" => Wire::ModifyResponse {
                common,
                response: input.response.ok_or_else(missing)?,
            },
            "modify-request" => Wire::ModifyRequest {
                common,
                request: input.request.ok_or_else(missing)?,
            },
            "redirect" => Wire::Redirect {
                common,
                status: input.status.ok_or_else(missing)?,
                location: input.location.ok_or_else(missing)?,
            },
            "abort" => Wire::Abort { common },
            "direct" => Wire::Direct { common },
            "proxy" => Wire::Proxy { common },
            "script" => Wire::Script {
                common,
                js: input.js.ok_or_else(missing)?,
                ttl,
            },
            action => {
                let name = action
                    .strip_prefix("proxy(")
                    .and_then(|s| s.strip_suffix(')'))
                    .ok_or_else(|| serde::de::Error::custom("未知复写动作"))?;
                if !name.is_empty() {
                    proxy_node = Some(name.to_owned());
                }
                Wire::Proxy { common }
            }
        };
        Ok(Self { wire, proxy_node })
    }
}
impl Rule {
    pub(super) fn validate(&mut self) -> Result<()> {
        if let Some(node) = &self.proxy_node {
            super::name(node)?;
        }
        self.wire.validate()
    }
    pub(super) fn proxy_node(&self) -> Option<&str> {
        self.proxy_node.as_deref()
    }
    pub(super) fn script_reference(&self) -> Option<(String, u32)> {
        match &self.wire {
            Wire::Script { common, js, ttl } if common.enable => Some((js.clone(), *ttl)),
            _ => None,
        }
    }
}
enum Wire {
    Reject {
        common: Common,
        status: u16,
    },
    Respond {
        common: Common,
        response: Response,
    },
    ModifyResponse {
        common: Common,
        response: Response,
    },
    ModifyRequest {
        common: Common,
        request: Request,
    },
    Redirect {
        common: Common,
        status: u16,
        location: String,
    },
    Abort {
        common: Common,
    },
    Direct {
        common: Common,
    },
    Proxy {
        common: Common,
    },
    Script {
        common: Common,
        js: String,
        ttl: u32,
    },
}
#[derive(Deserialize)]
struct Common {
    url: String,
    #[serde(default = "enabled")]
    enable: bool,
    #[serde(default, deserialize_with = "present")]
    phase: Option<Phase>,
    #[serde(default, deserialize_with = "present")]
    when: Option<When>,
    #[serde(skip)]
    compiled_url: Option<crate::rewrite::UrlPattern>,
}
fn enabled() -> bool {
    true
}
#[derive(Clone, Copy, PartialEq, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Phase {
    Request,
    Response,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    #[serde(default, deserialize_with = "present")]
    headers: Option<Changes>,
    #[serde(default, deserialize_with = "present")]
    method: Option<String>,
    #[serde(default, deserialize_with = "present")]
    query: Option<Changes>,
    #[serde(default, deserialize_with = "present")]
    body: Option<Body>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Response {
    #[serde(default, deserialize_with = "present")]
    status: Option<u16>,
    #[serde(default, deserialize_with = "present")]
    headers: Option<Changes>,
    #[serde(default, deserialize_with = "present")]
    body: Option<Body>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Changes {
    #[serde(default)]
    set: BTreeMap<String, String>,
    #[serde(default)]
    remove: Vec<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Body {
    #[serde(default, deserialize_with = "present")]
    text: Option<String>,
    #[serde(default, deserialize_with = "present")]
    preset: Option<Preset>,
}
#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
enum Preset {
    TransparentPng,
    MinimalMp4,
}
// 出现的可选字段必须有真实值，null 不能代替省略。
fn present<'de, D, T>(deserializer: D) -> std::result::Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
}
impl Wire {
    fn common(&self) -> &Common {
        match self {
            Self::Reject { common, .. }
            | Self::Respond { common, .. }
            | Self::ModifyResponse { common, .. }
            | Self::ModifyRequest { common, .. }
            | Self::Redirect { common, .. }
            | Self::Abort { common }
            | Self::Direct { common }
            | Self::Proxy { common }
            | Self::Script { common, .. } => common,
        }
    }
    fn phase(&self) -> Phase {
        self.common()
            .phase
            .unwrap_or(if matches!(self, Self::ModifyResponse { .. }) {
                Phase::Response
            } else {
                Phase::Request
            })
    }
    pub(super) fn validate(&mut self) -> Result<()> {
        let phase = self.phase();
        if matches!(self, Self::Script { .. }) && self.common().phase.is_none() {
            return Err(Error::InvalidInput);
        }
        if matches!(self, Self::ModifyResponse { .. }) && phase != Phase::Response
            || !matches!(self, Self::ModifyResponse { .. } | Self::Script { .. })
                && phase != Phase::Request
        {
            return Err(Error::InvalidInput);
        }
        if let Some(when) = &self.common().when {
            when.validate(phase)?;
        }
        let common = match self {
            Self::Reject { common, status } => {
                final_status(*status)?;
                common
            }
            Self::Respond { common, response } | Self::ModifyResponse { common, response } => {
                if response.status.is_none()
                    && response.headers.is_none()
                    && response.body.is_none()
                {
                    return Err(Error::InvalidInput);
                }
                if let Some(status) = response.status {
                    final_status(status)?;
                }
                if let Some(headers) = &response.headers {
                    headers.validate(true)?;
                }
                if let Some(body) = &response.body {
                    body.validate(false)?;
                }
                if matches!(response.status, Some(204 | 205 | 304)) && response.body.is_some() {
                    return Err(Error::InvalidInput);
                }
                common
            }
            Self::ModifyRequest { common, request } => {
                request.validate()?;
                common
            }
            Self::Redirect {
                common,
                status,
                location,
            } => {
                if !matches!(status, 301 | 302 | 303 | 307 | 308) {
                    return Err(Error::InvalidInput);
                }
                crate::rewrite::UrlPattern::compile(location)?;
                if location.contains('*') {
                    return Err(Error::InvalidInput);
                }
                common
            }
            Self::Script { common, js, .. } => {
                if js.is_empty()
                    || js.len() > 128
                    || js.starts_with('.')
                    || !js
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.'))
                {
                    return Err(Error::InvalidInput);
                }
                common
            }
            Self::Abort { common } | Self::Direct { common } | Self::Proxy { common } => common,
        };
        common.compiled_url = Some(crate::rewrite::UrlPattern::compile(&common.url)?);
        Ok(())
    }
}
fn final_status(status: u16) -> Result<()> {
    if (200..=599).contains(&status) {
        Ok(())
    } else {
        Err(Error::InvalidInput)
    }
}
impl Request {
    fn validate(&self) -> Result<()> {
        if self.headers.is_none()
            && self.method.is_none()
            && self.query.is_none()
            && self.body.is_none()
        {
            return Err(Error::InvalidInput);
        }
        if let Some(headers) = &self.headers {
            headers.validate(true)?;
        }
        if let Some(method) = &self.method
            && (method.is_empty()
                || method.len() > 32
                || !method.bytes().all(crate::https::is_token)
                || method.eq_ignore_ascii_case("CONNECT"))
        {
            return Err(Error::InvalidInput);
        }
        if let Some(query) = &self.query {
            query.validate(false)?;
        }
        if let Some(body) = &self.body {
            body.validate(true)?;
        }
        Ok(())
    }
}
impl Body {
    fn validate(&self, request: bool) -> Result<()> {
        match (&self.text, &self.preset) {
            (Some(text), None) if text.len() <= 1024 * 1024 => Ok(()),
            (None, Some(_)) if !request => Ok(()),
            (Some(text), None) if text.len() > 1024 * 1024 => Err(Error::ResourceLimit),
            _ => Err(Error::InvalidInput),
        }
    }
}
impl Changes {
    fn validate(&self, headers: bool) -> Result<()> {
        if self.set.len() + self.remove.len() > 128 {
            return Err(Error::ResourceLimit);
        }
        if self.set.is_empty() && self.remove.is_empty() {
            return Err(Error::InvalidInput);
        }
        let mut names = HashSet::new();
        let mut bytes = 0usize;
        for name in self.set.keys().chain(&self.remove) {
            if name.is_empty() || name.len() > 256 || name.chars().any(char::is_control) {
                return Err(Error::InvalidInput);
            }
            let key = if headers {
                name.to_ascii_lowercase()
            } else {
                name.clone()
            };
            if !names.insert(key) {
                return Err(Error::InvalidInput);
            }
            if headers
                && (!name.bytes().all(crate::https::is_token)
                    || crate::rewrite::protected_header(name))
            {
                return Err(Error::InvalidInput);
            }
            bytes = bytes.checked_add(name.len()).ok_or(Error::ResourceLimit)?;
        }
        for value in self.set.values() {
            if value.len() > 8192 || (headers && value.bytes().any(|b| b < 32 || b == 127)) {
                return Err(Error::InvalidInput);
            }
            bytes = bytes.checked_add(value.len()).ok_or(Error::ResourceLimit)?;
        }
        if bytes > crate::http::MAX_HEADER {
            return Err(Error::ResourceLimit);
        }
        Ok(())
    }
}
#[cfg(test)]
#[path = "../../tests/test_configuration_rewrites.rs"]
mod test_configuration_rewrites;
