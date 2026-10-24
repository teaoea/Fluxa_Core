# Nodus Core

[English](README_ENGLISH.md)

Nodus Core 是一个使用 Rust 编写的网络内核，可嵌入网络代理、VPN 和网络调试工具，为使用方提供统一的配置校验、流量规则、域名解析、连接处理和流量改写能力。

Nodus Core 以库的形式使用，通过 Rust API 或 C ABI 接入程序。配置文件及插件、脚本、规则和 GeoIP 等资源由使用方提供，内核负责只读解析、校验和执行。

## 主要能力

- **配置校验**：检查 YAML 语法、字段、重复配置和引用关系，返回错误代码、字段路径及源文件位置。
- **规则与路由**：按本地、插件和远程规则决定 DIRECT、PROXY 或 REJECT，支持域名、IP、端口及协议等条件。
- **DNS 与 GeoIP**：提供 DNS 解析、Fake-IP 映射和离线 IP 地理信息查询。
- **连接与协议**：管理连接、取消与资源释放，支持 HTTP／HTTPS 请求以及 VLESS、REALITY、Vision 和 XUDP 的已实现组合。
- **流量改写**：提供 HTTPS MITM、条件复写、声明式插件和受限 JavaScript 执行。
- **数据包处理**：提供有界的 IPv4／IPv6、TCP／UDP 数据包会话能力，供需要数据面处理的程序调用。

系统 VPN 的权限、接口和路由接管需要使用方接入相应平台能力；仅链接本库不会自动建立系统 VPN。

## 使用方式

### 在 Rust 项目中使用

在使用方的 `Cargo.toml` 中加入 Git 依赖：

```toml
[dependencies]
nodus_core = { package = "Nodus-Core", git = "https://github.com/teaoea/Nodus-Core", branch = "main" }
```

下面的示例检查一份默认直连配置，并在失败时取得具体字段和位置：

```rust
use nodus_core::configuration::check_yaml;

fn main() {
    let yaml = "rules:\n  - final,direct\n";
    match check_yaml(yaml) {
        Ok(summary) => println!("配置有效，规则数量：{}", summary.rule_count),
        Err(issue) => {
            eprintln!("错误代码：{}，字段：{}", issue.error_info.code.value(), issue.field);
            if let Some(position) = &issue.position {
                eprintln!("位置：第 {} 行，第 {} 列", position.line, position.column);
            }
        }
    }
}
```

检查成功表示配置通过校验，不会应用配置或启动网络。可从 [中文默认模板](default_config.yaml) 或 [英文默认模板](default_config_en.yaml) 开始编写配置。

### 通过 C ABI 接入

将匹配目标平台的内核静态库与 [公开头文件](include/nodus_core.h) 链接到使用方程序，通过头文件声明的接口创建句柄、调用能力、复制结果并释放资源。配置检查与 VPN 前置检查事务命令的请求和响应示例见 [C ABI 配置检查与 VPN 前置检查事务 API](Documentation/API/External/ffi_api.md)；前置检查只报告真实网络检查结果，不接管系统 VPN。

### 运行配置检查示例

```sh
git clone https://github.com/teaoea/Nodus-Core.git
cd Nodus-Core
cargo run --locked --example validate_main_configuration -- default_config.yaml
```

该示例读取指定 YAML 文件并输出校验摘要；配置错误时返回错误信息，不启动监听或 VPN。

## 文档

- [API 文档](Documentation/API/README.md)
- [配置检查 Rust API](Documentation/API/External/configuration/check_api.md)
- [错误代码与参数](Documentation/API/External/error_api.md)

## 许可证

作者：可莉唯一的狗。自有代码采用 [MIT 许可证](LICENSE)；第三方依赖及许可说明见 [LICENSE.md](LICENSE.md)。

平台权限与能力结果新增受约束的[只读诊断契约](Documentation/API/External/vpn/platform_check_api.md)。当前缺少真实平台证据时仍拒绝接管，网络检查成功不代表权限或防回环已经通过。

主配置支持有序混合策略与自动连接重试，见[配置契约](Documentation/API/External/configuration/rules_api.md)。仅节点建连失败触发下一候选，不重放业务请求。
