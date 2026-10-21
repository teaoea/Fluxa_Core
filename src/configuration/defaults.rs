//! 默认模板仅返回内容；宿主决定创建、位置和保存，不执行文件系统操作。
/// 当前模板语言为简体中文和英语；其他语言回退英语。
pub(crate) fn yaml(language: &str) -> &'static str {
    if language.to_ascii_lowercase().starts_with("zh") {
        include_str!("../../default_config.yaml")
    } else {
        include_str!("../../default_config_en.yaml")
    }
}
