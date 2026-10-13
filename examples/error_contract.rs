//! 生成并核验内核公开错误契约；只输出固定代码元数据，不读取用户配置。
use libfluxa_core::error::ErrorCode;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let contracts: Vec<_> = ErrorCode::ALL.iter().map(|code| code.contract()).collect();
    let json = serde_json::to_string_pretty(&contracts)? + "\n";
    let mut document = include_str!("../Documentation/ErrorContractIntroduction.md").to_owned();
    document.push_str("\n| 错误代码 | 中文名称 | 所属模块 | 分类 | 具体错误说明 | 触发条件 | 处理后果 | 错误参数 | 源码位置 | 状态 |\n| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |\n");
    for row in &contracts {
        document.push_str(&format!(
            "| `{}` | {} | {} | {} | {} | {} | {} | {} | {} | {} |\n",
            row.code,
            row.name,
            row.modules,
            row.category,
            row.detail,
            row.trigger,
            row.effect,
            row.params,
            row.source,
            row.status
        ));
    }
    match std::env::args().nth(1).as_deref() {
        Some("--write") => {
            std::fs::write("Documentation/error-codes.json", json)?;
            std::fs::write("Documentation/错误代码查询表.md", document)?;
        }
        Some("--check") => {
            if std::fs::read_to_string("Documentation/error-codes.json")? != json
                || std::fs::read_to_string("Documentation/错误代码查询表.md")? != document
            {
                return Err("错误契约、Schema 或查询表不一致；请审查后重新生成".into());
            }
            println!(
                "{} 项公开代码全部登记，重复/遗漏/未定义/空字段：0",
                contracts.len()
            );
        }
        _ => return Err("使用 --write 或 --check；请在内核根目录运行".into()),
    }
    Ok(())
}
