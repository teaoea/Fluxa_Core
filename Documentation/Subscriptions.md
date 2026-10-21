# 规则提供者生命周期

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

接口职责和资源清单见 [只读文件接入](共享存储.md)。完整回归、只读文件、失败保留及旧引用关闭断言见 tests/test_providers、test_providers_document、test_provider_contract 和同名报告。旧下载／目录恢复专项为历史行为证据，不再作为当前只读架构的通过结论。
