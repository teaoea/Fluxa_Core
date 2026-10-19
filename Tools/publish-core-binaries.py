#!/usr/bin/env python3
"""把门禁通过的 dev 附件分发到固定公开仓库；不提交内核源码或二进制到 Git。"""
import base64
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import zipfile

SOURCE = 'teaoea/Fluxa_Core'
DESTINATION = 'teaoea/libfluxa-core'
FILES = ('provenance.json', 'FluxaEngine.xcframework.zip', 'fluxa-test-fixture-macos-arm64.zip')
LIMITS = dict(zip(FILES, (16384, 80 * 1024 * 1024, 20 * 1024 * 1024)))


class GitHub:
    def __init__(self, token):
        if not token:
            raise ValueError('缺少 GitHub 发布授权；禁止回退至其他仓库或匿名伪成功')
        self.env = {key: value for key, value in os.environ.items()
                    if key not in ('GH_TOKEN', 'GITHUB_TOKEN', 'FLUXA_BINARY_PUBLISH_TOKEN')}
        self.env.update(GH_TOKEN=token, GH_HOST='github.com', GH_PROMPT_DISABLED='1')

    def command(self, *args, payload=None):
        result = subprocess.run(['gh', *args], env=self.env, input=payload,
                                stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=300)
        return result

    def api(self, endpoint, payload=None, missing=False, method=None):
        args = ['api', '--include', endpoint]
        if payload is not None:
            args += ['--method', method or 'POST', '--input', '-']
        result = self.command(*args, payload=None if payload is None else json.dumps(payload).encode())
        output = result.stdout.decode('utf-8')
        status = re.search(r'^HTTP/\S+ (\d+)', output)
        if missing and status and int(status[1]) == 404:
            return None
        if result.returncode or not status or not 200 <= int(status[1]) < 300:
            raise RuntimeError('GitHub API 操作失败；未输出认证数据或底层响应')
        return json.loads(re.split(r'\r?\n\r?\n', output, maxsplit=1)[1])

    def download(self, repository, tag, directory):
        args = ['release', 'download', tag, '--repo', repository, '--dir', str(directory)]
        for name in FILES:
            args += ['--pattern', name]
        if self.command(*args).returncode:
            raise RuntimeError('固定发布附件下载失败')

    def release(self, *args):
        if self.command('release', *args).returncode:
            raise RuntimeError('分发发布操作失败；保留草稿与已有附件，不覆盖或删除')


def digest(path):
    value = hashlib.sha256()
    with path.open('rb') as stream:
        while data := stream.read(1024 * 1024):
            value.update(data)
    return value.hexdigest()


def verify_archive(path, framework):
    forbidden = {'.rs', '.swift', '.py', '.sh', '.c', '.cc', '.cpp', '.m', '.mm'}
    with zipfile.ZipFile(path) as archive:
        total = 0
        for item in archive.infolist():
            parts = Path(item.filename).parts
            if item.filename.startswith('/') or '..' in parts or '.git' in parts:
                raise ValueError('附件包含非法路径')
            if Path(item.filename).suffix.lower() in forbidden or Path(item.filename).name in ('Cargo.toml', 'Cargo.lock', 'Package.swift'):
                raise ValueError('公开附件包含实现源码或构建脚本')
            if framework and not item.filename.startswith('FluxaEngine.xcframework/'):
                raise ValueError('框架附件根目录不匹配')
            if not framework and item.filename != 'fluxa-test-fixture':
                raise ValueError('测试服务附件只能包含编译后的独立程序')
            total += item.file_size
            if total > 200 * 1024 * 1024:
                raise ValueError('附件展开尺寸超过预算')
        if not archive.infolist():
            raise ValueError('附件为空')


def verify_files(directory, expected=None):
    for name, limit in LIMITS.items():
        path = directory / name
        if path.is_symlink() or not path.is_file() or not 0 < path.stat().st_size <= limit:
            raise ValueError('附件缺失或超过上限：' + name)
    verify_archive(directory / FILES[1], True)
    verify_archive(directory / FILES[2], False)
    record = json.loads((directory / 'provenance.json').read_text())
    if expected is not None and record != expected:
        raise ValueError('下载来源证明与确定版本不一致')
    for name, key in [(FILES[1], 'framework_sha256'), (FILES[2], 'fixture_sha256')]:
        if digest(directory / name) != record[key]:
            raise ValueError('附件 SHA-256 不一致：' + name)
    return record


def validate_assets(release):
    if not release or release['draft'] or not release['prerelease']:
        raise ValueError('来源必须是已公开的不稳定测试版本')
    assets = {asset['name']: asset for asset in release['assets']}
    if set(assets) != set(FILES):
        raise ValueError('远程附件不符合唯一公开白名单')
    for name, limit in LIMITS.items():
        if name not in assets or not 0 < assets[name]['size'] <= limit:
            raise ValueError('远程附件缺失或超过上限：' + name)


def publish(source, version, run, writer, directory):
    if not re.fullmatch(r'[0-9a-f]{40}', source) or not re.fullmatch(r'\d+\.\d+\.\d+-dev', version):
        raise ValueError('源码或 dev 版本无效')
    if not re.fullmatch(r'https://github.com/teaoea/Fluxa_Core/actions/runs/\d+', run):
        raise ValueError('验证记录来源无效')
    tag = 'v' + version + '.' + source
    record = verify_files(directory)
    allowed = {'version', 'tag', 'source_commit', 'unstable', 'platforms', 'minimum_macos',
               'framework_sha256', 'fixture_sha256', 'verification_run'}
    if set(record) != allowed or record['unstable'] is not True:
        raise ValueError('来源清单字段或不稳定标记无效；禁止公开任意配置')
    if (record['source_commit'], record['version'], record['tag'], record['verification_run']) != (source, version, tag, run):
        raise ValueError('来源证明不匹配本次门禁提交')
    if record['platforms'] != ['macos-arm64'] or record['minimum_macos'] != '14.0':
        raise ValueError('平台声明不匹配实际构建')
    repo = writer.api(f'repos/{DESTINATION}')
    if repo['full_name'] != DESTINATION or repo['private']:
        raise ValueError('分发目标必须是约定的公开仓库')
    existing = writer.api(f'repos/{DESTINATION}/releases/tags/{tag}', missing=True)
    if existing is not None:
        validate_assets(existing)
        check = directory / 'existing'; check.mkdir()
        writer.download(DESTINATION, tag, check)
        verify_files(check, record)
        manifest = writer.api(f'repos/{DESTINATION}/contents/versions/{tag}/provenance.json?ref={tag}')
        if json.loads(base64.b64decode(manifest['content'])) != record:
            raise ValueError('已发布标签的来源清单不一致')
        return tag
    # 分发仓库标签指向来源清单的提交；绝不把不同仓库的 Git SHA 伪装成本仓库提交。
    content_path = f'versions/{tag}/provenance.json'
    item = writer.api(f'repos/{DESTINATION}/contents/{content_path}', missing=True)
    if item is None:
        # 支持空分发仓库首次初始化；仅写入无凭据的来源清单。
        response = writer.api(f'repos/{DESTINATION}/contents/{content_path}', {
            'message': '登记内核测试附件来源 ' + tag,
            'content': base64.b64encode((directory / FILES[0]).read_bytes()).decode()}, method='PUT')
        anchor = response['commit']['sha']
    else:
        if json.loads(base64.b64decode(item['content'])) != record:
            raise ValueError('既有来源清单冲突；禁止覆盖')
        commits = writer.api(f'repos/{DESTINATION}/commits?path={content_path}&per_page=1')
        anchor = commits[0]['sha']
    target_ref = writer.api(f'repos/{DESTINATION}/git/ref/tags/{tag}', missing=True)
    if target_ref is None:
        writer.api(f'repos/{DESTINATION}/git/refs', {'ref': 'refs/tags/' + tag, 'sha': anchor})
    elif target_ref['object']['type'] != 'commit' or target_ref['object']['sha'] != anchor:
        raise ValueError('分发标签冲突；禁止移动')
    notes = directory / 'notes.md'
    notes.write_text('作者：可莉唯一的狗。\n\n不稳定测试版本，不代表系统 VPN 或最低系统验收完成。\n\n'
                     + '内核源码：' + source + '\n\n门禁：' + run + '\n\n不发布 crates.io，不更新稳定版 latest。\n')
    writer.release('create', tag, *(str(directory / name) for name in FILES), '--repo', DESTINATION,
                   '--verify-tag', '--target', anchor, '--draft', '--prerelease', '--latest=false',
                   '--title', 'libfluxa_core ' + version + '（不稳定测试）', '--notes-file', str(notes))
    check = directory / 'uploaded'; check.mkdir()
    writer.download(DESTINATION, tag, check)
    verify_files(check, record)
    writer.release('edit', tag, '--repo', DESTINATION, '--draft=false', '--prerelease', '--latest=false')
    validate_assets(writer.api(f'repos/{DESTINATION}/releases/tags/{tag}'))
    return tag


def main():
    source = os.environ.get('FLUXA_DEV_SOURCE', '')
    actual = subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip()
    if source != actual or os.environ.get('GITHUB_REF') != 'refs/heads/dev':
        raise ValueError('只允许已检出的 dev 门禁提交分发')
    if subprocess.check_output(['git', 'status', '--porcelain', '--untracked-files=all']):
        raise ValueError('发布时源码不再干净，拒绝上传')
    version = re.search(r'^version = "([^"]+)"', Path('Cargo.toml').read_text(), re.M)[1]
    writer = GitHub(os.environ.get('FLUXA_BINARY_PUBLISH_TOKEN'))
    import sys
    if len(sys.argv) != 2:
        raise ValueError('必须指定本次构建附件目录')
    directory = Path(sys.argv[1]).resolve(strict=True)
    tag = publish(source, version, os.environ.get('FLUXA_DEV_RUN', ''), writer, directory)
    print('固定公开仓库测试附件分发校验通过：' + tag)


if __name__ == '__main__':
    main()
