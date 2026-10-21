#!/usr/bin/env python3
"""构建用户约定的 Apple ARM64 静态切片；仅保存本地产物，不执行上传。"""
import argparse
import json
import os
from pathlib import Path
import plistlib
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
# 名称、Rust 目标、SDK、部署变量、版本、Apple 平台、平台变体。
PLATFORMS = (
    ('macos', 'aarch64-apple-darwin', 'macosx', 'MACOSX_DEPLOYMENT_TARGET', '13.0', 'MacOSX', None),
    ('ios', 'aarch64-apple-ios', 'iphoneos', 'IPHONEOS_DEPLOYMENT_TARGET', '16.0', 'iPhoneOS', None),
    ('ios-simulator', 'aarch64-apple-ios-sim', 'iphonesimulator', 'IPHONEOS_DEPLOYMENT_TARGET', '16.0', 'iPhoneSimulator', 'simulator'),
    ('tvos', 'aarch64-apple-tvos', 'appletvos', 'TVOS_DEPLOYMENT_TARGET', '17.0', 'AppleTVOS', None),
    ('tvos-simulator', 'aarch64-apple-tvos-sim', 'appletvsimulator', 'TVOS_DEPLOYMENT_TARGET', '17.0', 'AppleTVSimulator', 'simulator'),
)
DEPLOYMENT_KEYS = ('MACOSX_DEPLOYMENT_TARGET', 'IPHONEOS_DEPLOYMENT_TARGET',
                   'TVOS_DEPLOYMENT_TARGET', 'WATCHOS_DEPLOYMENT_TARGET', 'XROS_DEPLOYMENT_TARGET')


def environment(row):
    # 不继承调用方残留的部署目标、SDK 或 C 编译标志，避免高版本 SDK 污染最低系统。
    env = dict(os.environ)
    for key in (*DEPLOYMENT_KEYS, 'SDKROOT', 'CFLAGS', 'CXXFLAGS', 'RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS'):
        env.pop(key, None)
    env[row[3]] = row[4]
    env['SDKROOT'] = subprocess.check_output(['xcrun', '--sdk', row[2], '--show-sdk-path'], text=True).strip()
    return env


def build(selected, output):
    if os.uname().sysname != 'Darwin':
        raise ValueError('Apple 成品必须使用 macOS/Xcode 构建')
    for name in ('LICENSE', 'LICENSE.md', 'Notices', 'include/fluxa_engine.h', 'include/fluxa_debug_ipc.h', 'include/module.modulemap'):
        if not (ROOT / name).exists():
            raise ValueError('缺少许可证、声明或 C ABI 头文件：' + name)
    version = next(line.split('"')[1] for line in (ROOT / 'Cargo.toml').read_text().splitlines() if line.startswith('version = '))
    source = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip()
    dirty = bool(subprocess.check_output(['git', 'status', '--porcelain'], cwd=ROOT))
    output = output.resolve()
    output.parent.mkdir(parents=True, exist_ok=True)
    if output.exists() or output.is_symlink():
        raise ValueError('本地输出必须是全新路径，禁止覆盖旧成品：' + str(output))
    with tempfile.TemporaryDirectory(prefix='fluxa-apple-framework-') as tmp:
        temporary = Path(tmp)
        args = ['xcodebuild', '-create-xcframework']
        for row in selected:
            name, target, _, _, minimum, platform, variant = row
            print('构建 ' + name + ' / ' + target + ' / 最低 ' + minimum, flush=True)
            env = environment(row)
            # 固定 target，并为该平台的 Rust 与原生依赖使用同一部署环境。
            subprocess.run(['cargo', 'build', '--locked', '--offline', '--release', '--lib',
                            '--target', target, '--target-dir', str(ROOT / 'target/apple')], cwd=ROOT, env=env, check=True)
            framework = temporary / name / 'fluxa_core.framework'
            for part in ('Headers', 'Modules', 'Resources/Notices'):
                (framework / part).mkdir(parents=True, exist_ok=True)
            shutil.copytree(ROOT / 'Notices', framework / 'Resources/Notices', dirs_exist_ok=True)
            for license_file in ('LICENSE', 'LICENSE.md'):
                shutil.copyfile(ROOT / license_file, framework / 'Resources/Notices' / license_file)
            for header in ('fluxa_engine.h', 'fluxa_debug_ipc.h'):
                shutil.copyfile(ROOT / 'include' / header, framework / 'Headers' / header)
            (framework / 'Modules/module.modulemap').write_text((ROOT / 'include/module.modulemap').read_text().replace('module ', 'framework module ', 1))
            shutil.copyfile(ROOT / 'target/apple' / target / 'release/libfluxa_core.a', framework / 'fluxa_core')
            info = dict(CFBundleIdentifier='app.fluxa.engine', CFBundleName='fluxa_core',
                        CFBundleExecutable='fluxa_core', CFBundlePackageType='FMWK', CFBundleVersion='1',
                        CFBundleShortVersionString=version.split('-')[0], CFBundleSupportedPlatforms=[platform],
                        MinimumOSVersion=minimum, FluxaCoreVersion=version, FluxaSourceCommit=source,
                        FluxaSourceDirty=dirty, FluxaRustTarget=target)
            if variant:
                info['DTPlatformVariant'] = variant
            with (framework / 'Info.plist').open('wb') as stream:
                plistlib.dump(info, stream)
            args += ['-framework', str(framework)]
        args += ['-output', str(temporary / 'FluxaEngine.xcframework')]
        subprocess.run(args, cwd=ROOT, check=True)
        shutil.move(str(temporary / 'FluxaEngine.xcframework'), output)
    print('本地框架：' + str(output), flush=True)


def package_slices(framework, directory):
    """每个文件只含包名指定的平台/架构/ABI；先全部打包后原子移动交付目录。"""
    import hashlib
    import zipfile
    with (framework / 'Info.plist').open('rb') as stream:
        info = plistlib.load(stream)
    if directory.exists() or directory.is_symlink():
        raise ValueError('禁止覆盖既有本地交付目录')
    directory.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='fluxa-core-packages-', dir=directory.parent) as temp:
        staging = Path(temp)
        records = []
        for library in info['AvailableLibraries']:
            if library['SupportedArchitectures'] != ['arm64']:
                raise ValueError('Apple 产物包含非约定架构')
            os_name = library['SupportedPlatform']
            abi = library.get('SupportedPlatformVariant')
            if os_name not in ('macos', 'ios', 'tvos') or abi not in (None, 'simulator'):
                raise ValueError('产物含未支持的平台或 ABI')
            name = 'fluxa-core-' + os_name + '-arm64' + ('-' + abi if abi else '')
            identifier = library['LibraryIdentifier']
            container = staging / name / 'FluxaEngine.xcframework'
            container.mkdir(parents=True)
            shutil.copytree(framework / identifier, container / identifier)
            selected_info = dict(info, AvailableLibraries=[library])
            with (container / 'Info.plist').open('wb') as stream:
                plistlib.dump(selected_info, stream)
            filename = name + '.zip'
            with zipfile.ZipFile(staging / filename, 'x', zipfile.ZIP_DEFLATED) as archive:
                for path in sorted(container.rglob('*')):
                    if path.is_file():
                        archive.write(path, path.relative_to(container.parent))
            with (container / identifier / 'fluxa_core.framework/Info.plist').open('rb') as stream:
                metadata = plistlib.load(stream)
            records.append(dict(file=filename, os=os_name, arch='arm64', abi=abi,
                                minimum=metadata['MinimumOSVersion'], version=metadata['FluxaCoreVersion'],
                                source_commit=metadata['FluxaSourceCommit'], dirty=metadata['FluxaSourceDirty'],
                                sha256=hashlib.sha256((staging / filename).read_bytes()).hexdigest()))
            shutil.rmtree(container.parent)
        (staging / 'manifest.json').write_text(json.dumps(records,ensure_ascii=False,indent=2)+'\n')
        (staging / 'SHA256SUMS').write_text(''.join(row['sha256']+'  '+row['file']+'\n' for row in records))
        shutil.move(str(staging), directory)


def build_platforms(requested=None):
    """支持矩阵与当前构建范围分开：日常默认只编译 macOS。"""
    names = requested or ('macos',)
    return [row for row in PLATFORMS if row[0] in names]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--platform', action='append', choices=[row[0] for row in PLATFORMS])
    parser.add_argument('--package-dir', type=Path, help='可选：按强制命名逐切片生成本地 ZIP 与清单')
    parser.add_argument('--list', action='store_true', help='输出支持矩阵；不构建、不访问网络')
    parser.add_argument('--output', type=Path, default=ROOT / 'Vendor/FluxaEngine.xcframework')
    args = parser.parse_args()
    selected = build_platforms(args.platform)
    if args.list:
        supported = [row for row in PLATFORMS if not args.platform or row[0] in args.platform]
        print(json.dumps([dict(name=r[0], target=r[1], sdk=r[2], minimum=r[4], variant=r[6]) for r in supported], ensure_ascii=False, indent=2))
        return
    build(selected, args.output)
    if args.package_dir:
        package_slices(args.output, args.package_dir.resolve())


if __name__ == '__main__':
    main()
