#!/bin/bash
# 全部受支持 Apple 平台的 ARM64 本地构建入口；不包含 Intel、watchOS 或 visionOS。
set -euo pipefail
cd "$(dirname "$0")"
exec python3 Tools/build-apple.py "$@"
