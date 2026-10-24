# 配置混合策略与自动连接重试

- 分类：外部配置／Rust API契约。
- 来源源码：`src/configuration/rules.rs`、`src/configuration/models.rs`、`src/outbound/execution.rs`。
- 入口：`configuration::check_yaml/check_file`、`Document::from_yaml`、`Manager::connect_configured`与原C ABI配置检查／应用／出站命令，无新增C符号。
- 版本：2.13.0-dev，兼容新增候选表达式；未正式发布。

## 自动连接重试

策略列表最多8项、每个节点名最多128 UTF-8字节；禁止空项、重复、嵌套和reject后的不可达候选。主配置规则使用括号保护逗号，例如`final,(节点1,节点2,reject)`；规则集`policy`允许`节点1,节点2`。所有节点引用先校验，禁用来源也不豁免语义。规则集`enable`省略为true。列表整体在原规则优先级位置参与匹配，不拆成可改变优先级的多条规则；reject候选不是提前生效的全局拦截规则。

运行时只在请求到节点的建连阶段发生网络失败、拒绝或超时时按列表重试；TLS身份/配置错误、目标接收方的DNS/连接/响应错误、取消、预算或状态失效不切换。每次复用并克隆本次已校验的目标/端口/传输及路由上下文；不缓存或重放HTTP正文、TCP业务数据或UDP报文。建立代理流后发生目标连接失败或读写故障无法可靠归因，不再自动连接重试。UDP指代理到节点的承载连接建立阶段，不把UDP静默无响应判成节点连接失败。

仅显式direct候选允许直连，direct到接收方失败不再尝试其他节点；reject终止。所有候选共用总期限，按剩余候选分配剩余建连时间；失败的socket/future/预算由RAII释放，旧会话身份及取消仍由同一准入边界约束。返回决策记录实际选中节点或direct，不伪造全列表已验证。预检展开必要代理集合，各成员未被实际覆盖仍拒绝通过。

## 示例与错误

```yaml
# 节点名称必须在proxies中实际定义；完整可解析样例见example_config.yaml。
rule-providers:
  - name: remote
    url: https://example.com/rules.yaml
    policy: 节点1,节点2
rules:
  - final,(节点1,节点2,reject)
```

`Rule.policy`／`Config.fallback`与`Decision.policy`保留direct/reject/proxy枚举；Proxy的outbound可携带规范化`(节点1,节点2)`表达式。纯路由查询展示计划；实际出站成功后decision为实际选中节点或direct。结构化输入、缓存和本地规则共用同一解析约束。

候选列表中的每个节点都必须在proxies中定义：只要有一个未定义（包括首个有效、后续缺失，以及规则集policy不带括号的列表），配置检查与加载即以InvalidInput整体拒绝；运行时路由给出的列表含未配置节点时，在任何拨号之前返回Unsupported，不忽略无效项、不只用有效部分、不降级direct。无效语法、空项、重复及缺失节点使用既有InvalidInput，超限使用ResourceLimit；配置检查保留具体rules[index]或rule-providers[index].policy字段与原文位置。取消、状态、预算、TLS等错误不改号改义。所有候选失败返回实际建连失败；显式reject返回Rejected。Core不写配置、不下载文件；主配置检查只返回首个错误。可运行完整样例为仓库example_config.yaml，连接前替换虚构节点信息。

成功的TCP/UDP回环测试覆盖显式direct候选；未执行真实外部节点间的成功切换矩阵。

对应tests/test_configuration_rules.rs、test_configuration_check.rs、test_outbound_execution.rs及同名报告。真实外部节点、UDP远端投递和系统VPN未验收；连接成功不等于目标服务已处理请求。
