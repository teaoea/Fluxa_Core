# VLESS、REALITY 与 Vision

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
| `fluxa` | Fluxa TLS 1.3 | 原生最小模板，仅 AES-128-GCM |

缺少 `fp` 时使用 `chrome`。未知名称，包括尚未实现的 `randomized`，明确拒绝。Android 指纹指 Chrome 浏览器，不冒充仅支持 TLS 1.2 的 Android 11 OkHttp。固定版本不表示当前所有浏览器版本完全相同，也不模拟 HTTP/2、HTTP 头、JavaScript 或操作系统。ECH 这里只是 GREASE，不是加密客户端问候服务。模板公开字段参考固定 uTLS 源码 `ff1b50fbbe9a6dff1dcb1cfc0493bd5b0f073f67`，保留其 BSD 声明；不链接 uTLS 或第三方代理内核。

## TLS、认证与资源

TLS 1.3 记录层支持 AES-128-GCM/SHA-256、AES-256-GCM/SHA-384、ChaCha20-Poly1305/SHA-256；使用 ring 密码原语。混合密钥交换使用固定 ml-kem 0.3.2，启用秘密清零，与普通 X25519 密钥独立管理。REALITY 认证执行 X25519/HKDF/AES-GCM、Ed25519 公钥 HMAC 校验、CertificateVerify 和 Finished 验证。普通站点证书不能当作 REALITY 成功。

REALITY 认证内的客户端版本字节固定为已验证的互通基线 `26.3.27`，与 crate 版本分开管理；服务端可配置版本范围，不能承诺适用于任意范围。版本元数据不表示使用了第三方内核。

每条协议连接预留 384 KiB，共享协议预算 16 MiB；连接与 FFI 原有预算仍生效。握手总量上限 128 KiB，TLS 密文记录上限 16640 字节，Vision 待解帧上限 131091 字节，业务模块无 unsafe。异步协议状态置于堆上，避免扩大 Swift 宿主线程的调用栈；任务实际退出后归还预算。未知控制帧、非法 UUID、帧长度、认证失败与限额耗尽明确失败，不直连回退。

本实现仍需要独立密码协议审计；第三方 ML-KEM 上游亦说明尚未独立审计。通过互通测试不能替代安全审计。

## 路由语义

沿用现有域名优先语义、Fake-IP 所有者和代次检查。真正连接目标使用已经检查的数字 IP，避免远端重新 DNS 后换到未经检查的候选。全部候选仍先通过既有拦截检查；域名规则命中后跳过 IP 规则的产品语义保持不变。

VLESS TCP 没有目标连接确认帧。因此自动地址族的代理出站优先选择已经通过检查、同策略同出站的 IPv4；明确 IPv6 时使用 IPv6。不会在发送内容后重放请求、切换未经检查的地址或回退直连。节点域名使用配置解析器自举，控制连接不递归代理、不调用系统 DNS。

## 已验证与边界

macOS 27.2 / Apple M4 ARM64 / Rust 1.98.1。`check-protocol.sh` 使用固定官方 Xray v26.3.27 **独立测试进程**，仅作为协议对端，不链接或交付该二进制。下载 SHA-256 为 `2e93a67e8aa1936ecefb307e120830fcbd4c643ab9b1c46a2d0838d5f8409eaf`。

各模板分别验证三种 TLS 1.3 密码套件（原生模板仅自己的 AES-128），1 MiB Rust 往返、真实内层 TLS 1.3 双向 Vision 裸流、错误公钥/Short ID/SNI/UUID 拒绝。实际 Swift FFI 覆盖 64 KiB 往返、HTTPS、重复配置回滚、XUDP 报文往返、读超时恢复、超小接收整包拒绝与继续读取、TUN UDP 经代理返回、关闭和阻塞读取取消。模板单元测试核对套件及扩展顺序、ECH 长度、GREASE、随机排列和输入边界。

本轮全量 Rust 回归 139 项通过、0 失败，独立协议测试通过专用脚本执行（常规 cargo test 中 1 项有条件忽略）；原有 11 组真实 FFI 全部通过，Swift SDK 值类型测试 13 项通过。严格 Clippy 与 SwiftLint 通过。

用户授权节点使用 `chrome`（Chrome 133）分别完成 Rust 共用 HTTPS 客户端与真实 Swift FFI 请求 `https://www.apple.com/`，返回 200，正文 253998 字节；凭据只保存在权限受限临时文件，不进入源码、文档或发布附件。

尚未实现：任意 Mux、其他传输、普通 VLESS TLS、TLS 1.2 外层 REALITY、HelloRetryRequest、会话恢复、实际 ECH、ML-DSA 附加认证、下载证书压缩帧解码、要求回发的 KeyUpdate。浏览器模板可以声明旧套件和压缩算法，但当前 REALITY 仅接受支持的 TLS 1.3 握手；不支持的服务端行为明确失败。旧浏览器模板可能被新 REALITY 服务端的握手形状限制拒绝，不静默更换用户指定模板。

TUN、系统 VPN、网关和 SOCKS/HTTP 入口不是此协议出站的验收结果。最低 macOS 14、Intel、iOS/iPadOS/tvOS 运行、实际系统隧道和 App Store 审核尚未验收。

## XUDP v2.0.0

复用 REALITY/Vision 字节流实现单目标 XUDP，独立 Rust 入口为 protocol::connect_udp，既有共用出站、C ABI、NativeOutboundClient 与 NativeTunUDPClient 自动使用同一实现。没有新增配置字段或另一套节点格式。只发送已检查的真实数字地址；不远端重解析、不隐式直连。

每个会话只包含一个目标与会话编号，首帧携带地址，后续帧保留报文边界；不提供任意 TCP 多路复用或全锥 UDP。入站携带地址时必须与批准目标一致。发送上限 7526 字节，空报文明确返回 Unsupported，超限返回 ResourceLimit；直接 UDP 的空报文能力保持不变。接收超出调用者容量时丢弃整包并报错，不返回截断内容。

XUDP 全局额外缓冲预算 4 MiB，每连接预留 32 KiB；原有 16 MiB 协议与连接预算继续适用。读取取消保留分帧缓存，写超时使连接失效，避免部分帧重发。协议错误和关闭取消读写并回收任务、缓存与预算。无新增业务 unsafe。

macOS ARM64 独立固定服务端真实互通覆盖所有已支持指纹及三种 TLS 套件（原生模板限 AES-128），Rust 验证 3、1472、7526 字节报文；真实 Swift FFI 验证往返、超限/空包/半关闭拒绝、超时恢复和 TUN UDP 经代理返回。完整 Rust 回归 158 项通过、常规套件 1 项独立服务端测试通过专用脚本执行；严格 Clippy 与完整 FFI 回归 13 组通过。此版本不限制目标 UDP 端口；本机低端口绑定权限限制，UDP/443 与真实 QUIC 应用尚未运行验收，不能用普通 UDP 回环代替 QUIC 验收。用户私有节点本轮未进行 UDP 实测。最低 macOS 14 运行、Intel、其他平台和系统 VPN 未测。

同轮回归发现 DNS 服务自动端口的历史竞争：TCP 分配的临时端口可能已被 UDP 占用，真实重复 FFI 第 44 轮捕获系统 AddrInUse。修复为仅在端口 0 且 UDP 地址占用时最多重选 8 次；固定端口冲突仍报错，其他错误不重试。新增固定冲突与双监听同端口测试，临时定位输出已移除。
