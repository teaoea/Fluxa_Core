//! 共享存储的受控业务入口；只操作系统授予当前宿主的容器，不接受外部绝对路径。
use crate::{
    budget::{ByteBudget, ByteLease},
    error::{Error, Result},
};
use zeroize::Zeroizing;
const READ_MEMORY: usize = 2 * crate::configuration::MAX_CONFIG_BYTES + 4096;
static MEMORY: ByteBudget = ByteBudget::new(8 * READ_MEMORY);
// 同进程写意图先串行准入，跨进程再由同一持久锁文件排他；读取仍是原子无锁快照。
static WRITER: std::sync::Mutex<()> = std::sync::Mutex::new(());
/// 读取正文持有独立预算；释放时清零，不包含实际存放路径。
pub struct ConfigurationText {
    yaml: Zeroizing<String>,
    _lease: ByteLease,
}
impl serde::Serialize for ConfigurationText {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}
impl ConfigurationText {
    pub fn as_str(&self) -> &str {
        &self.yaml
    }
}
/// 幂等初始化；仅无任何 YAML 配置时由 Core 创建完整 config.yaml。
/// 返回 true 表示本次创建；false 表示已存在配置，绝不覆盖、启用 VPN 或发起下载。
pub fn initialize() -> Result<bool> {
    initialize_in(crate::platform::storage::context()?)
}
fn initialize_in(context: crate::platform::storage::Context) -> Result<bool> {
    if context.read_only_configuration {
        return Err(Error::InvalidState);
    }
    #[cfg(unix)]
    {
        let _writer = WRITER.lock().map_err(|_| Error::Internal)?;
        let config = crate::platform::storage::Directory::open(&context.root, "config", true)?;
        let _lock = config.lock()?;
        for name in ["log", "rule", "plugin", "js"] {
            crate::platform::storage::Directory::open(&context.root, name, true)?;
        }
        if config.has_yaml()? {
            return Ok(false);
        }
        let yaml = crate::configuration::defaults::yaml(&context.language);
        crate::configuration::Document::from_yaml(yaml)?;
        config.write("config.yaml", yaml.as_bytes(), false)
    }
    #[cfg(not(unix))]
    {
        Err(Error::Unsupported)
    }
}
/// 配置读取不创建目录或锁文件，Network Extension 可以使用；拒绝路径遍历与链接。
pub fn read_configuration(filename: &str) -> Result<ConfigurationText> {
    filename_check(filename)?;
    read_in(crate::platform::storage::context()?, filename)
}
fn read_in(
    context: crate::platform::storage::Context,
    filename: &str,
) -> Result<ConfigurationText> {
    filename_check(filename)?;
    #[cfg(unix)]
    {
        let lease = MEMORY.reserve(READ_MEMORY)?;
        let directory = crate::platform::storage::Directory::open(&context.root, "config", false)?;
        let yaml = directory.read(filename)?;
        crate::configuration::Document::from_yaml(&yaml)?;
        Ok(ConfigurationText {
            yaml,
            _lease: lease,
        })
    }
    #[cfg(not(unix))]
    {
        Err(Error::Unsupported)
    }
}
/// 显式保存合法主配置；当前宿主为扩展时拒绝，不能通过参数自报为 App。
/// 保存不代表提交运行配置，更不代表网络接管。
pub fn save_configuration(filename: &str, yaml: &str) -> Result<()> {
    filename_check(filename)?;
    save_in(crate::platform::storage::context()?, filename, yaml)
}
fn save_in(context: crate::platform::storage::Context, filename: &str, yaml: &str) -> Result<()> {
    if context.read_only_configuration {
        return Err(Error::InvalidState);
    }
    filename_check(filename)?;
    crate::configuration::Document::from_yaml(yaml)?;
    #[cfg(unix)]
    {
        let _writer = WRITER.lock().map_err(|_| Error::Internal)?;
        let directory = crate::platform::storage::Directory::open(&context.root, "config", false)?;
        let _lock = directory.lock()?;
        // 拒绝已有链接/非常规文件；缺失时允许明确的新建。
        directory.validate_target(filename)?;
        directory.write(filename, yaml.as_bytes(), true)?;
        Ok(())
    }
    #[cfg(not(unix))]
    {
        Err(Error::Unsupported)
    }
}
fn filename_check(filename: &str) -> Result<()> {
    if filename.len() > 128
        || !filename.ends_with(".yaml")
        || filename.starts_with('.')
        || filename.contains('/')
        || filename.contains('\\')
        || filename.chars().any(char::is_control)
    {
        return Err(Error::InvalidInput);
    }
    Ok(())
}
/// 编译缓存独立于会整目录交换的原始 rule 资产，避免提供者刷新删除已打开的二进制。
pub(crate) fn runtime_rule_directory() -> Result<std::path::PathBuf> {
    let context = crate::platform::storage::context()?;
    #[cfg(unix)]
    {
        crate::platform::storage::Directory::open(&context.root, ".runtime-rules", true)?;
        Ok(context.root.join(".runtime-rules"))
    }
    #[cfg(not(unix))]
    {
        Err(Error::Unsupported)
    }
}
/// 供实际提供者/GeoIP缓存取得同一系统根目录，不向 FFI 回复暴露路径。
pub(crate) fn asset_root() -> Result<std::path::PathBuf> {
    Ok(crate::platform::storage::context()?.root)
}
#[cfg(test)]
#[path = "../tests/test_storage.rs"]
mod test_storage;
