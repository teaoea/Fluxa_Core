# 主配置字段模型

## 职责（干嘛的）

models.rs定义Raw及代理、规则来源、DNS等严格反序列化字段；业务校验与执行分别由configuration和outbound负责。本轮仅改变RuleProvider.enable的缺省值为true，不改变PluginProvider.enabled。

## 目标（想达成什么）

用户省略规则集enable时按默认启用解释；显式false仍不参与来源，禁用不能绕过字段及策略检查。policy保留字符串表示，后续交由统一有界候选解析器校验。

## 禁止行为（不允许做什么）

未知/重复字段仍拒绝；模型不下载文件、不创建目录、不执行网络；不以字段存在授予系统权限或Running。

## 生命周期、错误与验证

Raw持有主配置解析数据；敏感代理UUID沿用Drop清零约定，不新增活动资源所有权。RuleProvider的policy非空/引用与上限由规则校验负责；enable缺省函数仅返回true。tests/test_configuration_check.rs验证缺省启用、合法策略及原文错误位置；其他字段保留既有测试，不宣称全模型已重新独立审查。
