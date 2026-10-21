#!/bin/bash
# macOS ARM64 本地构建入口；最低 13.0，不执行上传。
set -euo pipefail
cd "$(dirname "$0")"
exec python3 Tools/build-apple.py --platform macos "$@"
