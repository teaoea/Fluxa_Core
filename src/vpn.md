# VPN 候选资产准备代码说明

## 职责（干嘛的）

对应源码：`src/vpn.rs`。该模块严格只读加载宿主准备的配置和资源，异步编译隔离候选，查询结果并处理关闭，不发布活动配置、不下载文件、不接管系统网络。

`Preparation::begin`接受有界YAML；`begin_file`先验证storage::Resources，再有界读取明确授权配置。调用configuration::Document严格解析、preparation_preflight，再经outbound::Manager::prepare_vpn_candidate加载宿主只读资源。Authority管理候选身份、取消与实际活动操作，ByteBudget约束全局两个候选及输入预算，总准备期限120秒。

`Status`展示Preparing/CandidateReady/Failed/Closing/Closed，配置摘要只在候选就绪时返回，activation_permitted始终false。关闭先撤销身份，等待后台操作停止和Manager::stop，再释放槽并完成关闭；失败保留error_info与cleanup_error_info，不把Arc析构当作业务关闭完成。Drop只请求内核清理。宿主持有真实读取权限并提供不可变版本文件；配置不能扩大授权路径。

例：`Preparation::begin("tun: {enable: true, name: nodus0}\nrules: ['final,direct']")`后查询status，最终显式await close。该例只是候选准备，无节点仍需要后续可信授权。对应测试`tests/test_vpn.rs`及同名报告包含隔离配置、文件错误、关闭和槽位回收。候选不能替代真实出站/DNS探测、权限/预算/防回环检查或系统验证。

新增四阶段驱动位于子模块transaction，其真实实现、状态机与未接入范围见[transaction.md](vpn/transaction.md)。现有候选API没有因此改变为系统启动API。

## 目标（想达成什么）

构造可取消、受预算约束的隔离配置候选，并提供可追溯状态；候选准备不发布活动配置，也不授予系统接管资格。

## 禁止行为（不允许做什么）

不得将 CandidateReady 或 preparation 成功当作 Running／activation_permitted，不得下载落盘、改写正式配置或绕过后续必要链路与系统授权验证。

## 真实网络前置检查

Controller 的 begin_preflight／preflight_status 复用同一事务槽，真实探测与未就绪边界见[preflight.md](vpn/preflight.md)。候选准备成功仍不代表完整前置检查通过。
