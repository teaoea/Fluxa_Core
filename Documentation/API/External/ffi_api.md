# FFI 配置检查 API

- API 分类：外部调用 API；仅承诺本文已说明的公开入口。

- 来源源码：`src/ffi.rs`；配置诊断由 `src/configuration/check.rs` 实现。
- 入口：`nodus_core_command_v2`，operation 为 `configuration_check` 或 `configuration_check_file`。
- 契约：ABI 2，开发接口；本页只覆盖配置检查命令，不宣称所有 FFI 操作已经文档化。
- Rust API、源位置与编辑器换算见 [check_api.md](configuration/check_api.md)；内部实现见 [src/ffi.md](../../../src/ffi.md)。


配置检查是只读操作。Core 负责读取、严格解析和校验主配置，返回首个错误的位置与固定整数错误代码；不创建、修复、覆盖或保存文件，不应用配置，不启动监听或 VPN，也不改变当前有效运行配置。

本页描述当前配置检查契约。Rust 入口位于 `src/configuration/check.rs`；JSON 命令通过既有 `nodus_core_command_v2` C ABI 调用，不是 HTTP 接口。App 接入及发布状态另行记录。

## 两种检查方式

| 用途 | JSON operation | Rust 入口 |
| --- | --- | --- |
| 检查编辑器当前正文，包括未保存内容 | `configuration_check` | `configuration::check_yaml(yaml)` |
| 检查宿主明确授权的现有配置文件 | `configuration_check_file` | `configuration::check_file(path)` |

实时编辑检查应传入当前编辑器的原始正文。文件检查针对磁盘当前内容，不能用它的定位结果标记一份尚未保存、内容已经不同的编辑器缓冲区。

## 检查 YAML 正文

### 请求

```json
{
  "operation": "configuration_check",
  "yaml": "rules: ['final,direct']\n"
}
```

| 字段 | 类型 | 必填 | 说明 |
| --- | --- | --- | --- |
| `operation` | string | 是 | 固定为 `configuration_check`。 |
| `yaml` | string | 是 | 原始 UTF-8 YAML 文本，最多 4 MiB；保留原始注释、空行、缩进和换行符。 |

不接受其他字段。调用仍受通用 ABI 请求长度、响应句柄、并发和内存预算约束；4 MiB 是配置正文上限，不保证转义后的完整 JSON 请求一定可准入。

### 成功响应

下例只列出与配置检查有关的字段，省略其他通用响应中的空值字段。

```json
{
  "success": true,
  "configuration": {
    "mode": "rule",
    "system_proxy_port": 0,
    "lan_share_port": 0,
    "node_count": 0,
    "rule_count": 1,
    "rule_provider_count": 0,
    "dns_enabled": false,
    "dns_timeout_seconds": 0,
    "geoip_enabled": false,
    "tun_requested": false
  }
}
```

`success: true` 仅表示主配置通过现有严格解析和语义校验。它不证明节点连通、资源可用、权限有效、目标平台支持活动能力，或 VPN 可以启动。

## 检查配置文件

### 请求

```json
{
  "operation": "configuration_check_file",
  "path": "/absolute/host-authorized/config.yaml"
}
```

| 字段 | 类型 | 必填 | 说明 |
| --- | --- | --- | --- |
| `operation` | string | 是 | 固定为 `configuration_check_file`。 |
| `path` | string | 是 | 宿主明确提供的现有普通文件绝对路径；UTF-8，最多 4096 字节，不含控制字符、相对目录或上级目录跳转。 |

Core 只读打开文件，拒绝最终路径为符号链接、非普通文件、超限正文及读取期间检测到的文件变化。Core 不搜索默认目录，不根据路径推断 App Group，不在文件缺失或损坏时生成、回退或修复配置。

路径不是授权。Apple App 和 Network Extension 必须分别具备真实 sandbox、App Group 或必要的 security-scoped URL 读取权限；Core 不能凭字符串赋予跨进程文件访问权。扩展不得写入配置和资源。

成功响应与正文检查相同。

## 错误响应

例如下面的配置中，系统代理端口 `80` 低于启用端口的最小值 `1054`：

```yaml
# demo

system-proxy:
  port: 80
rules: ['final,direct']
```

与检查有关的错误响应为：

```json
{
  "success": false,
  "level": "error",
  "error_info": {
    "code": 1001,
    "params": {}
  },
  "configuration_issue": {
    "error_info": {
      "code": 1001,
      "params": {}
    },
    "field": "system-proxy.port",
    "position": {
      "line": 4,
      "column": 3,
      "utf16_offset": 24
    }
  }
}
```

### 错误字段

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `success` | boolean | 检查失败时为 `false`，不能因返回诊断而当作配置通过。 |
| `error_info` | object | 通用错误协议，由 `src/error.rs` 唯一定义。 |
| `error_info.code` | integer | 固定四位整数错误代码；不从文本、字段名或位置生成。 |
| `error_info.params` | object | 该代码既有 Schema 允许的参数；无参数代码返回 `{}`。 |
| `configuration_issue` | object | 仅在已进入配置检查且得到检查错误时存在；无效句柄、请求解析或预算准入失败不保证具有此字段。 |
| `configuration_issue.error_info` | object | 与顶层错误一致，供独立消费诊断对象；不是另一套错误代码。 |
| `configuration_issue.field` | string | 有界字段路径，最多 1024 ASCII 字节；数组索引从 0 开始，不包含字段值、凭据或完整文件路径。 |
| `configuration_issue.position` | object 或 null | 能定位时返回位置；没有可靠位置时为 `null`，不伪造第 1 行。 |

字段路径例如 `system-proxy.port`、`rule-providers[0].policy`、`rules[2]`。`$` 表示整个文档或无法细分的文档错误；`$file` 表示文件路径、权限或读取错误。动态 DNS hosts 键以序号表达，例如 `dns.hosts[0]`，不公开实际域名。

语义校验可能定位到完整条目或对象，例如 `proxies[0]`、`rewrites[0]` 或 `dns.fake-ip`；这表示错误属于该条目，不能将对象起始位置冒充内部某个值的精确位置。缺失字段可能没有可指向的原文位置。当前返回首个错误，不承诺一次列出所有问题。

### 文件访问错误

文件不存在、无权访问或读取失败时，不返回 YAML 行号。例如文件访问错误使用现有 `6001` 类别：

```json
{
  "success": false,
  "error_info": {"code": 6001, "params": {}},
  "configuration_issue": {
    "error_info": {"code": 6001, "params": {}},
    "field": "$file",
    "position": null
  }
}
```

具体代码仍取实际触发原因；非法相对路径、非普通文件和大小超限不会统一伪装成文件不存在。

## 行号与前端编辑器一致性

| 位置字段 | 单位及起点 | 用法 |
| --- | --- | --- |
| `line` | 从 1 开始的源文件逻辑行 | 注释和空行都计数；自动视觉换行不改变 YAML 文件行。 |
| `column` | 从 1 开始的 Unicode 标量列 | 不是 UTF-8 字节列，也不是把表情一律当作一个 UTF-16 单元。 |
| `utf16_offset` | 从原文起点开始、从 0 计数的 UTF-16 code unit 偏移 | 交给前端编辑器定位和换算实际显示行、列。 |

现有 macOS 编辑器会给自动换行后的显示行编号。显示行取决于窗口宽度、字体及排版，Core 无法仅凭 YAML 原文确定；前端应优先使用 `utf16_offset`，经现有 `CodeEditorLineLocator.position(at:)` 获取当前显示行、列。未自动换行时，显示行与逻辑行一致。

处理顺序：

1. 将检查时的同一份原始正文交给 Core，保留编辑器文档版本或内容指纹。
2. 收到结果时，确认正文仍是同一版本；过期结果丢弃或重新检查。
3. `position` 非空时，校验 `utf16_offset` 在当前正文 UTF-16 长度范围内，交给编辑器定位、滚动或高亮。
4. 优先展示编辑器换算的显示行；编辑器布局尚不可用时，可明确标为“文件第 N 行”，不能把源文件逻辑行冒充自动换行后的显示行。
5. `position` 为空时展示字段及错误说明，不跳转到伪造行号。

空行、中文注释、表情、LF 和 CRLF 都必须保留。前端不能先裁剪空行、重新序列化、替换换行符或格式化后，再把旧偏移用于新正文。现有编辑器逻辑索引使用 LF 分行，CRLF 中的 CR 不额外增加一行。

## 检查范围与应用边界

检查与实际配置解析共用严格 YAML 解码器和语义校验器，包括字段类型、未知／重复字段、端口约束、规则语法、节点引用及提供者元数据等；不通过解析错误字符串猜测字段位置。

- 默认模板采用 `final,direct`，由 App／宿主决定是否新建和保存。
- 若兜底策略指定不存在的节点，例如 `final,missing-node`，检查返回错误；配置应用和 VPN 候选准备同样拒绝，不能回退直连。
- 本接口不下载或遍历插件、JS、规则正文、GeoIP 文件，也不验证远端 URL 能否访问。宿主准备的资源由应用配置时的只读加载流程校验。
- 字段合法与当前活动能力是否实现分开处理。检查成功不等于可以运行尚未实现的监听、TUN、MITM 或其他模式。
- 沿用既有校验语义，例如启用 DNS 时非法 timeout 被归一为默认值；本 API 不借诊断改变既定处理规则，也不会将归一结果写回原文件。

## 常见错误代码

下表仅列举本接口可能遇到的既有代码，不新增或重定义错误协议。完整参数与含义见 [错误代码查询表](error_api.md)。

| 代码 | 含义 |
| --- | --- |
| `1001` | 字段类型、取值、交叉引用或路径不合法。 |
| `1021` | 配置声明当前不支持的结构或能力。 |
| `1201` | ABI 引擎句柄无效；属于调用错误，不保证有配置位置。 |
| `2001` | YAML 语法错误。 |
| `2002` | 未知配置字段。 |
| `2003` | 重复配置字段。 |
| `2401` | GeoIP URL 配置不合法。 |
| `6001` | 文件或网络 I/O 类别；本文件入口的文件访问失败使用此既有映射。 |
| `8001` | 输入长度、数量或结构超过限制。 |
| `8002` | 内存／资源预算准入失败。 |

其他错误按实际内核返回处理，前端不得把未知代码转换为检查成功或允许启动 VPN。

Rust 调用方式见 [check_api.md](configuration/check_api.md)。

## 验证记录入口

- 字段、重复键、Unicode／CRLF 和只读文件检查：`tests/test_configuration_check.rs` 及同名报告。
- JSON 命令与编辑器偏移：`tests/test_ffi.rs` 及同名报告。
- 不存在的默认节点拒绝候选：`tests/test_vpn.rs` 及同名报告。

测试报告以实际源码指纹及退出状态为准。本页不宣称 App 已接入、最低系统或真机已验收，也不代表二进制已发布。
