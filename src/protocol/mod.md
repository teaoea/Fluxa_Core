# mod.rs 代码说明

## 职责（干嘛的）

对应源码：`src/protocol/mod.rs`。本页说明实现职责、数据流程和边界；公开调用的请求／响应契约单独维护于 `Documentation/`，开发约束维护于 `AGENTS/`。

## 目标（想达成什么）

将已支持的 VLESS／REALITY／Vision 与 XUDP 处理组织为共用出站可调用的协议实现。

## 主要流程与模块协作

config 严格校验节点 URI 与凭据，fingerprint／crypto／reality 完成握手基础，vless／vision／stream 组织流，datagram 处理有界数据报；未知组合明确拒绝，不用其他内核替代。

## 输入、输出与失败边界

输入来自受约束的配置、明确授权的资源或校验后的网络数据；各入口的实际类型、字段和值域以下文及对应源码为准。正常结果只能说明该模块完成其职责，不能推定网络接管、其他模块初始化或外部权限已经成功。非法输入、协议错误、系统故障、取消和资源不足沿用 `src/error.rs` 的真实分类；不静默改为 DIRECT，也不修复或覆盖外部配置。

活动资源由 Core 的会话身份、有效租约、取消及明确关闭操作管理；只读快照引用不构成活动权限。内存与任务的实际存活期计入对应预算。具体模块没有活动任务时，不因本说明增加后台线程或额外权限。

## 测试与示例入口

相关测试按 `Tools/test-suites.json` 的实际源码映射核对，不以缺失的同名文件声称覆盖。

真实示例入口：[protocol_block.rs](../../examples/protocol_block.rs)。

性能结果及执行记录保存在对应测试文档；下文涉及旧版本的验证陈述仅是历史资料，不是本次重新执行的结果。已发布版本的历史能力不能替代当前代码、真实平台或系统 VPN 验收。

## 实现细节

作者：可莉唯一的狗。

## 实现与接入

自研 Rust 客户端支持 VLESS + TCP + REALITY + `xtls-rprx-vision`。普通 TCP 数据与内层 TLS 1.3 流量共用连接、路由、DNS、配额、超时和取消机制。内层 TLS 1.3 可以在完整记录边界切换到双向 Vision 裸流，普通数据结束填充后继续使用外层 TLS。

配置沿用完整主配置 `nodes` 的 `name`/`uri`，不新增第二套节点格式。宿主将节点稳定标识和完整 URI 放入 `network_configure.outbounds`，每项为 `{id, uri}`；最多 128 项，默认空数组。路由中的 `outbound` 引用这个标识。重复标识、不支持的组合或任何非法节点使整次配置失败，旧快照保持有效。Rust API 为 `Manager::configure_network_with_outbounds`，独立协议入口为 `protocol::connect`。不记录 URI、UUID 或密钥。

## 指纹

| 名称 | 固定模板 | 行为 |
| --- | --- | --- |
| `chrome`、`chrome133` | Chrome 133 | 随机扩展顺序、GREASE、ECH GREASE、X25519MLKEM768、ALPS 新扩展 |
| `chrome131` | Chrome 131 | 混合密钥交换、旧 ALPS 扩展 |
| `chrome120` | Chrome 120 | X25519、ECH GREASE |
| `firefox`、`firefox120` | Firefox 120 | 固定扩展顺序、X25519 与 P-256 双密钥份额、ECH GREASE |
| `safari`、`safari16` | Safari 16.0 | Apple 套件顺序、GREASE、zlib 证书压缩声明 |
| `ios`、`ios14` | iOS 14 | 独立 Apple 套件列表、固定扩展顺序 |
| `android`、`android-chrome133` | Android Chrome 133 | Chrome 浏览器模板，支持混合密钥交换 |
| `android-chrome120` | Android Chrome 120 | 明确保留旧模板 |
| `edge`、`edge106` | Edge 106 | 固定扩展顺序、ALPS、GREASE |
| `random` | 已支持的六种浏览器模板之一 | 在配置校验时选择，快照中的节点保持同一模板 |
| `nodus` | Nodus TLS 1.3 | 原生最小模板，仅 AES-128-GCM |

缺少 `fp` 时使用 `chrome`。未知名称，包括尚未实现的 `randomized`，明确拒绝。Android 指纹指 Chrome 浏览器，不冒充仅支持 TLS 1.2 的 Android 11 OkHttp。固定版本不表示当前所有浏览器版本完全相同，也不模拟 HTTP/2、HTTP 头、JavaScript 或操作系统。ECH 这里只是 GREASE，不是加密客户端问候服务。模板公开字段参考固定 uTLS 源码 `ff1b50fbbe9a6dff1dcb1cfc0493bd5b0f073f67`，保留其 BSD 声明；不链接 uTLS 或第三方代理内核。

## TLS、认证与资源

TLS 1.3 记录层支持 AES-128-GCM/SHA-256、AES-256-GCM/SHA-384、ChaCha20-Poly1305/SHA-256；使用 ring 密码原语。混合密钥交换使用固定 ml-kem 0.3.2，启用秘密清零，与普通 X25519 密钥独立管理。REALITY 认证执行 X25519/HKDF/AES-GCM、Ed25519 公钥 HMAC 校验、CertificateVerify 和 Finished 验证。普通站点证书不能当作 REALITY 成功。

REALITY 认证内的客户端版本字节固定为已验证的互通基线 `26.3.27`，与 crate 版本分开管理；服务端可配置版本范围，不能承诺适用于任意范围。版本元数据不表示使用了第三方内核。

每条协议连接预留 384 KiB，共享协议预算 16 MiB；连接与 FFI 原有预算仍生效。握手总量上限 128 KiB，TLS 密文记录上限 16640 字节，Vision 待解帧上限 131091 字节，业务模块无 unsafe。异步协议状态置于堆上，避免扩大 Swift 宿主线程的调用栈；任务实际退出后归还预算。未知控制帧、非法 UUID、帧长度、认证失败与限额耗尽明确失败，不直连回退。

本实现仍需要独立密码协议审计；第三方 ML-KEM 上游亦说明尚未独立审计。通过互通测试不能替代安全审计。

## 路由语义

沿用现有域名优先语义、Fake-IP 所有者和代次检查。真正连接目标使用已经检查的数字 IP，避免远端重新 DNS 后换到未经检查的候选。全部候选仍先通过既有拦截检查；域名规则命中后跳过 IP 规则的产品语义保持不变。

VLESS TCP 没有目标连接确认帧。因此自动地址族的代理出站优先选择已经通过检查、同策略同出站的 IPv4；明确 IPv6 时使用 IPv6。不会在发送内容后重放请求、切换未经检查的地址或回退直连。节点域名使用配置解析器自举，控制连接不递归代理、不调用系统 DNS。

## XUDP v2.0.0

复用 REALITY/Vision 字节流实现单目标 XUDP，独立 Rust 入口为 protocol::connect_udp，既有共用出站、C ABI、NativeOutboundClient 与 NativeTunUDPClient 自动使用同一实现。没有新增配置字段或另一套节点格式。只发送已检查的真实数字地址；不远端重解析、不隐式直连。

每个会话只包含一个目标与会话编号，首帧携带地址，后续帧保留报文边界；不提供任意 TCP 多路复用或全锥 UDP。入站携带地址时必须与批准目标一致。发送上限 7526 字节，空报文明确返回 Unsupported，超限返回 ResourceLimit；直接 UDP 的空报文能力保持不变。接收超出调用者容量时丢弃整包并报错，不返回截断内容。

XUDP 全局额外缓冲预算 4 MiB，每连接预留 32 KiB；原有 16 MiB 协议与连接预算继续适用。读取取消保留分帧缓存，写超时使连接失效，避免部分帧重发。协议错误和关闭取消读写并回收任务、缓存与预算。无新增业务 unsafe。

macOS ARM64 独立固定服务端真实互通覆盖所有已支持指纹及三种 TLS 套件（原生模板限 AES-128），Rust 验证 3、1472、7526 字节报文；真实 Swift FFI 验证往返、超限/空包/半关闭拒绝、超时恢复和 TUN UDP 经代理返回。完整 Rust 回归 158 项通过、常规套件 1 项独立服务端测试通过专用脚本执行；严格 Clippy 与完整 FFI 回归 13 组通过。此版本不限制目标 UDP 端口；本机低端口绑定权限限制，UDP/443 与真实 QUIC 应用尚未运行验收，不能用普通 UDP 回环代替 QUIC 验收。用户私有节点本轮未进行 UDP 实测。最低 macOS 14 运行、Intel、其他平台和系统 VPN 未测。

同轮回归发现 DNS 服务自动端口的历史竞争：TCP 分配的临时端口可能已被 UDP 占用，真实重复 FFI 第 44 轮捕获系统 AddrInUse。修复为仅在端口 0 且 UDP 地址占用时最多重选 8 次；固定端口冲突仍报错，其他错误不重试。新增固定冲突与双监听同端口测试，临时定位输出已移除。

XUDP 发布来源：crate 源码 4c041a8bfcc522b9a6de43fa59231c11b25aa6d0，v2.0.0 最终包提交 22afb215f95ac678c9d880b6313d2c393825a45a；框架 SHA-256 edea9d318ed7d6ad319dcc24f90fb6d92e45920f055913df1fc7fe1c3db4e95c。GitHub 工作流 38202687152 首次成功，实际下载 crate 源码和二进制均已核对。DNS 修复后 200 轮实际 FFI 全部通过。Fluxa v2.2.0 固定远程 v2.0.0，并通过独立 Chrome 指纹服务端的共用出站与 TUN UDP 诊断测试；main 未合并。

## 禁止行为（不允许做什么）

不得容忍未知节点参数或不支持的协议组合、暴露节点凭据、绕过握手身份校验，或失败后改走未经允许的出口。

违反输入、授权或资源边界时，使用实际模块的既有错误／拒绝路径，不伪造成功或擅自扩大权限；具体失败后果以上文流程和公开契约为准。
