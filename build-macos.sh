#!/bin/bash
# 本机 macOS 静态库；最终二进制 libfluxa_core，以 FluxaEngine.xcframework 接入。
set -euo pipefail
cd "$(dirname "$0")"
[[ "$(uname -s)" == Darwin ]] || { echo '此打包器仅用于 macOS'; exit 1; }
build_directory="$(mktemp -d /tmp/fluxa-macos-native.XXXXXX)"
trap 'rm -rf "$build_directory"' EXIT
MACOSX_DEPLOYMENT_TARGET=14.0 cargo build --locked --offline --release --target-dir "$build_directory"
framework="$build_directory/libfluxa_core.framework"
mkdir -p "$framework/Headers" "$framework/Modules" "$framework/Resources/Notices"
cp -R Notices/. "$framework/Resources/Notices/"
cp LICESE.md "$framework/Resources/Notices/"
# Cargo 的中间文件转为 framework 内无扩展名的静态二进制，不生成动态库。
mv "$build_directory/release/liblibfluxa_core.a" "$framework/libfluxa_core"
cp include/fluxa_engine.h include/fluxa_debug_ipc.h "$framework/Headers/"
sed 's/^module /framework module /' include/module.modulemap > "$framework/Modules/module.modulemap"
cat > "$framework/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleIdentifier</key><string>app.fluxa.engine</string>
<key>CFBundleName</key><string>libfluxa_core</string>
<key>CFBundleExecutable</key><string>libfluxa_core</string>
<key>CFBundlePackageType</key><string>FMWK</string>
<key>CFBundleVersion</key><string>1</string>
<key>CFBundleShortVersionString</key><string>0.1.0</string>
<key>CFBundleSupportedPlatforms</key><array><string>MacOSX</string></array>
<key>MinimumOSVersion</key><string>14.0</string>
</dict></plist>
PLIST
version="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
# Apple 标准版本字段仅使用数值版本；完整预发布版本和源码另行记录。
plutil -replace CFBundleShortVersionString -string "${version%%-*}" "$framework/Info.plist"
plutil -insert FluxaCoreVersion -string "$version" "$framework/Info.plist"
plutil -insert FluxaSourceCommit -string "$(git rev-parse HEAD)" "$framework/Info.plist"
xcodebuild -create-xcframework -framework "$framework" -output "$build_directory/FluxaEngine.xcframework"
mkdir -p Vendor
# 仅替换本构建器拥有的忽略产物；同时清理旧名称，避免选错库。
for artifact in Vendor/FluxaEngine.xcframework Vendor/FluxaRust.xcframework; do
  if [[ -d "$artifact" ]]; then rm -rf "$artifact"; fi
done
mv "$build_directory/FluxaEngine.xcframework" Vendor/FluxaEngine.xcframework
