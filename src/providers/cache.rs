//! 提供者缓存是整目录事务：所有 YAML 和来源/调度元数据先校验，再一次交换目录。
use crate::{
    budget::{ByteBudget, ByteLease},
    error::{Error, Result},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::Arc,
};
pub(super) const BODY_LIMIT: usize = 8 * 1024 * 1024;
const BODY_MEMORY: usize = 128 * 1024 * 1024;
const STATE_LIMIT: usize = 256 * 1024;
const MAX_ENTRIES: usize = 64;
const MAX_ATTEMPTS: usize = 128;
const STATE_FILE: &str = ".state.json";
static MEMORY: ByteBudget = ByteBudget::new(BODY_MEMORY);
pub(super) struct Text {
    pub text: String,
    _memory: ByteLease,
}
impl Text {
    pub fn new(text: String) -> Result<Arc<Self>> {
        if text.len() > BODY_LIMIT {
            return Err(Error::ResourceLimit);
        }
        let memory = MEMORY.reserve(
            text.capacity()
                .checked_add(128)
                .ok_or(Error::ResourceLimit)?,
        )?;
        Ok(Arc::new(Self {
            text,
            _memory: memory,
        }))
    }
}
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(super) struct Metadata {
    pub url_hash: String,
    pub digest: String,
    pub fetched_at: u64,
    pub last_slot: Option<i32>,
}
#[derive(Clone, Default)]
pub(super) struct Data {
    pub entries: BTreeMap<String, (Metadata, Arc<Text>)>,
    pub attempts: BTreeMap<String, u64>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    version: u32,
    entries: BTreeMap<String, Metadata>,
    #[serde(
        serialize_with = "serialize_attempts",
        deserialize_with = "deserialize_attempts"
    )]
    attempts: BTreeMap<String, u64>,
}
// 映射字段门禁仍是 64，不为内部日志放宽全局 YAML 校验；128 条记录使用有界结构序列。
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Attempt {
    key: String,
    time: u64,
}
fn serialize_attempts<S: serde::Serializer>(
    values: &BTreeMap<String, u64>,
    serializer: S,
) -> std::result::Result<S::Ok, S::Error> {
    values
        .iter()
        .map(|(key, time)| Attempt {
            key: key.clone(),
            time: *time,
        })
        .collect::<Vec<_>>()
        .serialize(serializer)
}
fn attempt_records(records: Vec<Attempt>) -> Result<BTreeMap<String, u64>> {
    if records.len() > MAX_ATTEMPTS {
        return Err(Error::ResourceLimit);
    }
    let mut values = BTreeMap::new();
    for Attempt { key, time } in records {
        if !digest_valid(&key) || time > 253402300799 || values.insert(key, time).is_some() {
            return Err(Error::InvalidInput);
        }
    }
    Ok(values)
}
fn deserialize_attempts<'de, D: serde::Deserializer<'de>>(
    decoder: D,
) -> std::result::Result<BTreeMap<String, u64>, D::Error> {
    let records = Vec::<Attempt>::deserialize(decoder)?;
    attempt_records(records).map_err(serde::de::Error::custom)
}
pub(super) fn hash(bytes: &[u8]) -> String {
    ring::digest::digest(&ring::digest::SHA256, bytes)
        .as_ref()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn digest_valid(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn read(path: &Path, maximum: usize) -> Result<String> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(Error::InvalidInput);
    }
    if metadata.len() > maximum as u64 {
        return Err(Error::ResourceLimit);
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let file = options.open(path)?;
    if !file.metadata()?.is_file() || file.metadata()?.len() > maximum as u64 {
        return Err(Error::ResourceLimit);
    }
    let mut value = String::new();
    file.take((maximum + 1) as u64).read_to_string(&mut value)?;
    if value.len() > maximum {
        return Err(Error::ResourceLimit);
    }
    Ok(value)
}
fn directory(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(m) if !m.is_dir() || m.file_type().is_symlink() => return Err(Error::InvalidInput),
        Ok(_) => (),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let mut b = fs::DirBuilder::new();
            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt;
                b.mode(0o700);
            }
            b.create(path)?;
        }
        Err(e) => return Err(e.into()),
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let m = fs::metadata(path)?;
        if m.mode() & 0o022 != 0 {
            return Err(Error::InvalidInput);
        }
    }
    Ok(())
}
pub(super) struct Cache {
    parent: PathBuf,
    pub directory: PathBuf,
    lock: Option<File>,
}
impl Cache {
    pub fn open(home: &Path) -> Result<(Self, Data)> {
        if !home.is_absolute() || home.as_os_str().len() > 4000 {
            return Err(Error::InvalidInput);
        }
        // 用户目录必须可信；只创建自己管理的两级目录，不递归穿越外部符号链接。
        let home = fs::canonicalize(home)?;
        let parent = home.join(".fluxa");
        directory(&parent)?;
        let path = parent.join(".rule-providers.lock");
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options
                .mode(0o600)
                .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
        }
        let lock = options.open(&path)?;
        if !lock.metadata()?.is_file() {
            return Err(Error::InvalidInput);
        }
        lock.try_lock().map_err(|e| match e {
            fs::TryLockError::WouldBlock => Error::InvalidState,
            fs::TryLockError::Error(e) => e.into(),
        })?;
        let path = parent.join("rules");
        directory(&path)?;
        // 独占租约后清理中断留下的内部暂存目录；不接触公开 YAML 名称或外部路径。
        for entry in fs::read_dir(&parent)? {
            let entry = entry?;
            if entry
                .file_name()
                .to_str()
                .is_some_and(|n| n.starts_with(".rules-stage-"))
            {
                let m = entry.file_type()?;
                if !m.is_dir() || m.is_symlink() {
                    return Err(Error::InvalidInput);
                }
                fs::remove_dir_all(entry.path())?;
            } else if entry
                .file_name()
                .to_str()
                .is_some_and(|n| n.starts_with(".attempt-stage-"))
            {
                if !entry.file_type()?.is_file() {
                    return Err(Error::InvalidInput);
                }
                fs::remove_file(entry.path())?;
            }
        }
        if !path.join(STATE_FILE).try_exists()? && fs::read_dir(&path)?.next().is_some() {
            return Err(Error::InvalidState);
        }
        let mut data = match read(&path.join(STATE_FILE), STATE_LIMIT) {
            Ok(value) => {
                let manifest: Manifest = crate::routing::runtime::yaml::decode(&value)?;
                if manifest.version != 1 {
                    return Err(Error::Unsupported);
                }
                if manifest.entries.len() > MAX_ENTRIES || manifest.attempts.len() > MAX_ATTEMPTS {
                    return Err(Error::ResourceLimit);
                }
                let mut data = Data {
                    entries: BTreeMap::new(),
                    attempts: manifest.attempts,
                };
                if data
                    .attempts
                    .iter()
                    .any(|(key, time)| !digest_valid(key) || *time > 253402300799)
                {
                    return Err(Error::InvalidInput);
                }
                for (name, meta) in manifest.entries {
                    crate::configuration::provider_file_name(&name)?;
                    if !digest_valid(&meta.url_hash)
                        || !digest_valid(&meta.digest)
                        || meta.fetched_at > 253402300799
                        || meta
                            .last_slot
                            .is_some_and(|d| !(19700101..=99991231).contains(&d))
                    {
                        return Err(Error::InvalidInput);
                    }
                    let _read_budget = MEMORY.reserve(BODY_LIMIT)?;
                    let value = read(&path.join(format!("{name}.yaml")), BODY_LIMIT)?;
                    if hash(value.as_bytes()) != meta.digest {
                        return Err(Error::InvalidInput);
                    }
                    data.entries.insert(name, (meta, Text::new(value)?));
                }
                data
            }
            Err(Error::Network) if !path.join(STATE_FILE).try_exists()? => Data::default(),
            Err(e) => return Err(e),
        };
        let journal = parent.join(".provider-attempts.json");
        if journal.try_exists()? {
            let attempts = attempt_records(crate::routing::runtime::yaml::decode(&read(
                &journal,
                STATE_LIMIT,
            )?)?)?;
            if attempts.len() > MAX_ATTEMPTS
                || attempts
                    .iter()
                    .any(|(k, v)| !digest_valid(k) || *v > 253402300799)
            {
                return Err(Error::InvalidInput);
            }
            for (key, time) in attempts {
                let value = data.attempts.entry(key).or_insert(time);
                *value = (*value).max(time);
            }
            while data.attempts.len() > MAX_ATTEMPTS {
                let oldest = data
                    .attempts
                    .iter()
                    .min_by_key(|(_, time)| *time)
                    .map(|(key, _)| key.clone())
                    .ok_or(Error::Internal)?;
                data.attempts.remove(&oldest);
            }
        }
        Ok((
            Self {
                parent,
                directory: path,
                lock: Some(lock),
            },
            data,
        ))
    }
    pub fn stage(&self, data: Data) -> Result<Batch> {
        if self.lock.is_none() {
            return Err(Error::InvalidState);
        }
        let mut pending = 0;
        for entry in fs::read_dir(&self.parent)? {
            let entry = entry?; // 目录读取失败必须传播，不能误当作没有残留批次。
            if entry
                .file_name()
                .to_str()
                .is_some_and(|n| n.starts_with(".rules-stage-"))
            {
                pending += 1;
            }
        }
        if pending >= 2 {
            return Err(Error::ResourceLimit);
        }
        if data.entries.len() > MAX_ENTRIES || data.attempts.len() > MAX_ATTEMPTS {
            return Err(Error::ResourceLimit);
        }
        let id = crate::lifecycle::identifier().ok_or(Error::ResourceLimit)?;
        let path = self.parent.join(format!(".rules-stage-{id}"));
        // create_dir 排他成功后才拥有清理责任，不能清理创建失败的他人目录。
        let mut builder = fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder.create(&path)?;
        let batch = Batch {
            path,
            destination: self.directory.clone(),
            data,
            committed: false,
        };
        for (name, (_, text)) in &batch.data.entries {
            crate::configuration::provider_file_name(name)?;
            write(
                &batch.path.join(format!("{name}.yaml")),
                text.text.as_bytes(),
            )?;
        }
        let manifest = Manifest {
            version: 1,
            entries: batch
                .data
                .entries
                .iter()
                .map(|(n, (m, _))| (n.clone(), m.clone()))
                .collect(),
            attempts: batch.data.attempts.clone(),
        };
        let bytes = serde_json::to_vec(&manifest).map_err(|_| Error::Internal)?;
        if bytes.len() > STATE_LIMIT {
            return Err(Error::ResourceLimit);
        }
        write(&batch.path.join(STATE_FILE), &bytes)?;
        File::open(&batch.path)?.sync_all()?;
        Ok(batch)
    }
    pub fn attempts(&self, attempts: &BTreeMap<String, u64>) -> Result<()> {
        if self.lock.is_none() {
            return Err(Error::InvalidState);
        }
        if attempts.len() > MAX_ATTEMPTS {
            return Err(Error::ResourceLimit);
        }
        let records: Vec<_> = attempts
            .iter()
            .map(|(key, time)| Attempt {
                key: key.clone(),
                time: *time,
            })
            .collect();
        attempt_records(records.clone())?;
        let bytes = serde_json::to_vec(&records).map_err(|_| Error::Internal)?;
        if bytes.len() > STATE_LIMIT {
            return Err(Error::ResourceLimit);
        }
        let path = self.parent.join(format!(
            ".attempt-stage-{}",
            crate::lifecycle::identifier().ok_or(Error::ResourceLimit)?
        ));
        if let Err(error) = write(&path, &bytes) {
            let _ = crate::error::diagnostics::observe(
                fs::remove_file(&path).map_err(Error::from),
                crate::error::diagnostics::DiagnosticSite::Cleanup,
            );
            return Err(error);
        }
        let destination = self.parent.join(".provider-attempts.json");
        let result = fs::rename(&path, &destination);
        if result.is_err() {
            let _ = crate::error::diagnostics::observe(
                fs::remove_file(&path).map_err(Error::from),
                crate::error::diagnostics::DiagnosticSite::Cleanup,
            );
        }
        result?;
        File::open(&self.parent)?.sync_all()?;
        Ok(())
    }
    /// 复用已下载正文前重新核验实际文件，既不为本地编辑发起下载，也不盲信内存副本。
    pub fn verify(&self, data: &Data) -> Result<()> {
        if self.lock.is_none() {
            return Err(Error::InvalidState);
        }
        let path = self.directory.join(STATE_FILE);
        if !path.try_exists()? {
            return if data.entries.is_empty() {
                Ok(())
            } else {
                Err(Error::InvalidState)
            };
        }
        let manifest: Manifest = crate::routing::runtime::yaml::decode(&read(&path, STATE_LIMIT)?)?;
        if manifest.version != 1 || manifest.entries.len() != data.entries.len() {
            return Err(Error::InvalidInput);
        }
        for (name, (meta, _)) in &data.entries {
            if manifest.entries.get(name) != Some(meta) {
                return Err(Error::InvalidInput);
            }
            let file_path = self.directory.join(format!("{name}.yaml"));
            let m = fs::symlink_metadata(&file_path)?;
            if !m.is_file() || m.file_type().is_symlink() || m.len() > BODY_LIMIT as u64 {
                return Err(Error::InvalidInput);
            }
            let mut options = OpenOptions::new();
            options.read(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
            }
            let mut file = options.open(file_path)?;
            let mut hash = ring::digest::Context::new(&ring::digest::SHA256);
            let mut buffer = [0; 16384];
            let mut total = 0;
            loop {
                let count = file.read(&mut buffer)?;
                if count == 0 {
                    break;
                }
                total += count;
                if total > BODY_LIMIT {
                    return Err(Error::ResourceLimit);
                }
                hash.update(&buffer[..count]);
            }
            let actual: String = hash
                .finish()
                .as_ref()
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect();
            if actual != meta.digest {
                return Err(Error::InvalidInput);
            }
        }
        Ok(())
    }
    pub fn close(&mut self) -> Result<()> {
        if let Some(file) = self.lock.take() {
            file.unlock()?;
        }
        Ok(())
    }
}
fn write(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}
pub(super) struct Batch {
    path: PathBuf,
    destination: PathBuf,
    pub data: Data,
    committed: bool,
}
impl Batch {
    /// 交换成功后不再返回可回滚错误；目录持久化结果另行报告，不能谎称未提交。
    pub fn commit(&mut self) -> Result<()> {
        if self.committed {
            return Err(Error::InvalidState);
        }
        for path in [&self.path, &self.destination] {
            let m = fs::symlink_metadata(path)?;
            if !m.is_dir() || m.file_type().is_symlink() {
                return Err(Error::InvalidInput);
            }
        }
        crate::platform::providers::exchange(&self.path, &self.destination)?;
        self.committed = true;
        Ok(())
    }
    pub fn sync_parent(&self) -> Result<()> {
        File::open(self.destination.parent().ok_or(Error::Internal)?)?.sync_all()?;
        Ok(())
    }
}
impl Drop for Batch {
    fn drop(&mut self) {
        if let Err(e) = fs::remove_dir_all(&self.path) {
            crate::error::diagnostics::record(
                e.into(),
                crate::error::diagnostics::DiagnosticSite::Cleanup,
            )
        }
    }
}

#[cfg(test)]
#[path = "../../tests/test_providers_cache.rs"]
mod test_providers_cache;
