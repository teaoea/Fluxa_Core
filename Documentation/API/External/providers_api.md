# 规则集正文校验API

- 分类：外部Rust/配置契约。
- 来源源码：`src/providers.rs`。
- 入口：`providers::validate_rules(yaml: &str, policy: Policy, outbound: Option<String>) -> Result<Validation>`。
- 版本：2.13.0-dev增量；复用已有入口，不新增C符号。

Proxy策略的outbound支持单节点或有序候选表达式。策略最多8项，完整表达式/名称长度由统一解析器验证；Direct/Reject不允许outbound。正文仅声明条件，不允许注入候选节点名或改变调用方策略；所有候选节点名参与远程行的注入检查。返回accepted/ignored计数，不创建来源、不发布快照、不赋予节点/Debug权限；无文件或网络副作用。

示例：`validate_rules("rules: ['domain,example.com']", Policy::Proxy, Some("(节点1,节点2)".into()))`只预验证正文；实际主配置仍须验证两个节点存在。语法/策略错误使用既有InvalidInput，超限ResourceLimit，结构正确的非法条件行按原约定忽略。输入及结果均为有界数据，没有活动资源所有权。自动连接重试边界见[策略契约](configuration/rules_api.md)。

测试为tests/test_providers.rs及同名报告：明确验证远程正文不能将列表中任何一个节点伪装成条件目标；不证明外部服务端或节点可用性。
