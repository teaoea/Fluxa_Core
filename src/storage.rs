//! Core 是配置及资源的只读消费者；所有目录、下载落盘和持久化由宿主负责。
use crate::{
    budget::{ByteBudget, ByteLease},
    error::{Error, Result},
};
use serde::Deserialize;
use std::{
    fs::{File, OpenOptions},
    io::{Read, Seek, SeekFrom},
    path::{Component, Path, PathBuf},
    sync::{Arc, Mutex},
};
use zeroize::Zeroizing;
const READ_MEMORY: usize = 2 * crate::configuration::MAX_CONFIG_BYTES + 4096;
static MEMORY: ByteBudget = ByteBudget::new(8 * READ_MEMORY);
static FILES: ByteBudget = ByteBudget::new(512);

/// 返回纯模板内容；不探测宿主语言、目录或文件，不决定是否创建默认配置。
pub fn default_configuration(language: &str) -> Result<&'static str> {
    if language.len() > 64 || language.chars().any(char::is_control) {
        return Err(Error::InvalidInput);
    }
    Ok(crate::configuration::defaults::yaml(language))
}
/// 一次受预算约束的配置正文；释放时清零，不泄漏文件路径。
pub struct ConfigurationText {
    yaml: Zeroizing<String>,
    _lease: ByteLease,
}
impl ConfigurationText {
    pub fn as_str(&self) -> &str {
        &self.yaml
    }
}
impl serde::Serialize for ConfigurationText {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}
/// 只读加载宿主授权的绝对文件路径；配置异常原样报错，不创建、修复或保存任何文件。
pub fn read_configuration(path: &str) -> Result<ConfigurationText> {
    let text = read_text(path)?;
    crate::configuration::Document::from_yaml(text.as_str())?;
    Ok(text)
}
pub(crate) fn read_text(path: &str) -> Result<ConfigurationText> {
    let lease = MEMORY.reserve(READ_MEMORY)?;
    let file = ReadOnlyFile::open(Path::new(path))?;
    let yaml = file.read(crate::configuration::MAX_CONFIG_BYTES)?;
    Ok(ConfigurationText {
        yaml,
        _lease: lease,
    })
}

/// 文件名是配置内的逻辑资源名，path 是宿主通过独立 API 明确授权的文件；配置正文不授予路径权限。
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NamedFile {
    pub name: String,
    pub path: PathBuf,
}
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginFiles {
    pub name: String,
    pub id: u64,
    pub path: PathBuf,
    #[serde(default)]
    pub script: Option<NamedFile>,
}
/// 宿主准备完毕的一批只读资源；Core 不搜索缺失资源、解析下载目录或自动更新磁盘。
#[derive(Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Resources {
    pub script: Option<NamedFile>,
    pub plugins: Vec<PluginFiles>,
    pub rules: Vec<NamedFile>,
    pub geoip: Option<PathBuf>,
}
impl Resources {
    pub(crate) fn validate(&self) -> Result<()> {
        if self.plugins.len() > 128 || self.rules.len() > 128 {
            return Err(Error::ResourceLimit);
        }
        let mut ids = std::collections::HashSet::new();
        let mut names = std::collections::HashSet::new();
        for plugin in &self.plugins {
            if plugin.id == 0 || !ids.insert(plugin.id) || !names.insert(plugin.name.as_str()) {
                return Err(Error::InvalidInput);
            }
            path_check(&plugin.path)?;
            if let Some(script) = &plugin.script {
                path_check(&script.path)?;
            }
        }
        names.clear();
        for file in &self.rules {
            if !names.insert(&file.name) {
                return Err(Error::InvalidInput);
            }
            path_check(&file.path)?;
        }
        if let Some(file) = &self.script {
            path_check(&file.path)?;
        }
        if let Some(path) = &self.geoip {
            path_check(path)?;
        }
        Ok(())
    }
}
fn path_check(path: &Path) -> Result<()> {
    let text = path.to_str().ok_or(Error::InvalidInput)?;
    if !path.is_absolute()
        || text.len() > 4096
        || text.chars().any(char::is_control)
        || path
            .components()
            .any(|part| matches!(part, Component::ParentDir | Component::CurDir))
    {
        return Err(Error::InvalidInput);
    }
    Ok(())
}
/// 打开现有普通文件且仅有读取权限；持有描述符，不持有或管理目录。
pub(crate) struct ReadOnlyFile {
    file: Mutex<File>,
    _lease: ByteLease,
}
impl ReadOnlyFile {
    pub(crate) fn open(path: &Path) -> Result<Arc<Self>> {
        path_check(path)?;
        let lease = FILES.reserve(1)?;
        let metadata = std::fs::symlink_metadata(path)?;
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return Err(Error::InvalidInput);
        }
        let mut options = OpenOptions::new();
        options.read(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC);
        }
        let file = options.open(path)?;
        let opened = file.metadata()?;
        if !opened.is_file() {
            return Err(Error::InvalidInput);
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if metadata.dev() != opened.dev() || metadata.ino() != opened.ino() {
                return Err(Error::InvalidInput);
            }
        }
        Ok(Arc::new(Self {
            file: Mutex::new(file),
            _lease: lease,
        }))
    }
    pub(crate) fn size(&self) -> Result<usize> {
        usize::try_from(
            self.file
                .lock()
                .map_err(|_| Error::Internal)?
                .metadata()?
                .len(),
        )
        .map_err(|_| Error::ResourceLimit)
    }
    pub(crate) fn check_size(&self, maximum: usize) -> Result<()> {
        if self
            .file
            .lock()
            .map_err(|_| Error::Internal)?
            .metadata()?
            .len()
            > maximum as u64
        {
            return Err(Error::ResourceLimit);
        }
        Ok(())
    }
    /// 调用者先按实际长度持有内存预算；仅读取本次固定长度，增长、截短或原地修改均拒绝。
    pub(crate) fn read_binary(&self, maximum: usize) -> Result<Vec<u8>> {
        let mut file = self.file.lock().map_err(|_| Error::Internal)?;
        let before = file.metadata()?;
        let length = usize::try_from(before.len()).map_err(|_| Error::ResourceLimit)?;
        if length > maximum {
            return Err(Error::ResourceLimit);
        }
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(length)
            .map_err(|_| Error::ResourceLimit)?;
        bytes.resize(length, 0);
        file.seek(SeekFrom::Start(0))?;
        file.read_exact(&mut bytes)?;
        let mut tail = [0];
        if file.read(&mut tail)? != 0 {
            return Err(Error::InvalidInput);
        }
        let after = file.metadata()?;
        if before.len() != after.len() || before.modified()? != after.modified()? {
            return Err(Error::InvalidInput);
        }
        Ok(bytes)
    }
    pub(crate) fn read(&self, maximum: usize) -> Result<Zeroizing<String>> {
        let mut file = self.file.lock().map_err(|_| Error::Internal)?;
        let before = file.metadata()?;
        if before.len() > maximum as u64 {
            return Err(Error::ResourceLimit);
        }
        file.seek(SeekFrom::Start(0))?;
        let mut result = Zeroizing::new(String::new());
        Read::by_ref(&mut *file)
            .take(maximum as u64 + 1)
            .read_to_string(&mut result)?;
        let after = file.metadata()?;
        if result.len() > maximum {
            return Err(Error::ResourceLimit);
        }
        if before.len() != after.len()
            || before.modified()? != after.modified()?
            || result.len() as u64 != after.len()
        {
            return Err(Error::InvalidInput);
        }
        Ok(result)
    }
}
#[cfg(test)]
#[path = "../tests/test_storage.rs"]
mod test_storage;
