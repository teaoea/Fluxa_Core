//! 宿主选择和保存文件；Core 仅提供默认模板并读取明确授权的现有绝对路径。
fn main() -> Result<(), Box<dyn std::error::Error>> {
    if let Some(path) = std::env::args().nth(1) {
        let content = nodus_core::storage::read_configuration(&path)?;
        let summary = nodus_core::configuration::Document::from_yaml(content.as_str())?.summary();
        println!("{}", serde_json::to_string(&summary)?);
    } else {
        print!("{}", nodus_core::storage::default_configuration("zh-Hans")?);
    }
    Ok(())
}
