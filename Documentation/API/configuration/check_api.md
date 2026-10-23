# 配置检查 Rust API

- 来源源码：`src/configuration/check.rs`，由 `src/configuration.rs` 重导出。
- 入口：`configuration::check_yaml`、`configuration::check_file`；返回 `Result<Summary, Box<CheckIssue>>`。
- 契约：当前开发中的只读诊断接口；检查成功不应用配置，不接管网络，不证明节点或系统能力可用。
- C ABI 请求／响应及完整字段契约见 [ffi_api.md](../ffi_api.md)。

## 目的与调用方式

`check_yaml(input: &str)` 检查编辑器原始 UTF-8 正文（含未保存修改）；`check_file(path: &str)` 只读检查宿主授权的现有绝对文件。前者诊断对应传入正文，后者对应磁盘实际内容，不能互换到不同的缓冲区版本。

```rust
use nodus_core::configuration::check_yaml;
let result = check_yaml("rules: ['final,direct']\n");
match result {
    Ok(summary) => println!("规则数量：{}", summary.rule_count),
    Err(issue) => {
        println!("错误代码：{}；字段：{}", issue.error_info.code.value(), issue.field);
        if let Some(position) = issue.position {
            println!("文件位置 {}:{}；UTF-16 偏移 {}", position.line, position.column, position.utf16_offset);
        }
    }
}
```

## 输入、结果与位置

正文最多 4 MiB；文件路径和有界只读加载遵循 storage 实现，不创建、写回或修复文件。字段类型、未知／重复字段、端口、规则与节点引用等使用与主配置相同的解析和语义验证，只返回首个已发现问题。动态键使用安全序号，不回显凭据或配置正文。

成功返回 `Summary`。失败返回 `CheckIssue { error_info, field, position }`；`error_info` 沿用 `src/error.rs`，`field` 是有界字段路径，缺失可靠位置时 `position=None`。文件访问失败对应 `$file`，不伪造 YAML 行号。

`SourcePosition.line` 是从 1 开始的逻辑行，`column` 是从 1 开始的 Unicode 标量列，`utf16_offset` 是从 0 开始的 UTF-16 code unit 偏移。注释和空行计入原文；LF／CRLF、中文和表情不得在检查前格式化或裁剪。视觉自动换行由编辑器根据同一版本正文的偏移换算；Core 不推断屏幕显示行。

## 错误和执行边界

通用输入、YAML、未知字段、重复字段、资源及 I/O 错误使用既有整数代码；完整语义见 [error_api.md](../error_api.md)。指定不存在的兜底节点必须失败，不能回退直连。合法字段不证明所有活动能力已实现；本接口不下载、遍历或验证外部资源连通性，不初始化 VPN，也不改变现有有效状态。

## 实现与验证入口

- [check.md](../../../src/configuration/check.md)：定位与诊断实现。
- [test_configuration_check.md](../../../tests/test_configuration_check.md)：字段、只读文件、Unicode 和换行诊断的真实执行记录。
- [test_ffi.md](../../../tests/test_ffi.md)：FFI JSON 与位置传递；结果以相应实际版本和源码指纹为准。
