# 受管理 TCP 流

## 职责（干嘛的）

对应源码：`src/outbound/stream.rs`。把共用出站建立的 TCP 连接转换为可交给 TLS／HTTP 的 ManagedStream，复用连接注册表、所有权、取消及流量计数。不是第二套拨号、路由或权限管线。

## 目标（想达成什么）

调用方保留流对象时，内核关闭仍能撤销活动权限、停止真实套接字，并阻止后续读写；同一次连接固定配置快照及实际路由决策，后续配置变更不能让该流改走其他出口。

## 禁止行为（不允许做什么）

禁止将 Arc 存活等同于活动资源占用权，禁止关闭后从旧对象恢复 Socket，禁止失败回退直连或绕过 DNS、路由和连接预算。不得因为 TLS 持有流而跳过确定关闭。

## 类型、调用和所有权

Manager::connect_stream 捕获同一 NetworkSnapshot，再调用 connect_stream_snapshot，经 connect_configured_snapshot_hop 沿用实际路由、解析和代理握手。成功后从注册表连接中移动 TCP 读写端，登记 Detached 流槽；Manager 仍持有撤销权。构造失败显式关闭连接，不保留半成品。

ManagedStream 持有连接权限与流槽、弱管理器引用、只读真实 Decision 以及读写取消观察器。每次 AsyncRead／AsyncWrite 先取得 operation、检查取消，再执行 I/O 与流量计数；不是仅在首次创建检查一次权限。流析构撤销／释放自己的连接引用，显式关闭仍由 Manager 的完整生命周期完成。

## 前置探测及错误

内部 decision() 返回实际连接 Decision 的副本，供 HTTPS 探测在目标 TLS／HTTP 成功后核对必要代理覆盖；常规建连直接移动既有 Decision，不重复分配节点名称。该摘要不能修改连接策略或恢复活动资源。

输入、配置、DNS、规则、代理及连接错误沿既有 Error 返回，流关闭转换为有原错误原因的 I/O 失败；不能把取消变成 EOF 成功或把 REJECT 当作连接失败后重试其他策略。

## 示例、测试与边界

HTTPS／服务管线使用本流，示例见`examples/vpn_preflight.rs`；对应测试`tests/test_outbound_stream.rs`验证旧流仍存活时关闭和资源释放，真实前置探测见`tests/test_vpn_preflight.rs`。该流不是系统 TUN 接管、防回环或系统 VPN 可用性证明；最低系统与外部真实代理端验收须另有证据。
