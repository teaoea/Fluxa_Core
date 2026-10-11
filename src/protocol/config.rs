//! 协议配置仅接受明确支持的组合，凭据不进入调试输出。
use crate::{
    error::{Error, Result},
    network::normalize_host,
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use zeroize::Zeroize;

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub id: String,
    pub uri: String,
}
impl Drop for Profile {
    fn drop(&mut self) {
        self.uri.zeroize();
    }
}
pub struct Node {
    pub host: String,
    pub port: u16,
    pub sni: String,
    pub public_key: [u8; 32],
    pub short_id: [u8; 8],
    pub uuid: [u8; 16],
    pub fingerprint: super::fingerprint::Fingerprint,
}
impl Drop for Node {
    fn drop(&mut self) {
        self.uuid.zeroize();
    }
}
impl Profile {
    pub fn validate(&self) -> Result<Node> {
        crate::routing::identifier(&self.id)?;
        if self.uri.len() > 4096 {
            return Err(Error::InvalidInput);
        }
        let u = url::Url::parse(&self.uri).map_err(|_| Error::InvalidInput)?;
        if u.scheme() != "vless" || u.password().is_some() || !matches!(u.path(), "" | "/") {
            return Err(Error::Unsupported);
        }
        let host = normalize_host(
            u.host_str()
                .ok_or(Error::InvalidInput)?
                .trim_matches(['[', ']']),
        )?;
        let port = u
            .port()
            .filter(|port| *port != 0)
            .ok_or(Error::InvalidInput)?;
        let uuid = uuid(u.username())?;
        let mut values = BTreeMap::new();
        for (key, value) in u.query_pairs() {
            if !matches!(
                key.as_ref(),
                "security" | "type" | "encryption" | "flow" | "sni" | "pbk" | "sid" | "fp"
            ) {
                return Err(Error::Unsupported);
            }
            if values
                .insert(key.into_owned(), value.into_owned())
                .is_some()
            {
                return Err(Error::InvalidInput);
            }
        }
        let get = |name: &str| values.get(name).map(String::as_str).unwrap_or("");
        if get("security") != "reality"
            || !matches!(get("type"), "" | "tcp" | "raw")
            || !matches!(get("encryption"), "" | "none")
            || get("flow") != "xtls-rprx-vision"
        {
            return Err(Error::Unsupported);
        }
        let fingerprint = super::fingerprint::Fingerprint::parse(if get("fp").is_empty() {
            "chrome"
        } else {
            get("fp")
        })?;
        let sni = normalize_host(get("sni"))?;
        if sni.parse::<std::net::IpAddr>().is_ok() {
            return Err(Error::InvalidInput);
        }
        let public_key: [u8; 32] = URL_SAFE_NO_PAD
            .decode(get("pbk"))
            .map_err(|_| Error::InvalidInput)?
            .try_into()
            .map_err(|_| Error::InvalidInput)?;
        if public_key == [0; 32] {
            return Err(Error::InvalidInput);
        }
        let sid = get("sid");
        if sid.len() > 16 || !sid.len().is_multiple_of(2) {
            return Err(Error::InvalidInput);
        }
        let mut short_id = [0; 8];
        for (i, part) in sid.as_bytes().as_chunks::<2>().0.iter().enumerate() {
            let text = std::str::from_utf8(part).map_err(|_| Error::InvalidInput)?;
            short_id[i] = u8::from_str_radix(text, 16).map_err(|_| Error::InvalidInput)?;
        }
        Ok(Node {
            host,
            port,
            sni,
            public_key,
            short_id,
            uuid,
            fingerprint,
        })
    }
}
fn uuid(value: &str) -> Result<[u8; 16]> {
    if value.len() != 36 || ![8, 13, 18, 23].iter().all(|i| value.as_bytes()[*i] == b'-') {
        return Err(Error::InvalidInput);
    }
    let digits: String = value.chars().filter(|c| *c != '-').collect();
    let mut result = [0; 16];
    for (i, bytes) in digits.as_bytes().as_chunks::<2>().0.iter().enumerate() {
        result[i] = u8::from_str_radix(
            std::str::from_utf8(bytes).map_err(|_| Error::InvalidInput)?,
            16,
        )
        .map_err(|_| Error::InvalidInput)?;
    }
    Ok(result)
}
