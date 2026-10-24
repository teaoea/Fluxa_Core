# providers.rs 代码说明

## 职责（干嘛的）

对应源码：`src/providers.rs`。本页说明实现职责、数据流程和边界；公开调用的请求／响应契约单独维护于 `Documentation/`，开发约束维护于 `AGENTS/`。

## 目标（想达成什么）

消费宿主准备的规则／GeoIP 资源，校验来源、构造候选并以失败保全方式更新内存状态。

## 主要流程与模块协作

Runtime 按主配置提供者声明核对 Resources 中的只读文件，document 解析正文，合并编译有效规则并准备运行状态；下载、设备日历调度和原子文件替换不在 Core 内完成。

## 输入、输出与失败边界

输入来自受约束的配置、明确授权的资源或校验后的网络数据；各入口的实际类型、字段和值域以下文及对应源码为准。正常结果只能说明该模块完成其职责，不能推定网络接管、其他模块初始化或外部权限已经成功。非法输入、协议错误、系统故障、取消和资源不足沿用 `src/error.rs` 的真实分类；不静默改为 DIRECT，也不修复或覆盖外部配置。

活动资源由 Core 的会话身份、有效租约、取消及明确关闭操作管理；只读快照引用不构成活动权限。内存与任务的实际存活期计入对应预算。具体模块没有活动任务时，不因本说明增加后台线程或额外权限。

## 测试与示例入口

对应测试及实际结果：[test_providers.rs](../tests/test_providers.rs)、[test_providers.md](../tests/test_providers.md)。

性能结果及执行记录保存在对应测试文档；下文涉及旧版本的验证陈述仅是历史资料，不是本次重新执行的结果。已发布版本的历史能力不能替代当前代码、真实平台或系统 VPN 验收。

## 实现细节

作者：可莉唯一的狗。当前契约：2026-10-21 main 开发源码；入口 `src/providers.rs`、`src/providers/document.rs`。本文件描述主配置 `rule-providers`，不将独立旧 `subscription::Check` 的 UTC 纯计划接口当作新配置的调度实现。

## 配置与正文

完整配置见 [example_config.yaml](../example_config.yaml)，远程正文见 [rules.yaml](../rules.yaml)。提供者名称唯一，使用安全文件名；策略为 `direct`、`reject` 或已声明的大小写敏感节点名，不能用 `policy: proxy` 加 `outbound`。声明式插件见插件专项；代理提供者仍未支持。

```yaml
rule-providers:
  - name: ads
    url: https://example.com/ads.yaml
    policy: reject
    enable: true
    update-time: "12:00"
rules: ['final,direct']
```

远程正文为 `rules` 列表和小写类型，只声明条件。`domain`、`domain-suffix`、`domain-keyword`、`ip-cidr`、`protocol`、`port` 等按主规则条件校验；不接受远程 final、来源身份或 Debug 声明。`protocol,quic,example.com` 和 `port,443,example.com` 的域名采用 suffix 语义。非拦截的全域 protocol 不合法。正文自行附加策略的行单独忽略；为避免三列歧义，组合条件的域名如果等于 direct、reject 或已声明节点名，也单独忽略。

所有远程 process-name 条目均忽略，不区分提供者策略。提供者 policy=reject 时另忽略 domain-suffix、domain-keyword 条目；只允许既定精确 domain、ip-cidr、protocol、port 条件执行拦截。非字符串、非法及不支持的行单独忽略，不影响其他合法行。YAML 语法、根结构、重复/未知字段及资源上限错误拒绝整次更新。正文 `payload` 不兼容：旧 `subscription::parse_rules` 的 Clash classical API 独立保留，不能将其格式传给新提供者入口。

## 只读加载与事务

2026-10-21 起，提供者的下载、更新计划、目录、文件保存及恢复由 App／宿主负责。Core 不查询共享容器或 HOME，不启动磁盘更新任务，不写 .state.json、尝试记录或缓存；旧目录事务实现已经移除。声明中的 URL/update-time 是宿主使用的来源／计划元数据，不能授权 Core 自行下载或写入。

宿主将每个启用提供者的现有普通文件路径作为 `resources.rules` 提交，名称和已启用声明精确对应。单项正文最多 8 MiB，Core 在实际长度和全局预算准入后只读加载；严格解析、策略过滤、编译和完整准备成功才一次发布新快照。任意源失败不部分发布；旧连接与旧有效配置保留，宿主文件不被修复或改回旧值。

正文只读校验与原有条件过滤继续有效；元数据策略变化后重新编译。新请求读取新版本，已建立连接不因更新主动断开。更新须由宿主明确重新提交清单；`refresh_providers`、`refresh_geoip` 返回既有 Unsupported 1021，不伪造刷新成功。

`provider_status` 返回 active、revision、各来源 rules/ignored、error_info 和 read_only=true。旧下载调度时间 last_success/last_slot 为 null，不伪造下载证据。关闭由 Core 撤销 Authority、等待解析／编译和活动操作结束，并实际归还控制器租约；旧 Arc 不阻止释放，实际快照内存仍计预算。

接口职责和资源清单见 [只读文件接入](storage.md)。完整回归、只读文件、失败保留及旧引用关闭断言见 tests/test_providers、test_providers_document、test_provider_contract 和同名报告。旧下载／目录恢复专项为历史行为证据，不再作为当前只读架构的通过结论。

## 禁止行为（不允许做什么）

不得把资源 URL 当作磁盘授权、下载落盘、创建目录、修复或覆盖宿主文件；资源错误不得发布部分状态或暗中回退旧文件。

违反输入、授权或资源边界时，使用实际模块的既有错误／拒绝路径，不伪造成功或擅自扩大权限；具体失败后果以上文流程和公开契约为准。

规则集policy支持有序候选；enable省略默认true。配置权威策略完整继承到来源规则，远程正文不能自带候选或更改顺序。详细契约见[routing/strategy.md](routing/strategy.md)。

## DNS日志观察（2.16.1-dev）

配置准备的DNS观察Journal与候选管理器日志状态分离；Runtime只传Scope，不发布候选加密/等级。 接口及边界见`Documentation/API/Internal/providers_api.md`和`Documentation/API/External/logging_api.md`。
