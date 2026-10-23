# plugins.rs 代码说明

## 职责（干嘛的）

对应源码：`src/configuration/plugins.rs`。本页说明实现职责、数据流程和边界；公开调用的请求／响应契约单独维护于 `Documentation/`，开发约束维护于 `AGENTS/`。

## 目标（想达成什么）

加载声明式插件中的 MITM、复写和命名脚本绑定，维护主配置优先级及缺失脚本规则排除语义。

## 主要流程与模块协作

load_local 对照宿主 Resources 只读加载插件，Bundle 合并受控声明，Assets 绑定 scripts.name 引用；rewrites 编译执行动作，script::worker 管理 js-ttl 缓存与取消，插件不自行下载或保存脚本。

## 输入、输出与失败边界

输入来自受约束的配置、明确授权的资源或校验后的网络数据；各入口的实际类型、字段和值域以下文及对应源码为准。正常结果只能说明该模块完成其职责，不能推定网络接管、其他模块初始化或外部权限已经成功。非法输入、协议错误、系统故障、取消和资源不足沿用 `src/error.rs` 的真实分类；不静默改为 DIRECT，也不修复或覆盖外部配置。

活动资源由 Core 的会话身份、有效租约、取消及明确关闭操作管理；只读快照引用不构成活动权限。内存与任务的实际存活期计入对应预算。具体模块没有活动任务时，不因本说明增加后台线程或额外权限。

## 测试与示例入口

对应测试及实际结果：[test_configuration_plugins.md](../../tests/test_configuration_plugins.md)、[test_configuration_plugins.rs](../../tests/test_configuration_plugins.rs)。

性能结果及执行记录保存在对应测试文档；下文涉及旧版本的验证陈述仅是历史资料，不是本次重新执行的结果。已发布版本的历史能力不能替代当前代码、真实平台或系统 VPN 验收。

## 实现细节

作者：可莉唯一的狗。适用于当前 main 开发源码；发布附件与提交以最终交付记录为准。

## 字段与示例

插件正文只接受 `mitm`、`rewrites`、`scripts`，缺省为空；拒绝未知字段、重复键、null 脚本对象、标签和别名。插件不能携带正式路由、节点、Debug 或宿主权限。插件不能自行跳过服务器证书验证或开启正文捕获。主配置中的 `plugin-providers` 继续采用 name/category/url/enabled/update-time；URL 必须 HTTPS，每个来源名称唯一，时间使用设备本地 HH:mm，下载时机与更新调度由宿主负责，Core 不启动下载或落盘更新。

```yaml
mitm:
  enabled: true
  hosts: [api.example.com]

rewrites:
  - url: https://api.example.com/user/**
    phase: response
    when:
      status: 200
      content-type: application/json
    action: script
    js: response
    js-ttl: 3600

scripts:
  name: response
  url: https://example.com/response.js
```

`scripts` 是一个对象，不接受数组；name 必填、1–128 字节安全 ASCII 名称，不接受路径、URL、源码、隐藏名称或目录穿越。`rewrites[].js` 必须与 name 精确一致，区分大小写；名称允许没有 .js 后缀，URL 文件名与引用键无关。url 可省略，此时只能引用已由用户放在约定目录的同名本地文件；给出 url 时必须为无凭据 HTTPS。主配置和插件使用同一格式。`js-ttl` 只允许 action: script，必须为正整数秒，省略 3600；JS 注释中的 TTL 不再作为配置读取。

## 宿主文件与只读绑定

App／宿主负责实际共享容器、目录、数字 ID 分配、排序、下载、文件保存和原子替换；Core 不查询容器或管理目录表、恢复日志及历史文件。宿主通过独立资源清单提供主脚本、各插件正文和插件脚本的绝对路径，格式见 [只读文件接入](../storage.md)。配置或插件中的 scripts.url 仅为来源元数据，不能授予文件访问权限。

插件正文最多 1 MiB、脚本最多 256 KiB；最多 64 个已声明插件，受全局文件及内存预算约束。插件解析另预留 64 倍输入和 64 KiB 固定开销。读取拒绝最终符号链接、非普通文件、超长内容及检测到的并发变更；不创建、重写或修复文件。

编译只绑定逻辑名称、宿主授权的绝对路径、只读文件描述符和不可变规则，不保存 JS 正文。源码在命中执行时按需读取；初次执行使用编译时绑定的描述符，TTL 或显式清理后的新执行重新读取同一授权路径。宿主须明确提交新版本，旧快照不会因主配置 URL 获得其他文件权限。

名称未登记、引用不匹配或脚本文件不存在时记录受控错误并排除相应脚本规则，后续有效规则仍可匹配。宿主清单的脚本名称与该来源声明不一致则拒绝整个提交。插件正文结构错误拒绝本次配置，保留已发布版本；脚本执行异常、超时、取消或输出非法直接报错，不默认 Continue 或 DIRECT。

## 优先级与解密边界

每阶段先主配置的有效规则，再插件 ID 升序，每个来源保持声明顺序；第一条 URL 与 when 都匹配的有效规则生效。未绑定脚本不占据这一位置，裸 proxy 继续忽略。同名脚本不跨主配置与插件或跨插件读取。

主配置 MITM 总开关和 bypass 优先；插件声明不能覆盖主配置禁止解密的目标。匹配复用主配置的域名/通配符边界，不另造更宽泛的范围。Manager::mitm_matches 查询已发布的合并范围；范围命中不代表 CA 已受信任、监听已启动或 VPN 已运行。完整主配置启用 MITM/监听仍明确返回 Unsupported；既有显式调试代理也执行主配置 bypass。不得把本节范围判断或回环测试视为系统 MITM 验收。

## JS 输入、输出与隔离

引擎为已固定版本 rquickjs/QuickJS，源码是同步函数表达式，例如 `(input) => ({body: input.body.replace("ad", "")})`。不运行 Promise 作业队列，不提供网络、文件、模块加载或宿主资源句柄。每次执行创建独立运行时与上下文，不跨请求保留 JS 全局变量。

请求阶段输入为 `{method, url, headers, body}`，headers 是 `{name, value}` 数组，body 是 UTF-8 字符串。响应阶段输入为 `{status, headers, body, request}`，request 是本次原始请求的上述结构。响应阶段必须先获得真实服务器响应并通过 when，不能提前构造响应冒充上游。

输出是只包含变更字段的对象，也可回传未修改输入。请求输出允许 method/url/headers/body；URL 必须保持原来源，禁止跨来源、凭据、片段、控制字符或反斜杠。响应输出允许 status/headers/body；如回传 request，必须保持原值。未知字段、字段类型错误、非法最终状态、受保护头篡改或注入均拒绝。替换正文清除失效的压缩、范围、长度与完整性头；写出层重新分帧。输入不支持自动解压或二进制 JS body，压缩正文明确拒绝。

```javascript
// response 资源的完整同步函数；没有缓存生命周期头注释。
input => {
  const data = JSON.parse(input.body);
  delete data.ad;
  return {body: JSON.stringify(data)};
}
```

源码最多 256 KiB，输入/输出 JSON 各 1 MiB，深度 64、节点 16384；运行时堆 32 MiB、执行最多 250 ms，最多四份全局运行时/工作线程配额，每线程有界排队。源码、输入及排队另有预算；预算不等于进程 RSS 上限。输出经内核重新校验，不能取得路由、存储、取消令牌或线程权限。

每个命名资源使用独立工作线程，同一资源被不同 TTL 规则引用时共用最短 TTL。到期标记停止接收新任务，等待在途执行自然结束后释放；手动清理会中断在途 QuickJS、拒绝排队任务并 join 实际线程。容量不足只淘汰无排队且无在途执行的闲置线程，不能依靠创建同一资源的新线程绕过待移除状态。

Rust 使用 `Manager::clear_script_cache().await`。C ABI 使用 `{"operation":"script_cache_clear"}`；成功回复表示实际清理已完成。关闭管理器也必须中断、join 后才能报告关闭完成。脚本失败不污染下一次独立执行。

## 验证边界

对应源码、测试与同名报告位于 tests/test_configuration_plugins、test_script_worker、test_outbound_configuration 和既有复写管线测试。临时私有目录夹具仅验证只读绑定和内存发布，不冒充真实 App Group。实际本机 TLS 测试验证请求/响应修改；真实 Swift FFI、完整回归、发布来源与附件校验分别记录，编译或局部通过不能代替其他验收。系统 VPN、完整主配置 MITM 启动、真机客户端去广告效果、最低部署系统运行及其他平台仍需要独立验证。

## 禁止行为（不允许做什么）

不得使插件覆盖主配置明确禁止的解密范围，不得把未声明或缺失脚本当作有效复写；插件及脚本资源不能自行扩大宿主授权路径或下载落盘。

违反输入、授权或资源边界时，使用实际模块的既有错误／拒绝路径，不伪造成功或擅自扩大权限；具体失败后果以上文流程和公开契约为准。
