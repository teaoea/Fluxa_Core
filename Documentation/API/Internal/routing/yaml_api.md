# YAML 动态映射定位内部 API

- API分类：内部自用。来源：`src/routing/yaml.rs`，实际模块路径为`routing::runtime::yaml`。
- 本次入口：`mapping_key_position(input: &str, path: &str, key: &str) -> Option<(usize, usize)>`，pub(crate)；调用方configuration::check。契约版本2.15.1-dev。

## 输入输出与前提

input是已通过严格主配置解码的原始正文，不得格式化或重新序列化。path为映射容器路径（当前dns.hosts），key为从业务模型选中的真实键；仅在内部匹配，不能放入公开诊断。返回原文条目序号（从0开始）及键事件的Unicode标量偏移（从0开始），quoted键包括引号起始。未找到、解析失败、路径不符或输入超过32MiB、path达到1024字节、key超过1024字节返回None。上层主配置另有4MiB限额。

示例：原文`dns: {hosts: {z.example: 192.0.2.1, a.example: invalid}}`中a.example即使在BTreeMap中排第0，返回原文条目序号1。公开CheckIssue.field应为dns.hosts[1]，不输出真实键。

## 信任、失败、副作用与生命周期

路径/键均不构成授权，不读取文件、不修改正文、不初始化业务资源、不联网。沿用既有Decoder的深度、节点、标量和重复键限制，失败不提供伪造位置；不是另一套配置校验或错误码协议。目标键只在本次扫描存活，无缓存或跨调用状态。结果不是可应用配置、启动授权或草稿版本令牌。

## 验证与边界

`tests/test_routing_yaml.rs`、`tests/test_configuration_check.rs`和`tests/test_ffi.rs`覆盖同一定位链路。Core位置仅对应本次输入，异步调用方必须自行绑定草稿版本，不能把旧结果套用新正文。界面视觉定位、已交付二进制和最低系统真机不在本次源码回归范围。
