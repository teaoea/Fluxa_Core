#!/usr/bin/env python3
"""macOS 实际加固签名、跨进程 Swift IPC 验证；不安装系统策略，不修改 App。"""
import json
import os
import pathlib
import platform
import shutil
import subprocess
import tempfile

ROOT = pathlib.Path(__file__).resolve().parents[1]


def run(arguments, **options):
    return subprocess.run(arguments, cwd=ROOT, check=True, **options)


def signed_requirement(path, identifier):
    run(["codesign", "--force", "--sign", "-", "--identifier", identifier,
         "--options", "runtime", str(path)])
    result = run(["codesign", "-d", "--verbose=4", str(path)], capture_output=True, text=True)
    code_hash = next(line.split("=", 1)[1] for line in result.stderr.splitlines()
                     if line.startswith("CDHash="))
    return 'cdhash H"' + code_hash + '"'


def main():
    if platform.system() != "Darwin":
        raise SystemExit("本检查需要 macOS 公共安全框架，其他平台未验证。")
    build = run(["cargo", "test", "--locked", "--offline", "--lib", "--no-run",
                 "--message-format=json"], capture_output=True, text=True)
    artifacts = [json.loads(line) for line in build.stdout.splitlines() if line.strip()]
    executable = next(item["executable"] for item in artifacts
                      if item.get("reason") == "compiler-artifact" and item.get("executable")
                      and item.get("target", {}).get("name") == "libfluxa_core")
    run(["cargo", "build", "--locked", "--offline", "--lib"])
    target = pathlib.Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target"))
    if not target.is_absolute():
        target = ROOT / target
    with tempfile.TemporaryDirectory(prefix="fluxa-debug-signed-") as temporary:
        directory = pathlib.Path(temporary)
        rust = directory / "rust-tests"
        swift = directory / "swift-tests"
        shutil.copy2(executable, rust)
        sources = ["Sources/FluxaCore/DNSConfiguration.swift", "Sources/FluxaCore/OutboundLimits.swift",
                   "Sources/FluxaCore/CoreError.swift"]
        sources += [str(path.relative_to(ROOT)) for pattern in
                    ["Sources/FluxaCore/Routing*.swift", "Sources/FluxaEngine/Native*Client.swift"]
                    for path in sorted(ROOT.glob(pattern))]
        sources += ["Sources/FluxaEngine/NativeOutboundResources.swift"]
        run(["swiftc", "-I", "include", *sources, "Tests/ffi_debug_ipc.swift", "-Xlinker",
             str(target / "debug/liblibfluxa_core.a"), "-framework", "Security", "-framework",
             "CoreFoundation", "-o", str(swift)])
        # 先证明普通业务 API 无法创建 Debug 授权，再运行固定代码哈希的签名联调。
        run([str(swift)])
        environment = os.environ.copy()
        environment["FLUXA_DEBUG_TEST_REQUIREMENT"] = signed_requirement(rust, "fluxa-debug-test")
        environment["FLUXA_DEBUG_TEST_SWIFT"] = str(swift)
        environment["FLUXA_DEBUG_TEST_SWIFT_REQUIREMENT"] = signed_requirement(swift, "fluxa-debug-swift-test")
        run([str(rust), "signed_os_peer_and_operation_permissions_use_real_unix_socket",
             "--nocapture", "--test-threads=1"], env=environment)
    print("签名 IPC、实际 Swift 跨进程与普通接口隔离全部通过；不代表 App 安装/沙箱验收。")


if __name__ == "__main__":
    main()
