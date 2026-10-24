# YAML 严格解码与原文定位

## 职责（干嘛的）

来源 `src/routing/yaml.rs`。Decoder通过yaml-rust2事件实现Serde解码，限制输入大小、节点数、嵌套深度、键数量和标量长度，拒绝重复键、锚点、别名及标签。decode_diagnosed返回既有Error、脱敏字段路径和原文Unicode标量偏移；field_offset扫描字段路径；mapping_key_position按真实动态键关联映射的原文条目序号和键起始偏移。Writer负责既有有界规则输出，不参与配置检查或修复。

## 目标（想达成什么）

保持严格解码和业务校验同一错误语义，位置依据原始事件，不重排文本。dns.hosts在业务模型中是BTreeMap，语义错误选择按键排序；原文定位必须用同一真实键回查，不能把排序索引当成源文件索引。对外仅返回原文序号，不回显域名。quoted键定位到引号起始，块/流式映射与多字节输入共用同一标记机制。UTF-16和逻辑行列转换由configuration::check完成。

## 禁止行为（不允许做什么）

不得从错误文字猜路径、按排序索引定位动态映射、修改原文、公开动态键、放宽重复键/标签/深度等限制或越权读取文件。缺失键、路径错误、超长输入或解析失败时mapping_key_position返回None，不伪造位置。该入口只在主配置已成功严格解码后定位语义错误，不承担新的合法性裁决。

## 入口、资源与生命周期

decode/decode_diagnosed执行一次严格解析；field_offset执行既有字段扫描；mapping_key_position使用同一Decoder扫描一次，目标路径小于1024字节、键不超过1024字节，返回Option<(原文条目序号,Unicode标量偏移)>。目标字符串仅在本次扫描存活；不保留正文或创建活动资源。输入最大32MiB、深度32、节点3000000、单映射最多64键，主配置另受4MiB和解析预算约束。不新增网络/磁盘副作用。

内部契约见 `Documentation/API/Internal/routing/yaml_api.md`；`tests/test_routing_yaml.rs`覆盖动态映射顺序、块/流式、quoted键及非法边界；`tests/test_configuration_check.rs`和`tests/test_ffi.rs`验证原文位置及ABI透传。真实编辑器视觉换行和异步草稿丢弃由宿主负责，本模块不验证UI。
