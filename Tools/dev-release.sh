#!/bin/bash
# 每个已通过门禁的 dev 提交只发布 GitHub 不稳定附件；无 crates.io 凭据或发布命令。
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
work="$(mktemp -d /tmp/fluxa-dev-release.XXXXXX)"
trap 'rm -rf "$work"' EXIT
verify_tag() {
  if gh api "repos/$FLUXA_DEV_REPOSITORY/git/ref/tags/$tag" > "$work/tag.json" 2>/dev/null; then
    python3 - "$work/tag.json" "$FLUXA_DEV_SOURCE" <<'PYCODE'
import json,sys
record=json.load(open(sys.argv[1])); assert record['object']['type']=='commit' and record['object']['sha']==sys.argv[2]
PYCODE
  else
    [[ "${1:-required}" == allow_missing ]] || { echo '无法确认版本标签的真实源码'; return 1; }
  fi
}
verify_tag allow_missing
# 已发布附件不可覆盖。重复执行验证已有来源与下载校验；仅本提交的未发布草稿允许重新准备。
if gh release view "$tag" --repo "$FLUXA_DEV_REPOSITORY" --json isDraft,isPrerelease > "$work/existing.json" 2>/dev/null; then
  if [[ "$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["isDraft"])' "$work/existing.json")" == True ]]; then
    gh release delete "$tag" --repo "$FLUXA_DEV_REPOSITORY" --yes
  else
    verify_tag required
    python3 - "$work/existing.json" <<'PYCODE'
import json,sys
assert json.load(open(sys.argv[1]))['isPrerelease'] is True
PYCODE
    gh release download "$tag" --repo "$FLUXA_DEV_REPOSITORY" --pattern provenance.json --pattern FluxaEngine.xcframework.zip --dir "$work"
    python3 - "$work" "$FLUXA_DEV_SOURCE" "$tag" "$version" <<'PY'
import hashlib,json,sys
from pathlib import Path
root=Path(sys.argv[1]); p=json.loads((root/'provenance.json').read_text())
assert p['source_commit']==sys.argv[2] and p['tag']==sys.argv[3] and p['version']==sys.argv[4] and p['unstable'] is True
assert hashlib.sha256((root/'FluxaEngine.xcframework.zip').read_bytes()).hexdigest()==p['framework_sha256']
PY
    echo '已有不稳定测试附件来源及下载校验通过，保留原附件'
    exit 0
  fi
fi
bash build-macos.sh
ditto -c -k --keepParent Vendor/FluxaEngine.xcframework "$work/FluxaEngine.xcframework.zip"
# 独立 HTTPS 互通服务供远程二进制适配测试使用，与框架来自同一源码。
CARGO_TARGET_DIR="$work/target" cargo build --locked --offline --release --example https_fixture
cp "$work/target/release/examples/https_fixture" "$work/fluxa-test-fixture"
(cd "$work" && zip -q fluxa-test-fixture-macos-arm64.zip fluxa-test-fixture)
python3 - "$work" "$version" "$tag" <<'PY'
import hashlib,json,os,sys
from pathlib import Path
root=Path(sys.argv[1]); sha=lambda name:hashlib.sha256((root/name).read_bytes()).hexdigest()
record={'version':sys.argv[2],'tag':sys.argv[3],'source_commit':os.environ['FLUXA_DEV_SOURCE'],'unstable':True,'platforms':['macos-arm64'],'minimum_macos':'14.0','framework_sha256':sha('FluxaEngine.xcframework.zip'),'fixture_sha256':sha('fluxa-test-fixture-macos-arm64.zip'),'verification_run':os.environ['FLUXA_DEV_RUN']}
(root/'provenance.json').write_text(json.dumps(record,ensure_ascii=False,indent=2)+'\n')
(root/'checksum.txt').write_text(record['framework_sha256']+'\n')
(root/'fixture-checksum.txt').write_text(record['fixture_sha256']+'\n')
(root/'notes.md').write_text('不稳定测试版本，仅供开发验证；不代表系统 VPN 已完成或正式发布。\n\n源码：'+record['source_commit']+'\n基础版本：'+record['version']+'\n平台：macOS ARM64，最低部署 14.0（最低系统运行未验收）。\n框架 SHA-256：'+record['framework_sha256']+'\n检查：'+record['verification_run']+'\n\n不发布 crates.io，不移动正式 latest，不提供 iOS/tvOS/Intel 切片。\n')
PY
# 草稿期间附件不作为可用版本交付；全部上传成功后再公开为 Pre-release。
gh release create "$tag" "$work/FluxaEngine.xcframework.zip" "$work/fluxa-test-fixture-macos-arm64.zip" "$work/checksum.txt" "$work/fixture-checksum.txt" "$work/provenance.json" --repo "$FLUXA_DEV_REPOSITORY" --target "$FLUXA_DEV_SOURCE" --draft --prerelease --latest=false --title "libfluxa_core $version · ${FLUXA_DEV_SOURCE:0:7}（不稳定测试）" --notes-file "$work/notes.md"
verify_tag required
mkdir "$work/download"
gh release download "$tag" --repo "$FLUXA_DEV_REPOSITORY" --pattern FluxaEngine.xcframework.zip --dir "$work/download"
[[ "$(shasum -a 256 "$work/download/FluxaEngine.xcframework.zip" | awk '{print $1}')" == "$(cat "$work/checksum.txt")" ]] || { echo '发布下载校验失败'; exit 1; }

gh release edit "$tag" --repo "$FLUXA_DEV_REPOSITORY" --draft=false --prerelease --latest=false
