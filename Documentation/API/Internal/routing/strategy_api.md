# 有界策略解析内部API

- 分类：Core内部自用，不向外暴露活动资源。
- 来源源码：`src/routing/strategy.rs`。
- 入口：`parse(&str)->Result<Plan>`、`Plan::items/nodes/canonical`、`members(&str)->Result<Vec<&str>>`，可见性pub(crate)。
- 版本：2.13.0-dev。

parse借用输入，最多8项、总1033字节、每项128字节；返回固定数组Plan，不拥有源文本。items保留完整顺序，nodes仅返回实际代理名供引用与必要覆盖检查，canonical只分配有界规范字符串，members为实际拨号返回最多8个借用项。禁止空白、空项、重复、括号嵌套及reject后候选；超限ResourceLimit，非法InvalidInput。输入借用不跨配置寿命，无I/O、权限或状态副作用。

单节点名称仍保留；多候选用括号规范化。调用方先保证节点均存在，再在同一会话准入/期限执行自动连接重试；成员列表不授予活动资源使用权。具体运行与公开配置见[外部契约](../../External/configuration/rules_api.md)；算法说明见[src/routing/strategy.md](../../../../src/routing/strategy.md)。测试见tests/test_routing_strategy.rs及同名报告，边界与性能以实际执行记录为准。
