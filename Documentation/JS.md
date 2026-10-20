# JS 引擎

## 当前能力与边界

第三阶段提供同步 JSON 转换引擎。Rust 使用固定版本 rquickjs 0.14.0 的安全 API，底层为其内嵌的 QuickJS C 解释器，不使用 JIT。网络内核仍为 Fluxa 自有实现，不引入代理内核。业务代码没有新增 unsafe。Swift 通过共用 NativeCommandClient 经现有 C ABI 调用，未增加 Swift 网络实现或 UI。

每次执行新建 Runtime 和 Context，结束即释放；不保存脚本全局变量，不返回 JS 对象地址。输入 JSON 会作为一个参数传入同步函数表达式，例如：

```javascript
input => ({status: input.status, body: input.body.toUpperCase()})
```

允许普通 JSON 数据：null、布尔值、有限数、字符串、数组和普通对象。不支持 Promise 返回值、异步作业、Date、BigInt、函数、undefined、Symbol、循环对象、稀疏数组、getter 和不可枚举数据属性。校验和序列化使用预先捕获的内建方法，并冻结相关原型；不调用用户 toJSON 或 getter，防止静默遗漏数据。JS 数字遵循 ECMAScript 双精度语义，大整数标识应使用字符串。

没有注册文件、网络、DOM、计时器、通知、系统命令、原生函数或模块加载器，没有启用动态库、异步桥接等可选特性。模块导入无法获取系统资源。浏览器 DOM 属于浏览器扩展阶段，不由这个运行时直接操作。

## 资源限制

| 项目 | 限制 |
| --- | --- |
| 源码 | 256 KiB |
| 输入和输出 JSON | 各 1 MiB |
| JSON 深度和数据节点 | 64 层、16384 节点 |
| 单运行时堆 | 32 MiB，由 QuickJS 分配限制控制 |
| 运行时栈 | 512 KiB |
| 执行预算 | 250 ms，解释器中断回调检查 |
| 全局运行时预算 | 四份 32 MiB 配额，超限拒绝执行 |

输入先做非递归深度检查，编码时限制写入长度。输出先在受限运行时中校验，再检查文本长度，转为 Rust 拥有的 JSON 数据。配额、Runtime、Context 和 FFI 缓冲区均由 RAII 回收；任何错误之后可以继续执行新脚本。

执行预算涵盖解释器准备、源码执行和结果校验；中断是协作式检查，不是操作系统硬实时抢占。内建 C 操作、编译、析构和主机编码可能使总调用时间超过 250 ms。32 MiB 也不是整个进程的 RSS 上限：主机 JSON、FFI 缓冲区、栈和依赖分配另计。当前无独立工作进程隔离，不承诺抵御底层解释器漏洞；JS 堆中的敏感副本也不保证逐字节清零。因此只应执行由宿主/用户明确选择且信任来源的脚本，不能把本实现当作任意恶意脚本的安全执行服务。

## Swift 与 C ABI

`NativeScriptClient.execute(source:input:)` 接收源码和 JSON Data，返回独立拥有的 JSON Data；同步调用应在后台执行。同一实例重入仍由既有 Rust busy guard 拒绝。

内部 ABI 命令为 `operation: script`，包含 `source` 和 `input`，响应的 `script_result_json` 是 JSON 文本。该字段用字符串封装以准确区分 `null` 结果和字段缺失。ABI 为 2；capabilities 的 JS 能力位仍为 2，完整位图以当前二进制查询为准。新增 script_cache_clear 命令需要固定到包含该能力的版本，不能用旧二进制已有的 JS 位推断新命令可用。响应继续使用统一句柄、复制、释放接口；旧 HTTPS/MITM 响应结构兼容。内部 JSON 不是用户配置格式，用户配置继续使用 YAML。

## App Store 约束

按 Apple [审核指南 2.5.2 与 4.7](https://developer.apple.com/cn/app-store/review/guidelines/)设计：原第三阶段只接入应用随附脚本；当前 dev 已按用户要求增加声明式远程资源，由 Core 下载到真实共享容器、按 when 和命名绑定调用，格式见《插件与脚本》。这不增加任何原生系统能力，也不代表 App 分发版本已完成接入或审核。核心库的源码执行接口本身不能鉴别来源，分发客户端必须在配置/插件导入层执行来源与能力政策。同步 execute 源码门面不负责下载；远程安装属于独立资源事务。未修改客户端 UI、系统功能或 App 接入。

受限解释器不自动意味着符合全部审核要求。提交版本仍需要完整披露、隐私与 VPN 要求、实际功能检查和 Apple 审核；不存在测试通过即可保证过审的结论。

## 验证

```sh
cargo test --manifest-path ./Cargo.toml --offline
bash ./check-ffi.sh script
bash ./check-ffi.sh all
bash ./build-macos.sh
swift test --package-path .
```

Rust 验证 JSON 转换、中文、null、原型键、非法结果、超时、递归、内存耗尽、环境隔离、无环境 IO、大小/深度/节点和全局配额。实际 Swift 客户端链接 Rust 静态库验证 JSON 转换、超时与内存失败恢复、隔离、null 和重复生命周期，并回归 HTTPS/MITM。macOS package 测试通过真实二进制调用，不以模拟实现替代。

当前只验收本机 macOS ARM64。最低 macOS 14 的实际运行、其他架构和 Apple 平台、UI 接入及 App Store 审核未验收。本轮已接入规则条件、命名绑定、按需源缓存、工作线程与请求/响应管线，具体约束、默认 js-ttl 和测试边界见 [插件与脚本](插件与脚本.md)。历史第三阶段验证不能替代本轮最终指纹的测试报告。
