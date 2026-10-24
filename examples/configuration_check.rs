//! 检查原始正文并打印脱敏字段与源位置；不联网、不应用配置或改写文件。
use nodus_core::configuration::check_yaml;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let input = "# 中文 😀\r\ndns: {enable: true, server: [system], hosts: {z.example: '192.0.2.1', a.example: invalid}}\r\nrules: ['final,direct']";
    let issue = check_yaml(input).err().ok_or("预期示例配置被拒绝")?;
    println!("{}", serde_json::to_string(&issue)?);
    Ok(())
}
