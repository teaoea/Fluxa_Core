# 流量规则与内置脚本处理

当前版本 0.2.0 在真实 HTTP 与 HTTPS MITM 共用转发管线中执行规则。普通请求先执行拒绝和请求头规则，响应按配置顺序执行响应头、文本替换或内置脚本。域名精确匹配，路径按前缀匹配；不扩展到子域名。

## JSON 响应脚本

新增 response-script 动作，script 仅接受编译进内核的 remove-json-keys，json_keys 为需要移除的根对象键列表。参数只描述数据，不接受源码、URL、文件路径或下载脚本。

内置 JS 在受限运行时中选择要移除的字段；Rust 保留其他原始 JSON 数值，避免大整数与高精度小数经过 JS Number 丢失精度。不递归扫描嵌套对象，不按名称猜测广告字段。没有命中键时保留原响应字节和校验头；正文改变后移除旧 Content-Length、ETag、Content-MD5 与 Digest，转发时重新计算长度。日志保留实际转发的处理结果。

限制：只接受未压缩的 application/json 或 application/*+json 根对象，正文最多 256 KiB，键最多 64 个、每个最多 256 字节且不重复。非 JSON、数组、压缩正文、错误配置或资源超限明确失败，不静默透传。正文有效时可能重新序列化，空白和字段顺序不保证保留。

脚本共享已有 250 ms 时间、32 MiB 堆、512 KiB 栈和全局四份运行时预算；放在阻塞工作线程执行，不占用 socket I/O 线程。取消后已开始的阻塞执行会在自身时间上限内结束。业务代码没有新增 unsafe。

## 接口

Rust Rewrite 与 Script 提供独立调用入口，process_response 用于异步管线。C ABI v1 的 mitm_start 扩展 rewrites 字段，能力位 4 表示内置流量脚本；旧字段兼容。Swift NativeRewriteRule 新增 script 与 jsonKeys 可选参数，匹配、执行和 JSON 正文处理均在 Rust。

用户 YAML 写为 script: remove-json-keys 和 json-keys: [ad]；C ABI 内部使用 json_keys，不是用户配置文件。Rust 源码不下载远程脚本，客户端也不得把任意下载源码传入脚本接口。

普通 HTTP 请求经过上游 HTTP 代理使用 absolute-form；HTTPS 保留 CONNECT。代理错误不回退直连。当前调试监听仍只允许回环，LAN、认证、SOCKS、TUN、网关、DNS 路由与代理出站协议仍未交付。

## 验证

2026-10-10，macOS ARM64：Rust 38 项测试与 Clippy 通过，真实 Swift FFI 四组通过。新增真实 HTTP/HTTPS MITM 互通验证 JSON 字段移除、相邻及嵌套字段保留、日志正文、HTTP 转发长度与后续规则；实际 Swift FFI 验证 HTTPS 脚本处理和日志。独立测试覆盖无匹配原文、高精度数字、未知脚本、重复/空参数、类型与压缩拒绝及正文上限。旧 HTTPS、MITM、JS 与生命周期回归保留。

未验证最低 macOS 14 运行、其他平台/架构、真实系统浏览器信任、真机隧道和网关。受限内置脚本方案不代表已通过 App Store 审核；公开发行仍需按实际行为披露和审核。
