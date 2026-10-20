//! 声明式插件只携带 MITM、复写及一个命名脚本，不获得路由表或宿主资源控制权。
use super::{
    Document,
    models::{Mitm, ScriptSource},
    rewrites::Rule,
};
use crate::{
    budget::{ByteBudget, ByteLease},
    error::{Error, Result},
    platform::storage::Directory,
    script::worker::Bindings,
};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};
const MAX_PLUGIN: usize = 1024 * 1024;
const MAX_ASSETS: usize = 16 * 1024 * 1024;
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
#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct ScriptRecord {
    name: String,
    url_hash: String,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    name: String,
    url: String,
    id: u64,
    slot: Option<i32>,
    script: Option<ScriptRecord>,
}
#[derive(Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Catalog {
    next: u64,
    entries: Vec<Entry>,
    main_script: Option<ScriptRecord>,
}
fn hash(value: &str) -> String {
    ring::digest::digest(&ring::digest::SHA256, value.as_bytes())
        .as_ref()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn catalog(root: &Path, create: bool) -> Result<Catalog> {
    let directory = Directory::open(root, "plugin", create)?;
    let raw = match directory.read_bounded(".catalog.json", 256 * 1024) {
        Ok(v) => v,
        Err(Error::Network) if !root.join("plugin/.catalog.json").try_exists()? => {
            return Ok(Catalog::default());
        }
        Err(e) => return Err(e),
    };
    let state: Catalog = crate::routing::runtime::yaml::decode(&raw)?;
    if state.entries.len() > 128 {
        return Err(Error::ResourceLimit);
    }
    let mut ids = std::collections::BTreeSet::new();
    let mut names = std::collections::BTreeSet::new();
    for item in &state.entries {
        super::name(&item.name)?;
        if let Some(script) = &item.script {
            super::script_name(&script.name)?;
            if script.url_hash.len() != 64
                || !script.url_hash.bytes().all(|b| b.is_ascii_hexdigit())
            {
                return Err(Error::InvalidInput);
            }
        }
        if item.id == 0
            || item.id > state.next
            || !ids.insert(item.id)
            || !names.insert(&item.name)
            || (!item.url.is_empty()
                && (item.url.len() != 64 || !item.url.bytes().all(|b| b.is_ascii_hexdigit())))
            || (item.url.is_empty() && (item.slot.is_some() || item.script.is_some()))
            || item
                .slot
                .is_some_and(|s| !(19700101..=99991231).contains(&s))
        {
            return Err(Error::InvalidInput);
        }
    }
    Ok(state)
}
fn record(error: Error) {
    crate::error::diagnostics::record(
        error,
        crate::error::diagnostics::DiagnosticSite::BackgroundJob,
    );
}
/// 先复制到私有候选目录，再绑定其 FD；未提交批次不改变运行中的目录或规则。
pub(crate) struct Batch {
    root: PathBuf,
    stage: PathBuf,
    lock: fs::File,
    plugin_stage: PathBuf,
    js_stage: PathBuf,
    plugin_swapped: bool,
    js_swapped: bool,
    accepted: bool,
    journal: bool,
    pub(crate) bundle: Arc<Bundle>,
    pub(crate) scripts: Arc<Bindings>,
    _memory: ByteLease,
}
impl Batch {
    fn new(root: &Path) -> Result<Self> {
        let memory = MEMORY.reserve(MAX_ASSETS)?;
        Directory::open(root, "plugin", true)?;
        Directory::open(root, "js", true)?;
        // 锁目录不参与交换，候选全过程持有跨进程独占权。
        let lock = Directory::open(root, ".assets-lock", true)?.lock()?;
        recover(root)?;
        use ring::rand::SecureRandom;
        let mut nonce = [0u8; 16];
        ring::rand::SystemRandom::new()
            .fill(&mut nonce)
            .map_err(|_| Error::Internal)?;
        let generation = nonce.iter().map(|b| format!("{b:02x}")).collect::<String>();
        let stage = root.join(format!(".assets-stage-{generation}"));
        let mut builder = fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder.create(&stage)?;
        let value = Self {
            root: root.into(),
            plugin_stage: stage.join("plugin"),
            js_stage: stage.join("js"),
            stage,
            lock,
            plugin_swapped: false,
            js_swapped: false,
            accepted: false,
            journal: false,
            bundle: Arc::default(),
            scripts: Arc::default(),
            _memory: memory,
        };
        let mut remaining = MAX_ASSETS;
        copy_tree(
            &root.join("plugin"),
            &value.stage.join("plugin"),
            2,
            &mut remaining,
        )?;
        copy_tree(&root.join("js"), &value.stage.join("js"), 1, &mut remaining)?;
        Ok(value)
    }
    pub(crate) fn commit(&mut self) -> Result<()> {
        if self.accepted || self.plugin_swapped || self.js_swapped {
            return Err(Error::InvalidState);
        }
        let generation = self
            .stage
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or(Error::InvalidInput)?
            .to_string();
        for (parent, child) in [
            (self.stage.as_path(), "plugin"),
            (self.stage.as_path(), "js"),
        ] {
            if !Directory::open(parent, child, false)?.write(
                ".generation",
                generation.as_bytes(),
                true,
            )? {
                return Err(Error::Network);
            }
        }
        let mut bytes = 0;
        sync_tree(&self.plugin_stage, 3, &mut bytes)?;
        sync_tree(&self.js_stage, 1, &mut bytes)?;
        self.scripts.pin()?;
        for plugin in &self.bundle.plugins {
            plugin.bindings.pin()?;
        }

        if self.accepted || self.plugin_swapped || self.js_swapped {
            return Err(Error::InvalidState);
        }
        let journal =
            serde_json::to_vec(&Transaction { stage: generation }).map_err(|_| Error::Internal)?;
        if !Directory::open(&self.root, ".assets-lock", false)?.write(
            "transaction.json",
            &journal,
            true,
        )? {
            return Err(Error::Network);
        }
        self.journal = true;
        if self.plugin_stage.parent() == Some(self.stage.as_path()) {
            let promoted = self.root.join(format!(
                "{}.plugin",
                self.stage
                    .file_name()
                    .and_then(|v| v.to_str())
                    .ok_or(Error::InvalidInput)?
            ));
            fs::rename(&self.plugin_stage, &promoted)?;
            self.plugin_stage = promoted;
        }
        if self.js_stage.parent() == Some(self.stage.as_path()) {
            let promoted = self.root.join(format!(
                "{}.js",
                self.stage
                    .file_name()
                    .and_then(|v| v.to_str())
                    .ok_or(Error::InvalidInput)?
            ));
            fs::rename(&self.js_stage, &promoted)?;
            self.js_stage = promoted;
        }
        fs::File::open(&self.root)?.sync_all()?;
        crate::platform::providers::exchange(&self.plugin_stage, &self.root.join("plugin"))?;
        self.plugin_swapped = true;
        if let Err(error) =
            crate::platform::providers::exchange(&self.js_stage, &self.root.join("js"))
        {
            self.rollback()?;
            return Err(error);
        }
        self.js_swapped = true;
        Ok(())
    }
    pub(crate) fn rollback(&mut self) -> Result<()> {
        if self.js_swapped {
            crate::platform::providers::exchange(&self.js_stage, &self.root.join("js"))?;
            self.js_swapped = false;
        }
        if self.plugin_swapped {
            crate::platform::providers::exchange(&self.plugin_stage, &self.root.join("plugin"))?;
            self.plugin_swapped = false;
        }
        fs::File::open(&self.root)?.sync_all()?;
        Ok(())
    }
    pub(crate) fn accept(&mut self) {
        self.accepted = true;
    }
    pub(crate) fn sync(&self) -> Result<()> {
        fs::File::open(&self.root)?.sync_all()?;
        Ok(())
    }
}
impl Drop for Batch {
    fn drop(&mut self) {
        if !self.accepted
            && let Err(e) = self.rollback()
        {
            record(e);
            return;
        }
        for path in [&self.plugin_stage, &self.js_stage] {
            if path.parent() != Some(self.stage.as_path())
                && let Err(error) = fs::remove_dir_all(path)
            {
                record(error.into());
            }
        }
        if let Err(e) = fs::remove_dir_all(&self.stage) {
            record(e.into());
        }
        if self.journal
            && let Err(error) = fs::remove_file(self.root.join(".assets-lock/transaction.json"))
        {
            record(error.into());
        }
        if let Err(e) = self.lock.unlock() {
            record(e.into());
        }
    }
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Transaction {
    stage: String,
}
fn stage_name(value: &str) -> bool {
    value.strip_prefix(".assets-stage-").is_some_and(|s| {
        (s.len() == 32 && s.bytes().all(|b| b.is_ascii_hexdigit()))
            || (!s.is_empty() && s.len() <= 20 && s.bytes().all(|b| b.is_ascii_digit()))
    })
}

fn generation(root: &Path, child: &str) -> Result<Option<String>> {
    let directory = Directory::open(root, child, false)?;
    match directory.read_bounded(".generation", 128) {
        Ok(value) => Ok(Some(value.to_string())),
        Err(Error::Network) if !root.join(child).join(".generation").try_exists()? => Ok(None),
        Err(error) => Err(error),
    }
}
fn remove_owned(path: &Path) -> Result<()> {
    if !path.try_exists()? {
        return Ok(());
    }
    let m = fs::symlink_metadata(path)?;
    if !m.is_dir() || m.file_type().is_symlink() {
        return Err(Error::InvalidInput);
    }
    fs::remove_dir_all(path)?;
    Ok(())
}
/// 仅在持有根级协调锁时恢复。单边交换回滚，双边完整交换可作为已下载缓存复用；不推断 VPN 运行状态。
fn recover(root: &Path) -> Result<()> {
    let journal = root.join(".assets-lock/transaction.json");
    if journal.try_exists()? {
        let text =
            Directory::open(root, ".assets-lock", false)?.read_bounded("transaction.json", 1024)?;
        let transaction: Transaction = crate::routing::runtime::yaml::decode(&text)?;
        if !stage_name(&transaction.stage) {
            return Err(Error::InvalidInput);
        }
        let plugin = generation(root, "plugin")?.as_deref() == Some(transaction.stage.as_str());
        let js = generation(root, "js")?.as_deref() == Some(transaction.stage.as_str());
        if plugin != js {
            let child = if plugin { "plugin" } else { "js" };
            let candidate = root.join(format!("{}.{child}", transaction.stage));
            let meta = fs::symlink_metadata(&candidate)?;
            if !meta.is_dir() || meta.file_type().is_symlink() {
                return Err(Error::InvalidInput);
            }
            crate::platform::providers::exchange(&candidate, &root.join(child))?;
        }
        fs::File::open(root)?.sync_all()?;
        fs::remove_file(journal)?;
    }
    // 日志在任何交换前持久化；没有日志的内部候选只能是未交换或已完成清理的残留。
    let mut pending = 0;
    let mut visited = 0;
    for entry in fs::read_dir(root)? {
        visited += 1;
        if visited > 2048 {
            return Err(Error::ResourceLimit);
        }
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_str().ok_or(Error::InvalidInput)?;
        let base = name
            .strip_suffix(".plugin")
            .or_else(|| name.strip_suffix(".js"))
            .unwrap_or(name);
        if stage_name(base) {
            pending += 1;
            if pending > 128 {
                return Err(Error::ResourceLimit);
            }
            remove_owned(&entry.path())?;
        }
    }
    fs::File::open(root)?.sync_all()?;
    Ok(())
}
fn sync_tree(path: &Path, depth: usize, bytes: &mut u64) -> Result<()> {
    if depth == 0 {
        return Err(Error::InvalidInput);
    }
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(Error::InvalidInput);
    }
    let mut count = 0;
    for entry in fs::read_dir(path)? {
        count += 1;
        if count > 256 {
            return Err(Error::ResourceLimit);
        }
        let entry = entry?;
        let m = fs::symlink_metadata(entry.path())?;
        if m.file_type().is_symlink() {
            return Err(Error::InvalidInput);
        }
        if m.is_dir() {
            sync_tree(&entry.path(), depth - 1, bytes)?;
        } else if m.is_file() {
            *bytes = bytes
                .checked_add(m.len())
                .filter(|n| *n <= MAX_ASSETS as u64)
                .ok_or(Error::ResourceLimit)?;
        } else {
            return Err(Error::InvalidInput);
        }
    }
    fs::File::open(path)?.sync_all()?;
    Ok(())
}
fn copy_tree(from: &Path, to: &Path, depth: usize, remaining: &mut usize) -> Result<()> {
    if depth == 0 {
        return Err(Error::InvalidInput);
    }
    let meta = fs::symlink_metadata(from)?;
    if !meta.is_dir() || meta.file_type().is_symlink() {
        return Err(Error::InvalidInput);
    }
    let mut builder = fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(to)?;
    let parent = from.parent().ok_or(Error::InvalidInput)?;
    let child = from
        .file_name()
        .and_then(|v| v.to_str())
        .ok_or(Error::InvalidInput)?;
    let source = Directory::open(parent, child, false)?;
    let destination = Directory::open(
        to.parent().ok_or(Error::InvalidInput)?,
        to.file_name()
            .and_then(|v| v.to_str())
            .ok_or(Error::InvalidInput)?,
        false,
    )?;
    let mut count = 0;
    for entry in fs::read_dir(from)? {
        count += 1;
        if count > 256 {
            return Err(Error::ResourceLimit);
        }
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_str().ok_or(Error::InvalidInput)?;
        let kind = entry.file_type()?;
        if kind.is_symlink() {
            return Err(Error::InvalidInput);
        }
        if kind.is_dir() {
            copy_tree(&entry.path(), &to.join(name), depth - 1, remaining)?;
        } else if kind.is_file() {
            // 内部锁不随公开资源目录复制。
            if name == ".configuration.lock" {
                continue;
            }
            let text = source.read_bounded(name, MAX_PLUGIN)?;
            *remaining = remaining
                .checked_sub(text.len())
                .ok_or(Error::ResourceLimit)?;
            destination.write(name, text.as_bytes(), false)?;
        } else {
            return Err(Error::InvalidInput);
        }
    }
    Ok(())
}
async fn fetch(client: &crate::https::HttpsClient, url: &str, limit: usize) -> Result<String> {
    let response = crate::subscription::fetch_limited(client, url, limit).await?;
    if response.body.len() > limit {
        return Err(Error::ResourceLimit);
    }
    String::from_utf8(response.body).map_err(|_| Error::InvalidInput)
}
async fn script(
    client: &crate::https::HttpsClient,
    source: Option<&ScriptSource>,
    previous: Option<&ScriptRecord>,
    root: &Path,
    child: &str,
    force: bool,
) -> Result<Option<ScriptRecord>> {
    let Some(source) = source else {
        return Ok(None);
    };
    let Some(url) = &source.url else {
        return Ok(None);
    };
    let key = ScriptRecord {
        name: source.name.clone(),
        url_hash: hash(url),
    };
    let directory = Directory::open(root, child, true)?;
    if force
        || previous != Some(&key)
        || directory
            .require_file(&source.name, crate::script::MAX_SOURCE)
            .is_err()
    {
        match fetch(client, url, crate::script::MAX_SOURCE).await {
            Ok(text) => {
                directory.write(&source.name, text.as_bytes(), true)?;
            }
            Err(error) => {
                record(error);
                let target = root.join(child).join(&source.name);
                if target.try_exists()? {
                    fs::remove_file(target)?;
                }
                return Ok(None);
            }
        }
    }
    Ok(Some(key))
}
/// 配置编译每次重新查询真实容器；下载、候选目录与发布都由内核独立完成。
pub(crate) struct Assets {
    batch: Option<Batch>,
    pub(crate) bundle: Arc<Bundle>,
    pub(crate) scripts: Arc<Bindings>,
}
impl Assets {
    pub(crate) fn commit(&mut self) -> Result<()> {
        self.batch.as_mut().map_or(Ok(()), Batch::commit)
    }
    pub(crate) fn rollback(&mut self) -> Result<()> {
        self.batch.as_mut().map_or(Ok(()), Batch::rollback)
    }
    pub(crate) fn accept(&mut self) {
        if let Some(batch) = &mut self.batch {
            batch.accept();
        }
    }
    pub(crate) fn sync(&self) -> Result<()> {
        self.batch.as_ref().map_or(Ok(()), Batch::sync)
    }
}
pub(crate) async fn load(
    main: &Arc<Document>,
    client: &crate::https::HttpsClient,
    home: Option<&Path>,
    force: bool,
) -> Result<Assets> {
    let context = crate::platform::storage::context()?;
    if let Some(home) = home
        && fs::canonicalize(home)? != fs::canonicalize(&context.root)?
    {
        return Err(Error::InvalidState);
    }
    // 扩展及未发布 VPN 候选只能复用完整本地资源，不下载、不创建目录、不更新调度元数据。
    if context.read_only_configuration || home.is_none() {
        return read_only(main, &context.root);
    }
    let batch = load_in(main, client, &context.root, force).await?;
    Ok(Assets {
        bundle: batch.bundle.clone(),
        scripts: batch.scripts.clone(),
        batch: Some(batch),
    })
}
fn read_only(main: &Document, root: &Path) -> Result<Assets> {
    let _lock = Directory::open(root, ".assets-lock", false)?.lock_existing()?;
    if root.join(".assets-lock/transaction.json").try_exists()? {
        return Err(Error::InvalidState);
    }
    let catalog = catalog(root, false)?;
    let mut bundle = Bundle::default();
    let mut order = Vec::new();
    for provider in &main.raw.plugin_providers {
        if !provider.enabled {
            continue;
        }
        let entry = catalog
            .entries
            .iter()
            .find(|e| e.name == provider.name && e.url == hash(&provider.url))
            .ok_or(Error::InvalidState)?;
        order.push(entry.id);
        let child = entry.id.to_string();
        let directory = Directory::open(&root.join("plugin"), &child, false)?;
        let text = directory.read_bounded(".plugin.yaml", MAX_PLUGIN)?;
        let (parsed, memory) = payload(&text, main)?;
        let declared = verified_name(parsed.scripts.as_ref(), entry.script.as_ref());
        let bindings = Bindings::in_directory(
            &root.join("plugin"),
            &child,
            &root.join("plugin").join(&child),
            declared,
            parsed
                .rewrites
                .iter()
                .filter_map(Rule::script_reference)
                .collect(),
        )?;
        bundle.slots.insert(provider.name.clone(), entry.slot);
        bundle.plugins.push(Plugin {
            id: entry.id,
            rules: parsed.rewrites,
            bindings,
            mitm: parsed.mitm,
            _memory: memory,
        });
    }
    // 候选不能通过只改变配置顺序、尚未执行目录事务而声称新的 ID 优先级已生效。
    if !order.windows(2).all(|pair| pair[0] < pair[1]) {
        return Err(Error::InvalidState);
    }
    let scripts = Bindings::in_directory(
        root,
        "js",
        &root.join("js"),
        verified_name(main.raw.scripts.as_ref(), catalog.main_script.as_ref()),
        main.raw
            .rewrites
            .iter()
            .filter_map(Rule::script_reference)
            .collect(),
    )?;
    scripts.pin()?;
    for plugin in &bundle.plugins {
        plugin.bindings.pin()?;
    }
    Ok(Assets {
        batch: None,
        bundle: Arc::new(bundle),
        scripts,
    })
}
fn verified_name<'a>(
    source: Option<&'a ScriptSource>,
    previous: Option<&ScriptRecord>,
) -> Option<&'a str> {
    let source = source?;
    if let Some(url) = &source.url
        && previous
            != Some(&ScriptRecord {
                name: source.name.clone(),
                url_hash: hash(url),
            })
    {
        record(Error::InvalidState);
        return None;
    }
    Some(&source.name)
}
async fn load_in(
    main: &Arc<Document>,
    client: &crate::https::HttpsClient,
    root: &Path,
    force: bool,
) -> Result<Batch> {
    let mut batch = Batch::new(root)?;
    let mut catalog = catalog(root, true)?;
    let mut selected = Vec::new();
    for provider in &main.raw.plugin_providers {
        let index =
            if let Some(index) = catalog.entries.iter().position(|p| p.name == provider.name) {
                index
            } else {
                if catalog.entries.len() >= 128 {
                    return Err(Error::ResourceLimit);
                }
                catalog.next = catalog.next.checked_add(1).ok_or(Error::ResourceLimit)?;
                catalog.entries.push(Entry {
                    name: provider.name.clone(),
                    url: String::new(),
                    id: catalog.next,
                    slot: None,
                    script: None,
                });
                catalog.entries.len() - 1
            };
        selected.push(index);
    }
    // ID 是可调整的插件显示/执行顺序；不参与正式路由规则类别算法。
    let mut ids = selected
        .iter()
        .map(|i| catalog.entries[*i].id)
        .collect::<Vec<_>>();
    ids.sort_unstable();
    let old_ids = selected
        .iter()
        .map(|i| catalog.entries[*i].id)
        .collect::<Vec<_>>();
    // 先从旧目录保留各插件独立副本，避免交换 1/2 时相互覆盖。
    let mut remaining = MAX_ASSETS;
    for (order, index) in selected.iter().enumerate() {
        let from = root.join("plugin").join(old_ids[order].to_string());
        let target = batch.stage.join(format!("move-{index}"));
        if from.try_exists()? {
            copy_tree(&from, &target, 1, &mut remaining)?;
        } else {
            Directory::open(&batch.stage, &format!("move-{index}"), true)?;
        }
    }
    for id in &old_ids {
        let path = batch.stage.join("plugin").join(id.to_string());
        if path.try_exists()? {
            fs::remove_dir_all(path)?;
        }
    }
    let mut bundle = Bundle::default();
    let at = crate::providers::now()?;
    for (order, index) in selected.into_iter().enumerate() {
        let provider = &main.raw.plugin_providers[order];
        let entry = &mut catalog.entries[index];
        entry.id = ids[order];
        let child = entry.id.to_string();
        let path = batch.stage.join("plugin").join(&child);
        fs::rename(batch.stage.join(format!("move-{index}")), &path)?;
        if !provider.enabled {
            continue;
        }
        let directory = Directory::open(&batch.stage.join("plugin"), &child, false)?;
        let current_slot = crate::providers::slot(provider.update_time.as_deref(), at)?;
        let download = force
            || entry.url != hash(&provider.url)
            || (current_slot.is_some() && entry.slot != current_slot)
            || directory.require_file(".plugin.yaml", MAX_PLUGIN).is_err();
        let text = if download {
            fetch(client, &provider.url, MAX_PLUGIN).await?
        } else {
            directory
                .read_bounded(".plugin.yaml", MAX_PLUGIN)?
                .to_string()
        };
        let (parsed, memory) = payload(&text, main)?;
        entry.script = script(
            client,
            parsed.scripts.as_ref(),
            entry.script.as_ref(),
            &batch.stage.join("plugin"),
            &child,
            download,
        )
        .await?;
        if download {
            directory.write(".plugin.yaml", text.as_bytes(), true)?;
            entry.url = hash(&provider.url);
            entry.slot = current_slot;
        }
        let refs = parsed
            .rewrites
            .iter()
            .filter_map(Rule::script_reference)
            .collect();
        let bindings = Bindings::in_directory(
            &batch.stage.join("plugin"),
            &child,
            &root.join("plugin").join(&child),
            parsed.scripts.as_ref().map(|s| s.name.as_str()),
            refs,
        )?;
        bundle.slots.insert(provider.name.clone(), entry.slot);
        bundle.plugins.push(Plugin {
            id: entry.id,
            rules: parsed.rewrites,
            bindings,
            mitm: parsed.mitm,
            _memory: memory,
        });
    }
    catalog.main_script = script(
        client,
        main.raw.scripts.as_ref(),
        catalog.main_script.as_ref(),
        &batch.stage,
        "js",
        force,
    )
    .await?;
    batch.scripts = Bindings::in_directory(
        &batch.stage,
        "js",
        &root.join("js"),
        main.raw.scripts.as_ref().map(|s| s.name.as_str()),
        main.raw
            .rewrites
            .iter()
            .filter_map(Rule::script_reference)
            .collect(),
    )?;
    Directory::open(&batch.stage, "plugin", false)?.write(
        ".catalog.json",
        &serde_json::to_vec(&catalog).map_err(|_| Error::Internal)?,
        true,
    )?;
    bundle.plugins.sort_by_key(|p| p.id);
    batch.bundle = Arc::new(bundle);
    Ok(batch)
}

#[cfg(test)]
#[path = "../../tests/test_configuration_plugins.rs"]
mod test_configuration_plugins;

impl Document {
    pub(crate) fn assets_due(
        &self,
        previous: &std::collections::BTreeMap<String, Option<i32>>,
        at: u64,
    ) -> Result<bool> {
        for provider in self.raw.plugin_providers.iter().filter(|p| p.enabled) {
            if let Some(slot) = crate::providers::slot(provider.update_time.as_deref(), at)?
                && previous.get(&provider.name).copied().flatten() != Some(slot)
            {
                return Ok(true);
            }
        }
        Ok(false)
    }
}
