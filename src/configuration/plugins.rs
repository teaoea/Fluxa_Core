//! 声明式插件只携带 MITM、复写及一个命名脚本，不获得路由表或宿主资源控制权。
use super::{
    Document,
    models::{Mitm, ScriptSource},
    rewrites::Rule,
};
use crate::{
    budget::{ByteBudget, ByteLease},
    error::{Error, Result},
    script::worker::Bindings,
};
use serde::Deserialize;
use std::sync::Arc;
const MAX_PLUGIN: usize = 1024 * 1024;
static MEMORY: ByteBudget = ByteBudget::new(128 * 1024 * 1024);
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Payload {
    #[serde(default)]
    mitm: Mitm,
    #[serde(default)]
    rewrites: Vec<Rule>,
    #[serde(default, deserialize_with = "super::models::optional_script")]
    scripts: Option<ScriptSource>,
}
pub(crate) struct Plugin {
    pub(crate) id: u64,
    pub(super) rules: Vec<Rule>,
    pub(crate) bindings: Arc<Bindings>,
    mitm: Mitm,
    _memory: ByteLease,
}
#[derive(Default)]
pub(crate) struct Bundle {
    pub(crate) plugins: Vec<Plugin>,
    pub(crate) slots: std::collections::BTreeMap<String, Option<i32>>,
}
impl Bundle {
    pub(crate) fn has_rules(&self) -> bool {
        self.plugins.iter().any(|p| !p.rules.is_empty())
    }
    /// 主配置排除列表先决；插件不能越过关闭的总开关或覆盖主配置禁止解密的目标。
    pub(crate) fn decrypts(&self, main: &Document, host: &str) -> bool {
        let main = &main.raw.mitm;
        if !main.enabled || matches_host(&main.exclude_hosts, host) {
            return false;
        }
        matches_host(&main.hosts, host)
            || self.plugins.iter().any(|p| {
                p.mitm.enabled
                    && !matches_host(&p.mitm.exclude_hosts, host)
                    && matches_host(&p.mitm.hosts, host)
            })
    }
}
fn matches_host(patterns: &[String], host: &str) -> bool {
    patterns
        .iter()
        .any(|p| super::host_pattern_matches(p, host))
}

fn payload(text: &str, main: &Document) -> Result<(Payload, ByteLease)> {
    if text.len() > MAX_PLUGIN {
        return Err(Error::ResourceLimit);
    }
    let memory = MEMORY.reserve(
        text.len()
            .checked_mul(64)
            .and_then(|v| v.checked_add(65536))
            .ok_or(Error::ResourceLimit)?,
    )?;
    let mut parsed: Payload = crate::routing::runtime::yaml::decode(text)?;
    super::validate_patterns(&parsed.mitm.hosts, false)?;
    super::validate_patterns(&parsed.mitm.exclude_hosts, false)?;
    if parsed.mitm.hosts.len() > 128
        || parsed.mitm.exclude_hosts.len() > 128
        || parsed.rewrites.len() > 1024
    {
        return Err(Error::ResourceLimit);
    }
    // 插件不能关闭服务器证书验证或擅自开启正文捕获；这些权限只由主配置/授权入口决定。
    if parsed.mitm.skip_server_cert_verify || parsed.mitm.capture_body {
        return Err(Error::InvalidInput);
    }
    if let Some(source) = &parsed.scripts {
        super::script_name(&source.name)?;
        if let Some(url) = &source.url {
            super::https_url(url)?;
        }
    }
    for rule in &mut parsed.rewrites {
        rule.validate()?;
        if let Some(node) = rule.proxy_node()
            && !main.raw.proxies.iter().any(|p| p.name == node)
        {
            return Err(Error::InvalidInput);
        }
    }
    Ok((parsed, memory))
}

pub(crate) struct Assets {
    pub(crate) bundle: Arc<Bundle>,
    pub(crate) scripts: Arc<Bindings>,
}
pub(crate) fn load_local(
    main: &Arc<Document>,
    resources: &crate::storage::Resources,
) -> Result<Assets> {
    resources.validate()?;
    let active: Vec<_> = main
        .raw
        .plugin_providers
        .iter()
        .filter(|provider| provider.enabled)
        .collect();
    if active.len() != resources.plugins.len() {
        return Err(Error::InvalidInput);
    }
    let scripts = Bindings::from_file(
        main.raw.scripts.as_ref().map(|s| s.name.as_str()),
        resources.script.as_ref(),
        main.raw
            .rewrites
            .iter()
            .filter_map(Rule::script_reference)
            .collect(),
    )?;
    let mut bundle = Bundle::default();
    for provider in active {
        let file = resources
            .plugins
            .iter()
            .find(|file| file.name == provider.name)
            .ok_or(Error::InvalidInput)?;
        let text = crate::storage::ReadOnlyFile::open(&file.path)?.read(MAX_PLUGIN)?;
        let (parsed, memory) = payload(&text, main)?;
        let bindings = Bindings::from_file(
            parsed.scripts.as_ref().map(|s| s.name.as_str()),
            file.script.as_ref(),
            parsed
                .rewrites
                .iter()
                .filter_map(Rule::script_reference)
                .collect(),
        )?;
        bundle.slots.insert(provider.name.clone(), None);
        bundle.plugins.push(Plugin {
            id: file.id,
            rules: parsed.rewrites,
            bindings,
            mitm: parsed.mitm,
            _memory: memory,
        });
    }
    bundle.plugins.sort_by_key(|plugin| plugin.id);
    Ok(Assets {
        bundle: Arc::new(bundle),
        scripts,
    })
}
#[cfg(test)]
#[path = "../../tests/test_configuration_plugins.rs"]
mod test_configuration_plugins;
