# 测试文件：tests/test_apple_platforms.py
# 测试类型：单元测试、构建契约回归
# 测试对象：Tools/build-apple.py；build-apple.sh；build-macos.sh；src/lib.rs
# 测试内容：test_default_build_is_macos_only；test_supported_matrix；test_excluded_targets_rejected_before_build；test_deployment_environment_isolated；test_existing_output_preserved；test_local_only_entrypoints；test_package_names_and_contents；test_packages_reject_excluded_architecture。
# 测试依据：用户 2026-10-21 最低系统矩阵；排除 Intel Mac、watchOS、visionOS；仅本地成品。
# 前置条件：Python；测试使用临时文件及受控 SDK 查询，不执行网络上传或真实 Cargo 构建。
import importlib.util
import json
import os
import plistlib
import zipfile
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT=Path(__file__).resolve().parents[1]
spec=importlib.util.spec_from_file_location('apple_builder', ROOT/'Tools/build-apple.py')
builder=importlib.util.module_from_spec(spec); spec.loader.exec_module(builder)

class ApplePlatformsTests(unittest.TestCase):
    def test_default_build_is_macos_only(self):
        self.assertEqual([row[0] for row in builder.build_platforms()],['macos'])
        self.assertEqual([row[0] for row in builder.build_platforms(['ios','ios-simulator'])],['ios','ios-simulator'])
        self.assertEqual(len(builder.PLATFORMS),5,'暂不构建不能删除系统支持矩阵')
    def test_supported_matrix(self):
        result=subprocess.run([sys.executable,str(ROOT/'Tools/build-apple.py'),'--list'],capture_output=True,text=True)
        self.assertEqual(result.returncode,0,result.stderr)
        actual={row['target']:(row['sdk'],row['minimum'],row['variant']) for row in json.loads(result.stdout)}
        self.assertEqual(actual,{
            'aarch64-apple-darwin':('macosx','13.0',None),
            'aarch64-apple-ios':('iphoneos','16.0',None),
            'aarch64-apple-ios-sim':('iphonesimulator','16.0','simulator'),
            'aarch64-apple-tvos':('appletvos','17.0',None),
            'aarch64-apple-tvos-sim':('appletvsimulator','17.0','simulator')})
    def test_excluded_targets_rejected_before_build(self):
        for excluded in ('x86_64-apple-darwin','x86_64','watchos','visionos','maccatalyst'):
            result=subprocess.run([sys.executable,str(ROOT/'Tools/build-apple.py'),'--platform',excluded],capture_output=True,text=True)
            self.assertEqual(result.returncode,2,result.stdout)
            self.assertIn('invalid choice',result.stderr)
    def test_deployment_environment_isolated(self):
        polluted={key:'99.0' for key in builder.DEPLOYMENT_KEYS}
        polluted.update(SDKROOT='/wrong-sdk',CFLAGS='-mmacosx-version-min=99.0',RUSTFLAGS='--cfg wrong')
        with patch.dict(os.environ,polluted),patch.object(builder.subprocess,'check_output',return_value='/selected-sdk\n') as query:
            for row in builder.PLATFORMS:
                env=builder.environment(row)
                self.assertEqual(env['SDKROOT'],'/selected-sdk')
                self.assertEqual(env[row[3]],row[4])
                self.assertEqual({key for key in builder.DEPLOYMENT_KEYS if key in env},{row[3]})
                self.assertNotIn('CFLAGS',env);self.assertNotIn('RUSTFLAGS',env)
                query.assert_called_with(['xcrun','--sdk',row[2],'--show-sdk-path'],text=True)
    def test_existing_output_preserved(self):
        with tempfile.TemporaryDirectory() as directory:
            output=Path(directory)/'existing.xcframework';output.mkdir();(output/'sentinel').write_text('保留')
            with patch.object(builder.subprocess,'check_output',return_value='a'*40):
                with self.assertRaisesRegex(ValueError,'禁止覆盖'):
                    builder.build(builder.PLATFORMS[:1],output)
            self.assertEqual((output/'sentinel').read_text(),'保留')
    def test_package_names_and_contents(self):
        with tempfile.TemporaryDirectory() as temporary:
            root=Path(temporary);framework=root/'NodusEngine.xcframework';framework.mkdir()
            library={'LibraryIdentifier':'ios-arm64-simulator','LibraryPath':'nodus_core.framework','SupportedArchitectures':['arm64'],'SupportedPlatform':'ios','SupportedPlatformVariant':'simulator'}
            with (framework/'Info.plist').open('wb') as stream:
                plistlib.dump({'AvailableLibraries':[library],'XCFrameworkFormatVersion':'1.0'},stream)
            child=framework/library['LibraryIdentifier']/'nodus_core.framework';child.mkdir(parents=True)
            (child/'nodus_core').write_bytes(b'static archive fixture')
            with (child/'Info.plist').open('wb') as stream:
                plistlib.dump({'MinimumOSVersion':'16.0','NodusCoreVersion':'2.8.0-dev','NodusSourceCommit':'a'*40,'NodusSourceDirty':True},stream)
            output=root/'packages';builder.package_slices(framework,output)
            self.assertEqual({p.name for p in output.iterdir()},{'nodus-core-ios-arm64-simulator.zip','manifest.json','SHA256SUMS'})
            with zipfile.ZipFile(output/'nodus-core-ios-arm64-simulator.zip') as archive:
                self.assertIn('NodusEngine.xcframework/ios-arm64-simulator/nodus_core.framework/nodus_core',archive.namelist())
                info=plistlib.loads(archive.read('NodusEngine.xcframework/Info.plist'))
                self.assertEqual(info['AvailableLibraries'],[library])
            record=json.loads((output/'manifest.json').read_text())[0]
            self.assertEqual((record['os'],record['arch'],record['abi'],record['minimum']),('ios','arm64','simulator','16.0'))
    def test_packages_reject_excluded_architecture(self):
        with tempfile.TemporaryDirectory() as temporary:
            root=Path(temporary);framework=root/'framework';framework.mkdir()
            with (framework/'Info.plist').open('wb') as stream:
                plistlib.dump({'AvailableLibraries':[{'SupportedArchitectures':['x86_64']}]},stream)
            with self.assertRaisesRegex(ValueError,'非约定架构'):
                builder.package_slices(framework,root/'packages')
            self.assertFalse((root/'packages').exists())
    def test_local_only_entrypoints(self):
        for name in ('build-apple.sh','build-macos.sh','Tools/build-apple.py'):
            text=(ROOT/name).read_text()
            for forbidden in ('gh release','curl ','upload-artifact','publish-core-binaries.py'):
                self.assertNotIn(forbidden,text)

if __name__=='__main__':unittest.main()
