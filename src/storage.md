# storage.rs 代码说明

## 职责（干嘛的）

对应源码：`src/storage.rs`。本页说明实现职责、数据流程和边界；公开调用的请求／响应契约单独维护于 `Documentation/`，开发约束维护于 `AGENTS/`。

## 目标（想达成什么）

只读取得宿主明确提供的配置与命名资源，不让内容中的路径或 URL 扩大文件授权。

## 主要流程与模块协作

default_configuration 返回纯模板；read_configuration 以受限文件入口读取；Resources 校验命名规则／插件／脚本／GeoIP 引用，ReadOnlyFile 验证绝对路径、类型、最终链接、大小与读取期间变化。

## 输入、输出与失败边界

输入来自受约束的配置、明确授权的资源或校验后的网络数据；各入口的实际类型、字段和值域以下文及对应源码为准。正常结果只能说明该模块完成其职责，不能推定网络接管、其他模块初始化或外部权限已经成功。非法输入、协议错误、系统故障、取消和资源不足沿用 `src/error.rs` 的真实分类；不静默改为 DIRECT，也不修复或覆盖外部配置。

活动资源由 Core 的会话身份、有效租约、取消及明确关闭操作管理；只读快照引用不构成活动权限。内存与任务的实际存活期计入对应预算。具体模块没有活动任务时，不因本说明增加后台线程或额外权限。

## 测试与示例入口

对应测试及实际结果：[test_storage.md](../tests/test_storage.md)、[test_storage.rs](../tests/test_storage.rs)。

真实示例入口：[rule_providers.rs](../examples/rule_providers.rs)、[shared_storage.rs](../examples/shared_storage.rs)。

性能结果及执行记录保存在对应测试文档；下文涉及旧版本的验证陈述仅是历史资料，不是本次重新执行的结果。已发布版本的历史能力不能替代当前代码、真实平台或系统 VPN 验收。

## 实现细节

当前开发契约：2026-10-21。Core 是只读消费者，App／CLI／平台宿主管理文件和目录。本说明替代旧的 Core 共享目录初始化、默认文件创建、保存、目录事务及自动落盘更新描述；历史实现与测试保留在提交历史中。

## 职责

宿主选择目录，取得真实权限，创建和保存配置，下载并校验来源，保存插件、JS、规则和 GeoIP。更新时宿主先准备完成一批文件，再明确提交绝对路径及资源清单。Core 不读取 HOME、不查询 App Group、不搜索目录、不创建锁、日志、索引、配置或缓存文件；读取失败不修复或回退其他文件。

Apple 共享组仍由宿主按项目约定申请 `group.app.fluxa.ios`。路径本身不能授予权限；App 和 Network Extension 需要各自有效的签名、sandbox 和共享组访问权。Apple 没有只读 App Group entitlement；只读约束来自扩展接口及 Core 的实际文件操作，不声称操作系统已经隔离共享容器写权限。

## 模板和配置

`default_config.yaml` 与 `default_config_en.yaml` 是默认模板，`storage::default_configuration(language)` 只返回内容；中文设备语言选择中文，其他语言回退英语。默认兜底为 `final,direct`。App 决定何时创建、保存位置、名称及覆盖策略，Core 不决定目录中有没有配置。

`storage::read_configuration(path)` 只读加载并严格校验现有绝对路径；`Manager::apply_configuration_file(path, resources)` 候选准备成功后才发布网络配置。配置错误或资源失败保留现有有效运行配置和原始文件；Core 不自行修改宿主先前保存的错误文件。正文检查、字段和编辑器位置见 [配置文件检查 API](../Documentation/API/External/ffi_api.md)。

旧 `storage_initialize`、`configuration_save` 和隐式共享容器加载已移除。能力位 30 不再声明；位 31 表示纯模板和宿主授权文件只读能力。旧客户端必须同步迁移，不能把旧保存请求当作成功。回滚需要恢复整组客户端与二进制，不能混用。

## 资源清单

文件路径只从宿主明确调用参数接收，不从主配置、远程正文或插件 URL 推断磁盘访问权限。清单只接受 `script`、`plugins`、`rules`、`geoip`，未知字段拒绝。

```json
{
  "script": {"name": "response", "path": "/host/files/response.js"},
  "plugins": [
    {
      "name": "ads",
      "id": 1,
      "path": "/host/files/plugin.yaml",
      "script": {"name": "response", "path": "/host/files/plugin-response.js"}
    }
  ],
  "rules": [{"name": "remote", "path": "/host/files/remote.yaml"}],
  "geoip": "/host/files/geoip.mmdb"
}
```

以上路径为说明性占位路径，不能直接运行；只传主配置实际启用并需要的资源。规则／插件资源名称必须与已启用提供者一致，数量须对应；插件 ID 非零且唯一，按 ID 升序确定插件优先级，宿主数组顺序不授予更高优先级。脚本清单 name 必须与该来源 scripts.name 精确一致。关闭 GeoIP 时不接收一个额外数据库冒充启用。

主配置最多 4 MiB，规则单项 8 MiB，插件单项 1 MiB，JS 256 KiB，MMDB 256 MiB；各类实际解析、索引、文件描述符和常驻内存另受独立预算。数量上限不保证任意大小组合都能准入。最终路径必须是现有普通文件，拒绝最终符号链接和读取期间检测到的变化；不以只读模式替代宿主的来源认证和路径授权。

## 运行时和缓存

Core 继续拥有不可变快照、DNS、连接、脚本线程、取消和真实关闭边界，不把这些活动控制权交给前端。JS 初次执行读取绑定的只读描述符；规则级 js-ttl 及手动清理后再次读取同一宿主授权路径。宿主如需强不可变版本应提交版本化文件，不能在读取期间改写正文。

运行规则 `Store::memory()` 只在内存更新；`Store::open(file)` 只读加载宿主提供的现有二进制文件，不打开目录或自动迁移 YAML/SQLite。`Snapshot::to_binary()` 与后台服务 `export_binary()` 只返回有界内存编码，宿主决定是否保存。旧 `directory_synced` 字段为 false，写盘耗时为零，不能当作 fsync 成功。

提供者与 GeoIP 不再调度下载、自动刷新或保存文件。宿主更新资源后重新提交清单；旧 refresh 命令明确返回 Unsupported。通用受控网络能力可返回有界内存响应，但不得偷偷落盘。Debug 本地 IPC 的套接字生命周期不是配置持久化，保留必要认证与清理边界。

## 验证边界

测试宿主创建临时资源，Core 只读使用，覆盖缺失／损坏文件不修复、有效配置不变、脚本精确绑定和真实关闭。临时目录不冒充 App Group，库／回环／FFI 测试不证明签名、Network Extension、真实系统 VPN、最低系统或真机验收。本次不包含 App 接入、安装或打包。

## 禁止行为（不允许做什么）

不得搜索隐式目录、初始化配置、创建锁或缓存文件、下载落盘，不能让配置正文自行扩大文件授权；读取失败不得修复或替换原文件。

违反输入、授权或资源边界时，使用实际模块的既有错误／拒绝路径，不伪造成功或擅自扩大权限；具体失败后果以上文流程和公开契约为准。
