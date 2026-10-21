#!/bin/bash
# 本机手动准备 main 预发布附件；上传为显式步骤，不依赖 GitHub Actions。
set -euo pipefail
cd "$(dirname "$0")/.."
[[ "$(uname -s)" == Darwin && "$(uname -m)" == arm64 ]] || { echo '测试附件当前只支持 macOS ARM64'; exit 1; }
[[ "${FLUXA_DEV_REPOSITORY:-}" == teaoea/Fluxa_Core ]] || { echo '仓库不匹配'; exit 1; }
[[ "${FLUXA_DEV_SOURCE:-}" =~ ^[0-9a-f]{40}$ ]] || { echo '源码提交无效'; exit 1; }
[[ "$(git rev-parse HEAD)" == "$FLUXA_DEV_SOURCE" ]] || { echo '检出源码不匹配'; exit 1; }
[[ -z "$(git status --porcelain --untracked-files=all)" ]] || { echo '测试附件必须来自干净提交'; exit 1; }
version="$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -1)"
[[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+-dev$ ]] || { echo 'dev 必须使用不稳定版本号'; exit 1; }
tag="v${version}.${FLUXA_DEV_SOURCE}"
[[ "${1:-}" == --prepare && $# -eq 2 ]] || { echo '用法：Tools/dev-release.sh --prepare <全新附件目录>'; exit 1; }
[[ -n "${FLUXA_DEV_VERIFICATION:-}" ]] || { echo '缺少本机验证记录'; exit 1; }
# 本机记录必须与确定提交和当前完整源码指纹绑定；失败、未执行和旧指纹不得发布。
FLUXA_DEV_RUN="$(python3 Tools/verify-manual-release.py "$FLUXA_DEV_VERIFICATION")"
export FLUXA_DEV_RUN
mkdir "$2"
work="$(cd "$2" && pwd)"
bash build-macos.sh --output "$work/FluxaEngine.xcframework"
ditto -c -k --keepParent "$work/FluxaEngine.xcframework" "$work/fluxa-core-macos-arm64.zip"
# 独立 HTTPS 互通服务供远程二进制适配测试使用，与框架来自同一源码。
MACOSX_DEPLOYMENT_TARGET=13.0 CARGO_TARGET_DIR="$work/target" cargo build --locked --offline --release --example https_fixture
cp "$work/target/release/examples/https_fixture" "$work/fluxa-test-fixture"
(cd "$work" && zip -q fluxa-core-fixture-macos-arm64.zip fluxa-test-fixture)
python3 - "$work" "$version" "$tag" <<'PY'
import hashlib,json,os,sys
from pathlib import Path
root=Path(sys.argv[1]); sha=lambda name:hashlib.sha256((root/name).read_bytes()).hexdigest()
record={'version':sys.argv[2],'tag':sys.argv[3],'source_commit':os.environ['FLUXA_DEV_SOURCE'],'unstable':True,'platforms':['macos-arm64'],'minimum_macos':'13.0','framework_sha256':sha('fluxa-core-macos-arm64.zip'),'fixture_sha256':sha('fluxa-core-fixture-macos-arm64.zip'),'verification_run':os.environ['FLUXA_DEV_RUN']}
(root/'provenance.json').write_text(json.dumps(record,ensure_ascii=False,indent=2)+'\n')
(root/'checksum.txt').write_text(record['framework_sha256']+'\n')
(root/'fixture-checksum.txt').write_text(record['fixture_sha256']+'\n')
(root/'notes.md').write_text('不稳定测试版本，仅供开发验证；不代表系统 VPN 已完成或正式发布。\n\n源码：'+record['source_commit']+'\n基础版本：'+record['version']+'\n平台：macOS ARM64，最低部署 13.0（最低系统运行未验收）。\n框架 SHA-256：'+record['framework_sha256']+'\n检查：'+record['verification_run']+'\n\n不发布 crates.io，不移动正式 latest，不提供 iOS/tvOS/Intel 切片。\n')
PY
# 公开仓库仅登记来源清单及 Release 附件；源码、测试与构建脚本不向外复制。
printf '已保存本地附件：%s\n' "$work"
printf '仅本地保存；未经用户明确许可，禁止上传或创建远程草稿。\n'
# 有现成的有限分发凭据时，也可显式运行 publish-core-binaries.py；不读取 Actions secret。
