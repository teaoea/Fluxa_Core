# dev 不稳定测试附件

作者：可莉唯一的狗。

用户已要求 dev 每次提交同步发布不稳定测试版本。本流程在 dev 推送后执行；同一提交必须先通过
Rust、报告/发布控制流回归、Linux 真实 TUN 及分片检查，随后构建并发布 GitHub Pre-release。
不同提交的运行互不取消，避免后续提交取消已经开始的测试发布。

- 基础内核版本为 `2.5.0-dev`；每个下载版本标签为 `v2.5.0-dev.<完整源码提交>`。
- 标签的提交、框架构建提交及 provenance.json 的 source_commit 必须一致。
- 每次版本单独保存 XCFramework、独立测试服务、SHA-256 与来源记录。框架的 Info.plist 也保存完整基础版本和源码。
- 不移动正式 latest，不上传源码仓库中的大型二进制，不发布 crates.io，不持有 crates.io 凭据，不合并 main。
- 已发布附件不可覆盖。重跑只核对已有来源、标签和下载 SHA-256；失败不伪报成功。
- 构建后先显式创建并核验本提交 Git 标签；草稿不能充当标签已经存在的证明。
- 上传时先建立本提交草稿；附件全部上传并重新下载校验成功后，才公开为 Pre-release。中断留下的本提交草稿允许重建。
- 当前只有 macOS ARM64 切片，部署目标 14.0；最低系统运行、Intel、iOS/tvOS 及真实系统 VPN 不因附件发布而视为通过。
- App 固定引用经验证的具体标签 URL 与 SHA-256，不使用 latest URL，不因后台出现新测试附件自动漂移。

自动入口为 `.github/workflows/tests.yml` 的 dev-release 作业，执行 `Tools/dev-release.sh`。
正式二进制工作流仅保留 main 手动入口；不能通过修改旧 Release.version 从 dev 触发正式发布。
GitHub Token 只对测试附件作业临时提供 contents:write；普通测试作业仍为只读。

离线控制流测试：

```sh
python3 Tools/test_reports.py --external test_dev_release -- python3 tests/test_dev_release.py
```

该测试使用本地 gh/cargo/打包桩，只证明拒绝条件、附件不可覆盖及草稿公开顺序。
真实云端构建、上传、标签与公开下载必须另行核验，不能以离线桩通过冒充已经发布。

## 打包许可回归

构建器从当前根目录的 `LICENSE` 与 `LICENSE.md` 复制自有许可和依赖声明，并保留 `Notices/`。缺少文件在编译前拒绝；不再使用已移除的 `LICESE.md`。`test_framework_preserves_current_license_files` 运行真实打包脚本的复制流程并逐字节校验许可，不以发布器桩替代文件打包检查。CI `39082177460` 的真实构建因旧许可路径失败，未产生可用附件；修复后的实际构建与后续 CI 单独验收。
