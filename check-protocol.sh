#!/bin/bash
# 独立官方服务端仅用于协议互通；固定版本和校验值，不链接或打包进内核。
set -euo pipefail
cd "$(dirname "$0")"
[[ "$(uname -s)" == Darwin && "$(uname -m)" == arm64 ]] || { echo '此互通脚本当前仅提供 macOS ARM64 对端；其他平台未测'; exit 1; }
umask 077
check_directory="$(mktemp -d /tmp/fluxa-protocol-check.XXXXXX)"
fixture_pid=''
peer_pid=''
cleanup() {
  for pid in "$peer_pid" "$fixture_pid"; do
    if [[ -n "$pid" ]]; then kill "$pid" 2>/dev/null || true; wait "$pid" 2>/dev/null || true; fi
  done
  rm -rf "$check_directory"
}
trap cleanup EXIT
check_target="${CARGO_TARGET_DIR:-$check_directory/rust}"
cargo build --locked --offline --target-dir "$check_target" --lib --example https_fixture
curl -fsSL --retry 2 https://github.com/XTLS/Xray-core/releases/download/v26.3.27/Xray-macos-arm64-v8a.zip -o "$check_directory/peer.zip"
[[ "$(shasum -a 256 "$check_directory/peer.zip" | awk '{print $1}')" == 2e93a67e8aa1936ecefb307e120830fcbd4c643ab9b1c46a2d0838d5f8409eaf ]] || { echo '独立服务端下载校验失败'; exit 1; }
unzip -q "$check_directory/peer.zip" -d "$check_directory/peer"
for suite in 4865 4866 4867; do
echo "验证 TLS 密码套件：$suite"
"$check_target/debug/examples/https_fixture" "$suite" > "$check_directory/fixture.json" &
fixture_pid=$!
for attempt in {1..100}; do
  [[ -s "$check_directory/fixture.json" ]] && break
  kill -0 "$fixture_pid" 2>/dev/null || { echo '回环服务启动失败'; exit 1; }
  sleep 0.1
done
"$check_directory/peer/xray" x25519 > "$check_directory/keys.txt"
python3 - "$check_directory" <<'PYCODE'
import json, pathlib, socket, sys, uuid
p=pathlib.Path(sys.argv[1]); keys={}
for line in (p/'keys.txt').read_text().splitlines():
    key,_,value=line.partition(':'); keys[key.strip().lower()]=value.strip()
f=json.loads((p/'fixture.json').read_text())
with socket.socket() as s:
    s.bind(('127.0.0.1',0)); port=s.getsockname()[1]
u=str(uuid.uuid4())
server={'log':{'loglevel':'warning'},'inbounds':[{'listen':'127.0.0.1','port':port,'protocol':'vless','settings':{'clients':[{'id':u,'flow':'xtls-rprx-vision'}],'decryption':'none'},'streamSettings':{'network':'tcp','security':'reality','realitySettings':{'target':'127.0.0.1:'+str(f['port']),'serverNames':['localhost'],'privateKey':keys['privatekey'],'shortIds':['1234567890abcdef']}}}],'outbounds':[{'protocol':'freedom'}]}
(p/'server.json').write_text(json.dumps(server))
(p/'profile.json').write_text(json.dumps({'id':'interop','uri':f"vless://{u}@127.0.0.1:{port}?security=reality&type=tcp&encryption=none&flow=xtls-rprx-vision&sni=localhost&pbk={keys['password (publickey)']}&sid=1234567890abcdef&fp=fluxa"}))
PYCODE
"$check_directory/peer/xray" run -config "$check_directory/server.json" > "$check_directory/peer.log" 2>&1 &
peer_pid=$!
python3 - "$check_directory" <<'PYCODE'
import json,pathlib,socket,sys,time,urllib.parse
p=pathlib.Path(sys.argv[1]);u=urllib.parse.urlparse(json.loads((p/'profile.json').read_text())['uri'])
for attempt in range(100):
    try:
        with socket.create_connection((u.hostname,u.port),timeout=.2): break
    except OSError: time.sleep(.1)
else: raise SystemExit('独立服务端启动失败')
PYCODE
swiftc -I include Sources/FluxaCore/DNSConfiguration.swift Sources/FluxaCore/OutboundLimits.swift Sources/FluxaCore/Routing*.swift Sources/FluxaCore/CoreError.swift Sources/FluxaEngine/Native*Client.swift Sources/FluxaEngine/NativeOutboundResources.swift Tests/ffi_protocol.swift -Xlinker "$check_target/debug/liblibfluxa_core.a" -framework Security -framework CoreFoundation -o "$check_directory/client"
echo_port="$(python3 -c 'import json,sys;print(json.load(open(sys.argv[1]))["outbound_tcp_port"])' "$check_directory/fixture.json")"
for fingerprint in fluxa chrome chrome120 chrome131 chrome133 firefox safari ios android android-chrome120 android-chrome133 edge random; do
  [[ "$fingerprint" == fluxa && "$suite" != 4865 ]] && continue
  python3 - "$check_directory/profile.json" "$fingerprint" <<'PYCODE'
import json,pathlib,re,sys
p=pathlib.Path(sys.argv[1]);v=json.loads(p.read_text());v['uri']=re.sub(r'fp=[^&]+','fp='+sys.argv[2],v['uri']);p.write_text(json.dumps(v))
PYCODE
  echo "验证指纹：$fingerprint"
  FLUXA_PROTOCOL_PROFILE="$check_directory/profile.json" FLUXA_PROTOCOL_FIXTURE="$check_directory/fixture.json" cargo test --locked --offline --target-dir "$check_target" --test protocol -- --include-ignored
  "$check_directory/client" "$check_directory/profile.json" "$check_directory/fixture.json" "$echo_port"
done

kill "$peer_pid" "$fixture_pid"
wait "$peer_pid" 2>/dev/null || true
wait "$fixture_pid" 2>/dev/null || true
peer_pid=''
fixture_pid=''
rm -f "$check_directory/fixture.json"
done
