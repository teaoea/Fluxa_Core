# GeoIP 配置来源与查询契约

作者：可莉唯一的狗。

v2.4.1 修复 GeoIP 来源管理：数据库下载只使用用户配置的 geoip_url，没有默认 URL、默认厂商或宿主文件回退。Rust 负责 HTTPS、DNS/路由、下载预算、MMDB 校验、状态和查询；随包 Swift SDK 只传递配置及读取结果。本轮不修改 Fluxa App，其现有配置解析器和固定内核版本不会自动获得新字段。

## 配置与使用示例

```yaml
# 这是主配置中的可选字段；缺少或 null 表示用户未启用。
geoip_url: null
# 启用时换成自己的原始 MMDB HTTPS 下载地址，不填 gzip/zip 包。
# geoip_url: https://ssss.aa.com
```

YAML 使用冒号，不使用等号。显式空串、控制符、超过 1024 字节、无有效主机、凭据、片段、端口 0 或格式非法返回 2401；非 HTTPS 协议返回 2402。整个非法配置拒绝，不改变旧来源、数据库或网络配置。主配置片段的未知字段、重复键沿用严格解析错误；Config::from_yaml 仅解析 GeoIP 片段，不能把完整主配置传入这个片段接口。

Rust 可运行示例为 examples/geoip_configuration.rs；运行 cargo run --example geoip_configuration，会打印 Disabled 状态，不下载占位地址。实际下载使用 Service::new(Config)、Service::refresh 和已配置 DNS/路由的 HttpsClient。完整主配置示例 fluxa.yaml 与 fluxa_english.yaml 同步提供该字段。

C ABI 命令仍经现有 ABI 2 JSON 协议：

```json
{"operation":"geoip_configure","geoip_url":"https://ssss.aa.com"}
```

配置只校验、保存来源；network_configure 也接受顶层 geoip_url。下载必须显式调用 geoip_refresh 或无参数 geoip_open，完成后查询 geoip_lookup。配置命令不隐式发起网络请求。修改或移除来源立即替换独立服务对象，旧下载不会更新新对象；新来源必须重新加载，不能使用旧来源数据库。

```json
{"operation":"geoip_refresh"}
{"operation":"geoip_lookup","address":"8.8.8.8","language":"zh-CN"}
```

随包 Swift SDK 使用 configureSource(url:)、refresh()/open() 与 lookupResult(address:language:)。旧 open(database:) 已迁移为 open()，geoip_open 的旧 path 参数会被严格拒绝。Database::open 是 Rust 开发/测试用离线原语，不构成公开命令的数据源回退。App 未修改；后续 App 接入时由其解析主配置并把字段传给此 API。

## 等级、状态与整数代码

所有公开处理结果都有中央登记的四位整数及有界空参数；状态与执行错误分离。唯一权威定义为 src/error.rs，查询表与 error-codes.json 自动生成。

| 条件 | 等级/结果 | 代码 | 实际处理 |
| --- | --- | --- | --- |
| 字段缺失或 null | WARN + Disabled，非 Err | 2410 | 不初始化 MMDB、不下载、不解析查询目标 DNS |
| URL 为空或格式非法 | ERROR + Err | 2401 | 拒绝整次配置 |
| 非 HTTPS | ERROR + Err | 2402 | 拒绝配置，不降级 HTTP |
| DNS、连接、TLS、非 200 HTTP 状态或其它访问失败 | ERROR + Err | 2403 | 下载失败；底层代码保留在固定诊断环 |
| 下载总期限耗尽 | ERROR + Err | 2404 | 取消下载，释放临时资源 |
| 非 MMDB、空正文或不支持内容编码 | ERROR + Err | 2405 | 拒绝加载 |
| 响应截断、可达数据结构损坏或验证失败 | ERROR + Err | 2406 | 拒绝加载 |
| 长度、分配、预算或校验工作量超限 | ERROR + Err | 2407 | 拒绝加载，不绕过预算 |
| 当前来源数据库未加载、关闭或状态不可用 | ERROR + Err | 2408 | 查询失败，不转成 Disabled |
| 查询/记录字段解码失败 | ERROR + Err | 2409 | 不能冒充合法无匹配 |
| 完整加载成功或取得有效查询记录 | INFO + Ready，非 Err | 2411 | 可正常查询 |
| 查询合法 IP，无记录 | 非错误 no-match | 2412 | 返回明确未匹配，无虚构国家或城市 |

示例响应：

```json
{"success":true,"geoip_status":{"state":"disabled","level":"warn","code":2410,"params":{}},"warn":{"level":"warn","code":2410,"params":{}}}
{"success":false,"level":"error","error_info":{"code":2408,"params":{}}}
{"success":true,"geoip_status":{"state":"no-match","level":"info","code":2412,"params":{}}}
```

纯配置已明确 URL 但尚未下载时配置提交可以成功；查询返回 2408。没有配置查询返回 Disabled，不作为失败。更新失败显式返回 Err，并保留同来源的当前有效库以维持原子替换契约；保留旧库不等于把更新失败转换成成功。调用方必须处理该 Err，不能静默吞没。换来源时不保留旧库。

## 资源、完整性及安全边界

单库/下载最多 256 MiB，共用数据库预算 512 MiB；下载之前预留最大正文预算，完成后缩减到实际 Vec 容量，校验或取消失败由 RAII 释放。普通 HTTP 8 MiB 限额保持不变；数据库专用头及正文入口才使用更大限额。头仍最多 32 KiB，不跟随重定向，不提供跳过证书验证，不解压归档。

MMDB 识别检查元数据标记与格式；初始化检查元数据及布局边界；加载前遍历全部可达网络并解码全部记录。验证不构造整份 JSON、记录对象树或 visited HashSet；上游解码深度及每次容器限制保留，共用工作量上限 64 Mi 单位，防止共享引用导致超量工作。2406 表示响应/可达结构完整性，不表示已经验证外部预期 SHA-256、来源签名或真实地理数据准确性；URL 配置未提供这些认证材料，不能虚构已验证。

数据库失败与正式路由、Debug Overlay、DNS 决策、REJECT 无关。公开错误不包含 URL、IP、凭据或内部路径。固定诊断环保存受控底层代码，等级由中央 ErrorCode 分类决定；没有无界错误日志或错误参数。

公网 IP 回显仍是独立的现有能力，与 GeoIP 数据源无关，本次未修改。系统 VPN、所有平台/最低系统、真实用户数据库及来源授权不以回环测试替代验证。
