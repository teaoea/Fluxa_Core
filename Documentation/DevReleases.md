# dev 本地产物与获准后的测试发布

默认只在本地构建、校验和保存，未经用户明确许可不上传二进制或配套文件，不创建远程发布草稿、标签。源码统一 main；-dev/dev 仅表示二进制预发布等级，通过门禁或推送 main 不构成发布授权；不使用 GitHub Actions 发布，永久禁止 crates.io，已发布契约变更另行授权。

- Apple 支持矩阵及五种 ARM64 切片见 [Apple 平台构建](Apple平台构建.md)。Mac 最低 13.0，仅 Apple Silicon，不支持 Intel、Mac Catalyst、watchOS 或 visionOS。
- 既有 macOS 独立附件入口保留，来源清单只声明实际构建的 macOS ARM64。全平台本地产物不得使用该单平台清单冒充完整来源证明。
- 只有获准上传后才执行 [本机手动发布](本机手动发布.md)；来源、版本、SHA-256、草稿下载校验、Pre-release 与稳定 latest 隔离继续强制。
- 离线控制流回归由 python3 Tools/test_reports.py --external test_dev_release -- python3 tests/test_dev_release.py 执行；不证明真实上传或公开下载已完成。

## 打包许可回归

构建器从当前根目录的 `LICENSE` 与 `LICENSE.md` 复制自有许可和依赖声明，并保留 `Notices/`。缺少文件在编译前拒绝；不再使用已移除的 `LICESE.md`。`test_framework_preserves_current_license_files` 运行真实打包脚本的复制流程并逐字节校验许可，不以发布器桩替代文件打包检查。CI `39082177460` 的真实构建因旧许可路径失败，未产生可用附件；修复后的实际构建与后续 CI 单独验收。
