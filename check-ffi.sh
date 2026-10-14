#!/bin/bash
# 统一检查实际 Swift → Rust FFI；按需启动回环 TLS 服务，清理所有临时输出。
set -euo pipefail
cd "$(dirname "$0")"
case "${1:-all}" in
  error_code|response_retry|tun_dns|protocol_block|lifecycle|https|mitm|script|tun|tun_udp|dns|dns_service|routing|outbound|runtime_rules|subscription|geoip) client_tests=("$1") ;;
  all) client_tests=(error_code response_retry tun_dns protocol_block lifecycle https mitm script tun tun_udp dns dns_service routing outbound runtime_rules subscription geoip) ;;
  *) echo '支持 error_code、response_retry、tun_dns、protocol_block、lifecycle、https、mitm、script、tun、tun_udp、dns、dns_service、routing、outbound、runtime_rules、subscription、geoip 或 all'; exit 1 ;;
esac
check_directory="$(mktemp -d /tmp/fluxa-ffi-check.XXXXXX)"
fixture_pid=''
cleanup() {
  if [[ -n "$fixture_pid" ]]; then kill "$fixture_pid" 2>/dev/null || true; wait "$fixture_pid" 2>/dev/null || true; fi
  rm -rf "$check_directory"
}
trap cleanup EXIT
build_targets=(--lib --example geoip_fixture)
needs_fixture=false
for client_test in "${client_tests[@]}"; do
  if [[ "$client_test" == geoip || "$client_test" == response_retry || "$client_test" == tun_dns || "$client_test" == protocol_block || "$client_test" == https || "$client_test" == mitm || "$client_test" == dns || "$client_test" == dns_service || "$client_test" == tun_udp || "$client_test" == outbound || "$client_test" == subscription ]]; then needs_fixture=true; fi
done
if $needs_fixture; then build_targets+=(--example https_fixture); fi
cargo build --locked --offline --target-dir "$check_directory/rust" "${build_targets[@]}"
mv "$check_directory/rust/debug/liblibfluxa_core.a" "$check_directory/rust/debug/libfluxa_core"
if $needs_fixture; then
  "$check_directory/rust/debug/examples/https_fixture" > "$check_directory/fixture.json" &
  fixture_pid=$!
  for attempt in {1..100}; do
    [[ -s "$check_directory/fixture.json" ]] && break
    kill -0 "$fixture_pid" 2>/dev/null || { echo 'TLS 测试服务启动失败'; exit 1; }
    sleep 0.1
  done
  [[ -s "$check_directory/fixture.json" ]] || { echo 'TLS 测试服务启动超时'; exit 1; }
fi
"$check_directory/rust/debug/examples/geoip_fixture" "$check_directory/geoip.mmdb"
for client_test in "${client_tests[@]}"; do
  if [[ "$client_test" == lifecycle ]]; then
    swift_sources=(-import-objc-header include/fluxa_engine.h)
  else
    swift_sources=(-I include Sources/FluxaCore/DNSConfiguration.swift Sources/FluxaCore/OutboundLimits.swift Sources/FluxaCore/Routing*.swift Sources/FluxaCore/CoreError.swift Sources/FluxaEngine/Native*Client.swift Sources/FluxaEngine/NativeOutboundResources.swift)
  fi
  CLANG_MODULE_CACHE_PATH="$check_directory/modules" swiftc \
    -module-cache-path "$check_directory/modules" "${swift_sources[@]}" \
    "Tests/ffi_$client_test.swift" -Xlinker "$check_directory/rust/debug/libfluxa_core" \
    -framework Security -framework CoreFoundation -o "$check_directory/client-ffi"
  if [[ "$client_test" == geoip ]]; then
    "$check_directory/client-ffi" "$check_directory/geoip.mmdb" "$check_directory/fixture.json"
  else
    "$check_directory/client-ffi" "$check_directory/fixture.json"
  fi
done
