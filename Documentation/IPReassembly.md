# TUN 入站 IP 重组与 IPv6 扩展头

作者：可莉唯一的狗。开发范围：dev 2.5.0 的 Rust 数据包会话，不表示已发布或已接管系统 VPN。

## 数据路径与语义

宿主仍通过 `Session::submit` / `nodus_core_tun_submit` 提交完整单个 IP 包，TCP/UDP 使用同一重组入口。
未分片且无扩展头的包借用原输入，不额外复制；需要处理的包规范化为基础 IP 头及完整传输段，
随后执行既有 TCP/UDP 校验和、DNS/Fake-IP、路由和出站。未收齐片段只表示已暂存，不能作为转发成功证据。

- IPv4/IPv6 乱序分片按端点、标识、协议及映射代次隔离；IPv6 原子片段单独处理。
- 重叠片段废弃整个数据报，保留截止期限内墓碑；重复包不延长截止时间。
- 首片必须包含完整的剩余扩展链和 TCP/UDP 头；重组完成后仍校验传输层校验和。
- 同一组片段基础头不一致时拒绝，不能借改变头字段建立不同重组并绕过重叠保护。
- 网络配置 revision 改变时丢弃未完成重组；不能把旧配置的片段接入新配置。
- IPv6 Hop-by-Hop、Destination Options 受数量和长度限制。未知选项只有 action=00 才按规范跳过；
  其余明确拒绝。Routing Header 仅接受非弃用类型且 Segments Left=0；不执行源路由。
- AH、ESP、Jumbogram、未知下一头和需要额外处理的选项仍拒绝，不剥掉安全语义后假装支持。
- 原有无状态 `tun::parse` / PacketSession 仍只检查完整基础包；有状态重组属于 Session，不给诊断入口隐式分配重组表。

## 资源、取消与错误

| 限制 | 当前值与后果 |
| --- | --- |
| 每个入包 | 必须不超过会话 MTU（1280–9000） |
| 每个重组完整包 | 最多 9000 字节，超过明确返回资源错误，不截断 |
| 每会话重组条目 | 最多 32，包括重叠墓碑 |
| 全局重组字节预算 | 2 MiB，每个活动条目预留 18512 字节，覆盖双缓冲及控制开销 |
| 片段存活时间 | 从首个片段进入起 60 秒；每会话一项登记任务每秒清理，另在提交时清理 |
| 扩展链 | 最多 8 个头、256 字节；至多一个首位 Hop-by-Hop、一个 Routing、两个 Destination Options |

包队列继续持有既有 4 MiB 包预算，不重复声称其属于重组预算。活动条目释放后租约归还；墓碑和空表
仍有受 32 条/既有会话数上限限制的元数据。上述预算不是进程 RSS 上限。

关闭会话由既有 Authority 撤销和任务登记执行；重组清理 guard 在任务未首次轮询或被 abort 时也释放
真实缓冲。重复/并发关闭完成后，保留旧 Session 引用不能延长活动重组预算，也不能再次提交。

复用现有统一错误：非法包为 InvalidInput，未支持语义为 Unsupported，数量/大小/字节超限为现有资源错误，
关闭后为 InvalidState/Cancelled，锁故障为 Internal 并进入既有诊断。不新增错误码或 YAML 字段，不改变
DIRECT/PROXY/REJECT、DNS 优先级或原 FFI 缓冲所有权。

## 可运行示例与验收入口

```sh
python3 Tools/test_reports.py --run --suite test_tun_ip
```

`tests/test_tun_ip.rs` 独立构造 IPv4/IPv6 包并启动本机 UDP 对端；尾片先到时对端不能收到数据，首片到齐后
必须收到完整 24 字节正文。非法/重叠输入不能提前拨号，关闭后继续提交必须失败。

真实 Swift FFI 在 Fluxa 仓库执行：

```sh
NODUS_CORE_ROOT=/实际受测内核目录 bash Packages/FluxaKernelBridge/check-ffi.sh all
```

`test_ffi_tun_tcp.swift` 在 20 次 TCP 生命周期中把 1000 字节应用正文拆成乱序 IP 片段，经真实回环或独立代理
对端往返后逐字节核对。上述源码联调不是 App 的生产本地依赖。

本阶段不宣称输出 IP 分片、ICMP/PMTU、TCP 选项扩展、完整 RFC 栈或 macOS 系统 VPN 已实现。
实际结果、失败、源码指纹及未测边界以逐文件报告和本次审查记录为准。
