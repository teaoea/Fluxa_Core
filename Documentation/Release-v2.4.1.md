# v2.4.1 GeoIP 修复包发布记录

作者：可莉唯一的狗。

本次只交付内核修复包，没有修改 App 或更新其固定内核依赖，没有合并 main。修复使用用户明确配置的 geoip_url，无默认下载地址；未配置为 WARN + Disabled，明确配置后失败为 ERROR + Err，成功加载为 INFO + Ready，合法未匹配是非错误结果。

## 已发布身份

- 构建及 crate 源码提交：`56d4d135a4fea832dc8ff9fd730f547830500ddc`。
- 最终标签：`v2.4.1`；标签提交：`f2171f8ffc5c3021c6fd2500555cca87c6b55b1f`。
- 标签相对构建源码仅更新 Package.swift 的框架校验值。
- [crates.io 2.4.1](https://crates.io/crates/libfluxa_core/2.4.1)：实际上传成功，并独立下载核对归档逐字节一致。
- [GitHub v2.4.1](https://github.com/teaoea/Fluxa_Core/releases/tag/v2.4.1)：框架、测试附件及来源记录实际发布。
- [最终发布流程](https://github.com/teaoea/Fluxa_Core/actions/runs/38552586805)：所有步骤成功，包含最终源码的 Rust、Swift FFI、独立协议互通及远程包下载验证。

| 交付物 | 独立下载 SHA-256 |
| --- | --- |
| libfluxa_core-2.4.1.crate | `41ec0d12d652fc5f0d2dc9a800119541d99e21833420cd9ec84ee42ce7f3b4d7` |
| FluxaEngine.xcframework.zip | `33f6a1aad6c8639a020fe98348395ebfa2b857264e8d2d75c38a14b37ae0f387` |
| fluxa-test-fixture-macos-arm64.zip | `d00acc22aa19686217a4678e2412891dbe492d1432d2dc959a31b889453a389c` |

附件实际下载后与 provenance.json、checksum.txt、fixture-checksum.txt 一致；远程 v2.4.1 Package.swift 的固定 URL 与框架校验值一致。干净归档的 VCS 来源与关键源码、示例、冻结契约均已核对；未把本地其它许可证/VPN 规范修改混入归档。

## 机器处理代码

| 代码 | 含义与实际结果 |
| --- | --- |
| 2401 | URL 格式非法；拒绝配置，保留旧有效状态 |
| 2402 | URL 协议不允许；仅接受 HTTPS |
| 2403 | 数据库下载失败；DNS、连接、TLS、HTTP 状态等底层分类保留在固定诊断环 |
| 2404 | 数据库下载超时；实际 60 秒延迟 TLS 响应测试通过 |
| 2405 | 数据库格式非法；空内容、非 MMDB 或不支持编码拒绝加载 |
| 2406 | 数据库完整性失败；截断或可达结构损坏拒绝加载，不冒充来源签名验证 |
| 2407 | 数据库资源超限；长度、预算、分配及校验工作量限制不可绕过 |
| 2408 | 数据库初始化/状态不可用；配置了来源但未加载或已关闭时查询失败 |
| 2409 | 查询执行失败；与合法无记录区分 |
| 2410 | WARN + Disabled，非 Err；不初始化数据库、不下载、不解析查询目标 DNS |
| 2411 | INFO + Ready，非 Err；当前来源的有效 MMDB 已加载或查询成功 |
| 2412 | no-match，非 Err；合法 IP 没有记录，返回明确状态 |

中央定义为 src/error.rs。现行登记 40 项，其中 37 个执行错误、3 个非错误状态；查询表、机器 Schema 和冻结契约自动检查通过，重复、遗漏、未定义和必要字段缺失均为 0。状态代码不会通过内部 Error 返回，不能把所有 code 误当成执行异常。

## 实际验证与 L1–L4

- L1：cargo fmt --check、cargo check --offline --all-targets、cargo clippy --offline --all-targets -- -D warnings 通过。
- L2：Rust 全量 223 通过、0 失败、1 忽略；错误契约 17 项；真实 Swift FFI 全部 17 组通过。
- 真正发布归档中错误契约 17 项与 GeoIP 5 项通过，包含真实 TLS 的 60 秒下载期限。
- 独立固定官方对端 37 轮 Rust 与 37 轮实际 Swift 通过；官方附件从同一公开 Release 下载，既定 SHA-256 校验通过。最初的下载握手/截断失败没有记作通过。
- GeoIP 下载实际验证超过普通 HTTP 8 MiB 的 MMDB；普通 HTTP 限额保持。搜索树损坏、非法 URL/协议、资源超限、HTTP 失败、截断、合法无匹配与实际字段故障均按独立预期验证。
- 发布前复现合法 uint128 附加字段误拒；未上传候选 38550605402 已取消，修正后重新验证再发布，未使用旧候选源码或归档。
- L3：以用户最终状态表核对配置、来源隔离、失败/非错误状态、代码映射及资源路径，证据详见 GeoIP-v2.4.1-Review.md。
- L4：本记录的实际归档来源、远程下载、哈希、流程、测试与代码提供证据；不能由文档一致推导全项目正确。

## 迁移、回滚与边界

C ABI 仍为 2；JSON 新增可选 warn、geoip_status、level。geoip_open 不接受旧 path，Swift open(database:) 改为 open()/refresh()，先配置来源。Rust Error 增加用户指定的九种类别，穷尽匹配调用方需更新。两个完整 YAML 示例和可运行 geoip_configuration 示例同步。App 未接入新字段，不作为本修复包已实现项目。

同来源更新失败显式返回 Err，并保留旧有效库以符合原子替换；换来源/移除来源不使用旧库。回滚须将 SDK 与框架一起固定回上一版本，并移除不支持的新配置字段。

最低部署系统、其它平台、真实用户数据库/来源、系统 VPN、全部内存故障与高并发压力未验证。完整性校验覆盖响应、布局、分隔区与可达记录；未提供外部预期哈希/签名，不能宣称来源认证或地理真实性。H1、Debug 完整出站优先级及宿主鉴权是既有独立待办，没有在本包修复。
