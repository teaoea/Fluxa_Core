//! 流量脚本只从编译进内核的注册表选择；配置不携带源码或远程脚本地址。
use crate::{
    error::{Error, Result},
    script,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::BTreeSet;

pub const MAX_BODY: usize = 256 * 1024;

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Script {
    RemoveJsonKeys,
}
impl Script {
    pub fn validate(self, keys: &[String]) -> Result<()> {
        if keys.is_empty() || keys.len() > 64 {
            return Err(Error::InvalidInput);
        }
        let mut distinct = BTreeSet::new();
        for key in keys {
            if key.is_empty()
                || key.len() > 256
                || key.chars().any(char::is_control)
                || !distinct.insert(key)
            {
                return Err(Error::InvalidInput);
            }
        }
        Ok(())
    }

    /// 只移除 JSON 根对象上明确指定的键，不递归推断广告字段，不修改相邻内容。
    pub fn execute(self, original: &[u8], keys: &[String]) -> Result<Vec<u8>> {
        self.validate(keys)?;
        if original.len() > MAX_BODY {
            return Err(Error::ResourceLimit);
        }
        let mut body: Value = serde_json::from_slice(original).map_err(|_| Error::InvalidInput)?;
        let object = body.as_object_mut().ok_or(Error::Unsupported)?;
        let fields: Vec<&String> = object.keys().collect();
        let source = match self {
            Self::RemoveJsonKeys => "input => input.fields.filter(key => input.keys.includes(key))",
        };
        let selected: Vec<String> = serde_json::from_value(script::execute(
            source,
            &json!({"fields": fields, "keys": keys}),
        )?)
        .map_err(|_| Error::Script)?;
        if selected.is_empty() {
            return Ok(original.to_vec());
        }
        for key in selected {
            if !keys.contains(&key) {
                return Err(Error::Script);
            }
            object.remove(&key);
        }
        let bytes = serde_json::to_vec(&body).map_err(|_| Error::Script)?;
        if bytes.len() > MAX_BODY {
            return Err(Error::ResourceLimit);
        }
        Ok(bytes)
    }
}
