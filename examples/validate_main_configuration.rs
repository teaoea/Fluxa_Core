//! 校验新主配置并输出无凭据摘要；不启动监听、不联网、不接管 VPN。
use nodus_core::configuration::Document;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "example_config.yaml".into());
    // 示例本身也先限长读取，不能在调用有界解析前把任意大文件读入内存。
    use std::io::Read;
    let mut text = String::new();
    std::fs::File::open(path)?
        .take((nodus_core::configuration::MAX_CONFIG_BYTES + 1) as u64)
        .read_to_string(&mut text)?;
    match Document::from_yaml(&text) {
        Ok(document) => println!("{}", serde_json::to_string(&document.summary())?),
        Err(error) => {
            eprintln!("{}", serde_json::to_string(&error.public_info())?);
            return Err(error.into());
        }
    }
    Ok(())
}
