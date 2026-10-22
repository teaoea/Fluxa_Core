# 测试文件：tests/test_dev_release.py
# 测试类型：发布控制流离线回归
# 测试对象：Tools/dev-release.sh；Tools/publish-core-binaries.py；Tools/verify-manual-release.py；build-macos.sh；.github/workflows/tests.yml
# 测试内容：test_dirty_source_never_publishes；test_wrong_source_never_publishes；test_missing_verification_never_builds；test_wrong_provenance_never_publishes；test_unknown_manifest_fields_never_become_public；test_implementation_sources_never_become_public；test_checksum_failure_never_publishes；test_symlink_input_never_publishes；test_new_release_stays_prerelease_and_not_latest；test_existing_release_is_verified_without_overwrite；test_failed_upload_never_publishes_draft；test_download_mismatch_never_publishes_draft；test_conflicting_tag_is_not_moved；test_api_auth_failure_is_not_missing_release；test_policy_blocks_crates_and_source_release；test_framework_preserves_current_license_files；test_actions_reference_is_rejected；test_manual_verification_rejects_stale_and_incomplete_evidence；test_prepare_does_not_require_upload_credentials。
# 测试依据：永久禁止 crates.io、源码仓库禁止发布二进制、公开仓库禁止源码与不可覆盖约定。
# 前置条件：Git、Bash、Python；网络发布使用明确的离线夹具，不访问真实发布服务。
import base64
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest
import zipfile
from unittest.mock import patch

MOCK = r'''#!/usr/bin/env python3
import sys
from pathlib import Path
args=sys.argv[1:]
print('Darwin' if args==['-s'] else 'arm64')
'''
ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location('publisher', ROOT/'Tools/publish-core-binaries.py')
publisher = importlib.util.module_from_spec(spec); spec.loader.exec_module(publisher)
SOURCE = 'a' * 40
RUN = 'manual-local:' + 'd' * 64
VERSION = '2.6.1-dev'

class Remote:
    def __init__(self, directory):
        self.files = {name: (directory/name).read_bytes() for name in publisher.FILES}
        self.calls = []; self.existing = False; self.published = False
        self.fail_upload = False; self.bad_download = False; self.conflicting_tag = False
    def release_record(self):
        return {'draft': False, 'prerelease': True, 'assets': [
            {'name': name, 'size': len(value)} for name, value in self.files.items()]}
    def api(self, endpoint, payload=None, missing=False, method=None):
        self.calls.append(('api', endpoint, payload, method))
        if endpoint == 'repos/'+publisher.DESTINATION:
            return {'full_name': publisher.DESTINATION, 'private': False}
        if '/releases/tags/' in endpoint:
            return self.release_record() if self.existing or self.published else None
        if '/contents/' in endpoint:
            if payload is not None:
                self.manifest = base64.b64decode(payload['content'])
                return {'commit': {'sha': 'b'*40}}
            if self.existing:
                return {'content': base64.b64encode(self.files['provenance.json']).decode()}
            return None
        if '/git/ref/tags/' in endpoint:
            return {'object': {'type':'commit','sha':'c'*40}} if self.conflicting_tag else None
        if endpoint.endswith('/git/refs'):
            return {}
        raise AssertionError(endpoint)
    def release(self, *args):
        self.calls.append(('release', *args))
        if args[0] == 'create' and self.fail_upload:
            raise RuntimeError('测试上传故障')
        if args[0] == 'edit': self.published = True
    def download(self, repository, tag, directory):
        self.calls.append(('download', repository, tag))
        for name, value in self.files.items():
            (directory/name).write_bytes(value + (b'bad' if self.bad_download and name.endswith('.zip') else b''))

class DevReleaseTests(unittest.TestCase):
    def setUp(self):
        enabled = patch.object(publisher, "UPLOAD_ENABLED", True)
        enabled.start(); self.addCleanup(enabled.stop)
        self.tmp = tempfile.TemporaryDirectory(prefix='nodus-publish-contract-')
        self.addCleanup(self.tmp.cleanup); self.home = Path(self.tmp.name)
        self.assets = self.home/'assets'; self.assets.mkdir()
        for name, member in [(publisher.FILES[1], 'NodusCore.xcframework/macos-arm64/nodus_core.framework/nodus_core'), (publisher.FILES[2], 'nodus-test-fixture')]:
            with zipfile.ZipFile(self.assets/name, 'w') as archive: archive.writestr(member, b'fixed binary fixture')
        self.record = {'version':VERSION,'tag':'v'+VERSION+'.'+SOURCE,'source_commit':SOURCE,
            'unstable':True,'platforms':['macos-arm64'],'minimum_macos':'13.0',
            'framework_sha256':publisher.digest(self.assets/publisher.FILES[1]),
            'fixture_sha256':publisher.digest(self.assets/publisher.FILES[2]),'verification_run':RUN}
        self.save_record(); self.remote = Remote(self.assets)
        self.repo = self.home/'repo'; (self.repo/'Tools').mkdir(parents=True)
        shutil.copyfile(ROOT/'Tools/dev-release.sh',self.repo/'Tools/dev-release.sh')
        (self.repo/'Cargo.toml').write_text('[package]\nname = "test"\nversion = "2.6.1-dev"\n')
        (self.repo/'.gitignore').write_text('Vendor/\n')
        for args in [['init','-q'],['config','user.name','测试'],['config','user.email','test@example.invalid'],['add','.'],['commit','-qm','测试基线']]:
            subprocess.run(['git',*args],cwd=self.repo,check=True,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
        sha=subprocess.check_output(['git','rev-parse','HEAD'],cwd=self.repo,text=True).strip()
        bin=self.home/'bin'; bin.mkdir()
        (bin/'uname').write_text('#!/bin/sh\nif [ "$1" = -s ]; then echo Darwin; else echo arm64; fi\n'); (bin/'uname').chmod(0o755)
        self.env={**os.environ,'PATH':str(bin)+os.pathsep+os.environ['PATH'],'NODUS_DEV_SOURCE':sha,
            'NODUS_DEV_REPOSITORY':'teaoea/Nodus-Core','NODUS_BINARY_PUBLISH_TOKEN':'offline-fixture','MOCK_ROOT':str(self.home)}
    def save_record(self):
        (self.assets/'provenance.json').write_text(json.dumps(self.record))
    def publish(self):
        return publisher.publish(SOURCE,VERSION,RUN,self.remote,self.assets)
    def run_release(self):
        return subprocess.run(['bash','Tools/dev-release.sh'],cwd=self.repo,env=self.env,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,text=True)
    def test_upload_disabled_before_remote_access(self):
        with patch.object(publisher, "UPLOAD_ENABLED", False):
            with self.assertRaises(ValueError): self.publish()
        self.assertEqual(self.remote.calls, [])
    def test_dirty_source_never_publishes(self):
        (self.repo/'untracked').write_text('dirty'); self.assertNotEqual(self.run_release().returncode,0)
    def test_wrong_source_never_publishes(self):
        self.env['NODUS_DEV_SOURCE']='0'*40; self.assertNotEqual(self.run_release().returncode,0)
    def test_missing_verification_never_builds(self):
        result=subprocess.run(['bash','Tools/dev-release.sh','--prepare',str(self.home/'output')],cwd=self.repo,env=self.env,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,text=True)
        self.assertNotEqual(result.returncode,0); self.assertIn('缺少本机验证记录',result.stdout)
    def test_wrong_provenance_never_publishes(self):
        self.record['source_commit']='f'*40; self.save_record()
        with self.assertRaises(ValueError): self.publish()
        self.assertEqual(self.remote.calls,[])
    def test_unknown_manifest_fields_never_become_public(self):
        self.record['credentials']='must not publish'; self.save_record()
        with self.assertRaises(ValueError): self.publish()
        self.assertEqual(self.remote.calls,[])
    def test_implementation_sources_never_become_public(self):
        with zipfile.ZipFile(self.assets/publisher.FILES[1], 'a') as archive:
            archive.writestr('NodusCore.xcframework/src/lib.rs', 'private implementation')
        self.record['framework_sha256']=publisher.digest(self.assets/publisher.FILES[1]); self.save_record()
        with self.assertRaises(ValueError): self.publish()
        self.assertEqual(self.remote.calls,[])
    def test_checksum_failure_never_publishes(self):
        (self.assets/publisher.FILES[1]).write_bytes(b'changed')
        with self.assertRaises((ValueError,zipfile.BadZipFile)): self.publish()
        self.assertEqual(self.remote.calls,[])
    def test_symlink_input_never_publishes(self):
        path=self.assets/publisher.FILES[1]; path.unlink(); path.symlink_to(self.assets/publisher.FILES[2])
        with self.assertRaises(ValueError): self.publish()
    def test_new_release_stays_prerelease_and_not_latest(self):
        self.publish(); create=next(c for c in self.remote.calls if c[:2]==('release','create'))
        edit=next(c for c in self.remote.calls if c[:2]==('release','edit'))
        self.assertIn('--draft',create); self.assertIn('--prerelease',create); self.assertIn('--latest=false',create)
        self.assertIn('--draft=false',edit); self.assertIn('--latest=false',edit)
        self.assertLess(self.remote.calls.index(create),self.remote.calls.index(edit))
        for call in self.remote.calls:
            if call[0]=='api': self.assertTrue(call[1].startswith('repos/teaoea/Nodus-Core'))
        self.assertEqual(json.loads(self.remote.manifest),self.record)
        self.assertFalse(any(str(x).endswith('.rs') or str(x).endswith('.swift') for x in create))
    def test_existing_release_is_verified_without_overwrite(self):
        self.remote.existing=True; self.publish()
        self.assertFalse(any(c[0]=='release' or c[0]=='api' and c[2] is not None for c in self.remote.calls))
    def test_failed_upload_never_publishes_draft(self):
        self.remote.fail_upload=True
        with self.assertRaises(RuntimeError): self.publish()
        self.assertFalse(self.remote.published)
    def test_download_mismatch_never_publishes_draft(self):
        self.remote.bad_download=True
        with self.assertRaises(ValueError): self.publish()
        self.assertFalse(self.remote.published)
    def test_conflicting_tag_is_not_moved(self):
        self.remote.conflicting_tag=True
        with self.assertRaises(ValueError): self.publish()
        self.assertFalse(any(c[0]=='release' for c in self.remote.calls))
    def test_api_auth_failure_is_not_missing_release(self):
        client=publisher.GitHub('offline-fixture')
        with patch.object(client,'command',return_value=subprocess.CompletedProcess([],1,b'HTTP/2.0 401 Unauthorized\n\n{}',b'')):
            with self.assertRaises(RuntimeError): client.api('repos/test',missing=True)
    def test_policy_blocks_crates_and_source_release(self):
        self.assertIn('publish = false',(ROOT/'Cargo.toml').read_text())
        self.assertFalse((ROOT/'.github/workflows/release.yml').exists())
        self.assertNotIn('gh release ',(ROOT/'Tools/dev-release.sh').read_text())
        workflow=''.join(p.read_text() for p in (ROOT/'.github/workflows').glob('*.yml'))
        self.assertNotIn('dev-release:',workflow)
        self.assertNotIn('secrets.NODUS_BINARY_PUBLISH_TOKEN',workflow)
        if workflow:
            self.assertIn('workflow_dispatch:',workflow)
    def test_actions_reference_is_rejected(self):
        self.record['verification_run']='https://github.com/teaoea/Nodus-Core/actions/runs/123'
        self.save_record()
        with self.assertRaises(ValueError):
            publisher.publish(SOURCE,VERSION,self.record['verification_run'],self.remote,self.assets)
        self.assertEqual(self.remote.calls,[])
    def test_manual_verification_rejects_stale_and_incomplete_evidence(self):
        import sys
        sys.path.insert(0,str(ROOT/'Tools'))
        spec=importlib.util.spec_from_file_location('manual',ROOT/'Tools/verify-manual-release.py')
        manual=importlib.util.module_from_spec(spec);spec.loader.exec_module(manual)
        path=self.home/'verification.json'
        record={'source_commit':SOURCE,'fingerprint':'fixed','checks':{name:{'status':'PASS','exit_code':0} for name in ['format','clippy','rust_tests','error_contract','swift_ffi']},'unverified':['真实系统 VPN']}
        with patch.object(manual.subprocess,'check_output',return_value=SOURCE+'\n'),patch.object(manual.test_reports,'fingerprint',return_value='fixed'):
            path.write_text(json.dumps(record));self.assertTrue(manual.verify(path).startswith('manual-local:'))
            record['fingerprint']='stale';path.write_text(json.dumps(record))
            with self.assertRaises(ValueError):manual.verify(path)
            record['fingerprint']='fixed';record['checks']['swift_ffi']={'status':'NOT RUN','exit_code':0};path.write_text(json.dumps(record))
            with self.assertRaises(ValueError):manual.verify(path)
    def test_prepare_does_not_require_upload_credentials(self):
        script=(ROOT/'Tools/dev-release.sh').read_text()
        self.assertNotIn('${NODUS_BINARY_PUBLISH_TOKEN',script)
        self.assertIn('Tools/verify-manual-release.py',script)
        self.assertIn('--prepare',script)
    def test_framework_preserves_current_license_files(self):
        # 运行真实打包脚本，仅替换耗时的工具；文件复制不打桩，能检出许可文件改名遗漏。
        shutil.copyfile(ROOT/'build-macos.sh',self.repo/'build-macos.sh')
        shutil.copyfile(ROOT/'Tools/build-apple.py',self.repo/'Tools/build-apple.py')
        for name in ['LICENSE','LICENSE.md']:
            shutil.copyfile(ROOT/name,self.repo/name)
        (self.repo/'Notices').mkdir();(self.repo/'Notices/third-party').write_text('第三方声明')
        (self.repo/'include').mkdir()
        for name in ['nodus_core.h','nodus_debug_ipc.h','module.modulemap']:
            shutil.copyfile(ROOT/'include'/name,self.repo/'include'/name)
        scripts={
            'cargo': '#!/bin/sh\nset -eu\nwhile [ $# -gt 0 ]; do if [ "$1" = --target-dir ]; then shift; directory=$1; fi; shift; done\nmkdir -p "$directory/aarch64-apple-darwin/release"\nprintf archive > "$directory/aarch64-apple-darwin/release/libnodus_core.a"\n',
            'xcrun': '#!/bin/sh\nprintf /offline-sdk\n',
            'xcodebuild': '#!/bin/sh\nset -eu\ncp -R "$3" "$5"\n'
        }
        for name,script in scripts.items():
            target=self.home/'bin'/name;target.write_text(script);target.chmod(0o755)
        result=subprocess.run(['bash','build-macos.sh'],cwd=self.repo,env=self.env,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,text=True)
        self.assertEqual(result.returncode,0,result.stdout)
        notices=self.repo/'Vendor/NodusCore.xcframework/Resources/Notices'
        for name in ['LICENSE','LICENSE.md']:
            self.assertEqual((notices/name).read_bytes(),(ROOT/name).read_bytes())
        self.assertEqual((notices/'third-party').read_text(),'第三方声明')

if __name__=='__main__': unittest.main()
