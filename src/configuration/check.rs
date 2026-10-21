//! 主配置只读诊断；与实际应用共用严格解析及语义校验，不修复或重新序列化原文。
use super::*;
use crate::error::{Error, ErrorInfo};
/// 源文件逻辑行/Unicode 标量列从 1 开始；UTF-16 偏移从 0 开始，交给编辑器换算自动换行后的显示行。
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct SourcePosition {
    pub line: usize,
    pub column: usize,
    pub utf16_offset: usize,
}
/// 独立诊断上下文；固定错误代码仍唯一来自 error.rs，不把字段或位置拼入代码。
#[derive(Debug, Serialize)]
pub struct CheckIssue {
    pub error_info: ErrorInfo,
    pub field: String,
    pub position: Option<SourcePosition>,
}
impl CheckIssue {
    fn new(error: Error, field: String, position: Option<SourcePosition>) -> Box<Self> {
        Box::new(Self {
            error_info: error.public_info(),
            field,
            position,
        })
    }
}
fn position(input: &str, scalar_offset: usize) -> SourcePosition {
    let mut result = SourcePosition {
        line: 1,
        column: 1,
        utf16_offset: 0,
    };
    // 前端 EditorLineIndex 以 LF 划分逻辑行；CRLF 的 CR 不另加一行。
    for ch in input.chars().take(scalar_offset) {
        result.utf16_offset += ch.len_utf16();
        if ch == '\n' {
            result.line += 1;
            result.column = 1;
        } else {
            result.column += 1;
        }
    }
    result
}
/// 返回首个实际错误；成功仅证明主配置字段合法，不证明外部文件、节点连通或 VPN 就绪。
pub fn check_yaml(input: &str) -> std::result::Result<Summary, Box<CheckIssue>> {
    let root = |error| CheckIssue::new(error, "$".into(), None);
    if input.len() > MAX_CONFIG_BYTES {
        return Err(root(Error::ResourceLimit));
    }
    let bytes = input
        .len()
        .checked_mul(PARSE_FACTOR)
        .and_then(|v| v.checked_add(PARSE_BASE))
        .ok_or_else(|| root(Error::ResourceLimit))?;
    let _lease = MEMORY.reserve(bytes).map_err(root)?;
    let mut raw: Raw = crate::routing::runtime::yaml::decode_diagnosed(input).map_err(|issue| {
        CheckIssue::new(
            issue.error,
            if issue.path.is_empty() {
                "$".into()
            } else {
                issue.path
            },
            Some(position(input, issue.offset)),
        )
    })?;
    let mut field = String::new();
    if let Err(error) = validate_at(&mut raw, &mut field) {
        let offset = crate::routing::runtime::yaml::field_offset(input, &field);
        return Err(CheckIssue::new(
            error,
            if field.is_empty() { "$".into() } else { field },
            offset.map(|offset| position(input, offset)),
        ));
    }
    Ok(Document {
        raw,
        _memory: _lease,
    }
    .summary())
}
/// 仅读取明确授权的现有绝对路径；I/O 错误没有 YAML 位置，禁止伪造第 1 行。
pub fn check_file(path: &str) -> std::result::Result<Summary, Box<CheckIssue>> {
    let text = crate::storage::read_text(path)
        .map_err(|error| CheckIssue::new(error, "$file".into(), None))?;
    check_yaml(text.as_str())
}
#[cfg(test)]
#[path = "../../tests/test_configuration_check.rs"]
mod test_configuration_check;
