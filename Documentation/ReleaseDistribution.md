> 2026-10-23：以下为历史发布流程，不作为当前操作入口。内核已开源为 `teaoea/Nodus-Core`，后续二进制在同仓库分发，本轮暂不启用。当前以 [发布与分发规范](../AGENTS/发布与分发规范.md) 为准。

# 二进制分发流程

作者：可莉唯一的狗。2026-10-19。

强制政策见 [发布与分发规范](../AGENTS/发布与分发规范.md)。永久停止 crates.io；两个私有源码仓库不再承载二进制 Release。

## 内核开发版本

每次私有 dev 提交先通过 macOS Rust 与 Linux 数据面门禁，再在同一提交构建 macOS ARM64 框架及独立测试服务。`Tools/dev-release.sh` 只构建固定成品，`Tools/publish-core-binaries.py` 只向 `teaoea/Nodus-Core` 操作。

配置私有源码仓库 Actions secret `NODUS_BINARY_PUBLISH_TOKEN`：限定分发仓库 Contents 读写、有限有效期，无源码仓库或 crates.io 权限。令牌不可用时明确失败，不回退原源码仓库。令牌创建与实际权限尚需独立配置验证，离线测试不能证明已经配置。

标签示例：`v2.6.2-dev.<完整源码提交>`。分发仓库 Git 仅写 `versions/<标签>/provenance.json`；标签指向该元数据提交，清单中 `source_commit` 指向真实私有内核源码。GitHub 自动生成的源码 ZIP 因而只包含这些清单，不包含 Rust、Swift、测试或构建脚本。

Release 唯一附件白名单：

- `NodusEngine.xcframework.zip`：静态框架、必要公开 ABI 声明和许可证。
- `nodus-test-fixture-macos-arm64.zip`：编译后的独立测试服务。
- `provenance.json`：版本、源码、平台、门禁链接及两份 ZIP 的 SHA-256。

脚本拒绝未知清单字段、实现源码、构建脚本、非法归档路径和超预算尺寸。先创建草稿，上传并重新下载校验，最后公开为 Pre-release，不设置稳定 latest。重跑只验证既有公开版本；冲突、损坏和未完成草稿明确报错，不删除或覆盖附件，不移动标签。

## App 渠道

`teaoea/fluxa-app` 只分发 App 成品：预览版与测试版标记 Pre-release；稳定版仍须已授权 main 来源、完整回归和适用签名/公证。Swift 封装、App 实现与工作流留在私有 Fluxa 仓库。App 继续使用固定的 Nodus-Core 版本 URL 与 SHA-256，不依赖本地 Rust 源码或构建。

本次不伪造尚未完成的稳定发布入口；旧的源码仓库手动发布工作流已移除。实际公开上传、匿名下载、App 依赖迁移、签名与系统 VPN 均须分别记录，不能用本地发布控制流测试代替。

历史 crates.io 与源码仓库 Release 保留原记录；本次不擅自撤回或删除。新版本不能继续走历史路径。

## 首轮远端失败记录

938903d 的 macOS Rust 门禁通过；Linux 门禁在编译阶段报 `No space left on device`，退出 101，未运行相应断言，发布任务被跳过。原始证据为 [运行 39183028964](https://github.com/teaoea/Nodus-Core/actions/runs/39183028964)，工件 `nodus-linux-tcp-39183028964-1`，ID 12156918209，下载 ZIP SHA-256 `d326889c7fc5e140b2074a0c6498c24dfb176a6de94d2533d5e10481e49054ac`。

后续仅对 Linux 门禁设置无调试符号、禁用增量缓存和最多两个编译任务；保留原有测试范围、优化级别、调试断言及溢出检查，不删除失败证据或把编译失败改记为通过。真实重跑结果另行核实。
