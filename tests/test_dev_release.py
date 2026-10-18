# 测试文件：tests/test_dev_release.py
# 测试类型：集成测试、发布控制流离线回归
# 测试对象：Tools/dev-release.sh；.github/workflows/tests.yml
# 测试内容：test_dirty_source_never_publishes；test_wrong_source_never_publishes；test_existing_release_is_verified_without_overwrite；test_existing_checksum_failure_does_not_overwrite；test_new_release_stays_prerelease_and_not_latest；test_failed_upload_never_publishes_draft。
# 测试依据：dev 每提交测试附件、确定源码、不可覆盖、非正式发布和失败不伪成功约定。
# 前置条件：Git、Bash、Python；所有 gh/cargo/ditto 均为本地桩，不访问真实发布服务。
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

ROOT=Path(__file__).resolve().parents[1]
MOCK=r'''#!/usr/bin/env python3
import os,sys,json,hashlib,shutil
from pathlib import Path
args=sys.argv[1:]; home=Path(os.environ['MOCK_ROOT']); mode=Path(sys.argv[0]).name
with (home/'calls').open('a') as f: f.write(json.dumps([mode,*args])+'\n')
if mode=='uname': print('Darwin' if args==['-s'] else 'arm64')
elif mode=='ditto': Path(args[-1]).write_bytes(b'fixed-test-archive')
elif mode=='cargo':
 p=Path(os.environ['CARGO_TARGET_DIR'])/'release/examples/https_fixture';p.parent.mkdir(parents=True,exist_ok=True);p.write_text('fixture')
elif mode=='gh':
 if args[:2]==['release','view']:
  if os.environ.get('MOCK_EXISTING')=='none':sys.exit(1)
  print(json.dumps({'isDraft':os.environ.get('MOCK_EXISTING')=='draft','isPrerelease':True}))
 if args[:1]==['api']:
  if '--method' in args and args[args.index('--method')+1]=='POST':
   (home/'tagged').touch()
  elif not (home/'tagged').exists() and os.environ.get('MOCK_EXISTING')=='none':sys.exit(1)
  print(json.dumps({'object':{'type':'commit','sha':os.environ['FLUXA_DEV_SOURCE']}}))
 elif args[:2]==['release','create']:
  if '--verify-tag' not in args or not (home/'tagged').exists() and os.environ.get('MOCK_EXISTING')=='none':sys.exit(1)
  if os.environ.get('MOCK_FAIL_UPLOAD')=='1':sys.exit(1)
  (home/'created').touch()
  for arg in args:
   if arg.endswith('/FluxaEngine.xcframework.zip'):shutil.copyfile(arg,home/'archive')
   if arg.endswith('/fluxa-test-fixture-macos-arm64.zip'):shutil.copyfile(arg,home/'fixture_archive')
 elif args[:2]==['release','download']:
  out=Path(args[args.index('--dir')+1]);out.mkdir(exist_ok=True)
  archive=(home/'archive').read_bytes() if (home/'archive').exists() else b'fixed-test-archive'
  (out/'FluxaEngine.xcframework.zip').write_bytes(archive)
  fixture=(home/'fixture_archive').read_bytes() if (home/'fixture_archive').exists() else b'fixed-test-fixture'
  (out/'fluxa-test-fixture-macos-arm64.zip').write_bytes(fixture)
  if 'provenance.json' in args:
   p={'source_commit':os.environ['FLUXA_DEV_SOURCE'],'tag':args[2],'version':'2.5.0-dev','unstable':True,'fixture_sha256':hashlib.sha256(fixture).hexdigest(),'framework_sha256':'0'*64 if os.environ.get('MOCK_BAD_HASH')=='1' else hashlib.sha256(archive).hexdigest()}
   (out/'provenance.json').write_text(json.dumps(p))
'''

class DevReleaseTests(unittest.TestCase):
    def setUp(self):
        self.tmp=tempfile.TemporaryDirectory(prefix='fluxa-dev-release-tests-')
        self.addCleanup(self.tmp.cleanup);self.home=Path(self.tmp.name);self.repo=self.home/'repo';self.repo.mkdir()
        (self.repo/'Tools').mkdir();shutil.copyfile(ROOT/'Tools/dev-release.sh',self.repo/'Tools/dev-release.sh')
        (self.repo/'Cargo.toml').write_text('[package]\nname = "test"\nversion = "2.5.0-dev"\n')
        (self.repo/'.gitignore').write_text('Vendor/\n')
        (self.repo/'build-macos.sh').write_text('set -eu\nmkdir -p Vendor/FluxaEngine.xcframework\nprintf fixture > Vendor/FluxaEngine.xcframework/fixture\n')
        for cmd in [['init','-q'],['config','user.name','测试'],['config','user.email','test@example.invalid'],['add','.'],['commit','-qm','测试基线']]:subprocess.run(['git',*cmd],cwd=self.repo,check=True,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
        sha=subprocess.check_output(['git','rev-parse','HEAD'],cwd=self.repo,text=True).strip()
        bin=self.home/'bin';bin.mkdir()
        for name in ['uname','cargo','ditto','gh']:
            p=bin/name;p.write_text(MOCK);p.chmod(0o755)
        self.env={**os.environ,'PATH':str(bin)+os.pathsep+os.environ['PATH'],'MOCK_ROOT':str(self.home),'MOCK_EXISTING':'none','FLUXA_DEV_SOURCE':sha,'FLUXA_DEV_REPOSITORY':'teaoea/Fluxa_Core','FLUXA_DEV_RUN':'https://example.invalid/run'}
    def run_release(self):
        return subprocess.run(['bash','Tools/dev-release.sh'],cwd=self.repo,env=self.env,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,text=True)
    def calls(self):
        return [json.loads(x) for x in (self.home/'calls').read_text().splitlines()]
    def test_dirty_source_never_publishes(self):
        (self.repo/'untracked').write_text('dirty');self.assertNotEqual(self.run_release().returncode,0);self.assertFalse(any(x[0]=='gh' for x in self.calls()))
    def test_wrong_source_never_publishes(self):
        self.env['FLUXA_DEV_SOURCE']='0'*40;self.assertNotEqual(self.run_release().returncode,0);self.assertFalse(any(x[0]=='gh' for x in self.calls()))
    def test_existing_release_is_verified_without_overwrite(self):
        self.env['MOCK_EXISTING']='published';r=self.run_release();self.assertEqual(r.returncode,0,r.stdout);self.assertFalse(any(x[1:3] in (['release','create'],['release','edit'],['release','delete']) for x in self.calls()))
    def test_existing_checksum_failure_does_not_overwrite(self):
        self.env.update(MOCK_EXISTING='published',MOCK_BAD_HASH='1');r=self.run_release();self.assertNotEqual(r.returncode,0);self.assertFalse(any(x[1:3]==['release','create'] for x in self.calls()))
    def test_new_release_stays_prerelease_and_not_latest(self):
        r=self.run_release();self.assertEqual(r.returncode,0,r.stdout);calls=self.calls();create=next(x for x in calls if x[1:3]==['release','create']);edit=next(x for x in calls if x[1:3]==['release','edit']);self.assertIn('--verify-tag',create);tagged=next(x for x in calls if x[1:2]==['api'] and 'POST' in x);self.assertLess(calls.index(tagged),calls.index(create));self.assertIn('--draft',create);self.assertIn('--prerelease',create);self.assertIn('--latest=false',create);self.assertIn('--prerelease',edit);self.assertIn('--latest=false',edit);self.assertIn('--draft=false',edit);self.assertLess(calls.index(create),calls.index(edit));self.assertFalse(any('publish' in x for x in calls))
    def test_failed_upload_never_publishes_draft(self):
        self.env['MOCK_FAIL_UPLOAD']='1';r=self.run_release();self.assertNotEqual(r.returncode,0);self.assertFalse(any(x[1:3]==['release','edit'] for x in self.calls()))

if __name__=='__main__':unittest.main()
