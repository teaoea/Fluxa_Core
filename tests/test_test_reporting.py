# 测试文件：tests/test_test_reporting.py
# 测试类型：单元测试、执行器失败保全与证据一致性回归
# 测试对象：Tools/test_reports.py 的报告执行契约，不虚构 src 源文件映射
# 测试内容：test_nonzero_is_never_pass、test_missing_and_ignored_are_not_full_pass、test_exact_case_ownership、test_timeout_retains_child_status、test_signaled_external_process_never_returns_success、test_unrun_collection_cannot_overwrite_actual_external_result、test_support_fixture_cannot_be_counted_as_passing_suite、test_named_external_suites_require_actual_cases、test_compile_failure_and_report_failure_keep_nonzero
# 测试依据：《测试文件与报告统一管理原则》第十八、十九、二十六条；失败不得变成功。
# 前置条件：Python 标准库，不依赖外网；临时子进程均由本测试清理。
import argparse
import importlib.util
from pathlib import Path
import sys
import tempfile
import unittest
from unittest import mock

spec = importlib.util.spec_from_file_location('reporting', Path(__file__).resolve().parents[1] / 'Tools/test_reports.py')
reporting = importlib.util.module_from_spec(spec)
spec.loader.exec_module(reporting)


class ReportingContract(unittest.TestCase):
    def test_nonzero_is_never_pass(self):
        self.assertEqual(reporting.overall(7, {'case': 'PASS'}, ['case']), 'FAIL')
        result = reporting.command([sys.executable, '-c', 'raise SystemExit(7)'])
        self.assertEqual(result['exit_code'], 7)
        self.assertIsNone(result['status'])

    def test_missing_and_ignored_are_not_full_pass(self):
        self.assertEqual(reporting.overall(0, {}, ['case']), 'UNKNOWN')
        self.assertEqual(reporting.overall(0, {'case': 'IGNORED/SKIPPED'}, ['case']), 'PARTIAL')
        self.assertEqual(reporting.overall(0, {'case': 'PASS'}, ['case']), 'PASS')

    def test_exact_case_ownership(self):
        results = reporting.parse_cases('test module::case ... ok\ntest other::case ... FAILED\n', ['module::case'])
        self.assertEqual(results, {'module::case': 'PASS'})
        self.assertEqual(reporting.parse_cases('999 passed\n', ['case']), {})

    def test_timeout_retains_child_status(self):
        result = reporting.command([sys.executable, '-c', 'import time; time.sleep(30)'], timeout=0.05)
        self.assertEqual(result['status'], 'TIMEOUT')
        self.assertNotEqual(result['exit_code'], 0)

    def test_signaled_external_process_never_returns_success(self):
        args = argparse.Namespace(external='test_external', command=['ignored'], evidence_dir=None, timeout=1, phase='run')
        row = {'id': 'test_external', 'kind': 'python_unit'}
        with tempfile.TemporaryDirectory() as directory:
            args.evidence_dir = directory
            result = {'exit_code': -9, 'status': None, 'output': '', 'command': ['ignored'], 'duration_seconds': 0.1}
            with mock.patch.multiple(reporting, suites=lambda: [row], environment=lambda: {}, fingerprint=lambda: 'fixed',
                                     command=lambda *a: result, save_report=lambda *a: None):
                self.assertEqual(reporting.external(args), 137)

    def test_unrun_collection_cannot_overwrite_actual_external_result(self):
        import json
        import re
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'tests').mkdir()
            evidence = root / 'evidence'
            evidence.mkdir()
            row = {'id': 'test_external', 'file': 'tests/test_external.py', 'kind': 'python_unit', 'sources': ['controlled contract']}
            record = {'status': 'PASS', 'fingerprint': 'same', 'time': '2026-10-15T00:00:00+08:00',
                      'discovered': ['process'], 'results': {'process': 'PASS'}, 'command': ['controlled'], 'exit_code': 0}
            with mock.patch.object(reporting, 'ROOT', root):
                reporting.save_report(row, record, {'ci_run_id': None}, evidence)
                report = root / 'tests/test_external.md'
                before = report.read_text()
                reporting.save_report(row, {'status': 'NOT RUN', 'fingerprint': 'same', 'time': record['time'],
                                           '_preserve_current': True}, {'ci_run_id': None}, evidence)
                self.assertEqual(report.read_text(), before)
                metadata = json.loads(re.search(r'<!-- nodus-test-report:(.*) -->', before)[1])
                self.assertEqual(metadata['status'], 'PASS')

    def test_support_fixture_cannot_be_counted_as_passing_suite(self):
        args = argparse.Namespace(external='test_fixture', command=['ignored'])
        with mock.patch.object(reporting, 'suites', lambda: [{'id': 'test_fixture', 'kind': 'fixture_python'}]):
            with self.assertRaises(ValueError):
                reporting.external(args)

    def test_named_external_suites_require_actual_cases(self):
        self.assertEqual(reporting.parse_named_external('Executed 0 tests', ['expected'], 'swift_xctest'), {})
        self.assertEqual(reporting.parse_named_external('Ran 0 tests', ['expected'], 'python_unit'), {})
        self.assertEqual(reporting.parse_named_external("Test Case '-[Tests expected]' passed (0.1 seconds).", ['expected'], 'swift_xctest'), {'expected': 'PASS'})
        self.assertEqual(reporting.parse_named_external('expected (__main__.Contract) ... ok', ['expected'], 'python_unit'), {'expected': 'PASS'})
        args = argparse.Namespace(external='test_external', command=['ignored'], evidence_dir=None, timeout=1, phase='run')
        row = {'id': 'test_external', 'kind': 'swift_xctest', 'declared_cases': ['expected']}
        with tempfile.TemporaryDirectory() as directory:
            args.evidence_dir = directory
            result = {'exit_code': 0, 'status': None, 'output': 'Executed 0 tests', 'command': ['ignored'], 'duration_seconds': 0.1}
            with mock.patch.multiple(reporting, suites=lambda: [row], environment=lambda: {}, fingerprint=lambda: 'fixed',
                                     command=lambda *a: result, save_report=lambda *a: None):
                self.assertNotEqual(reporting.external(args), 0)

    def test_compile_failure_and_report_failure_keep_nonzero(self):
        args = argparse.Namespace(suite=None, evidence_dir=None, timeout=1, filter=None)
        row = {'id': 'test_fake', 'kind': 'unit', 'file': 'tests/test_fake.rs'}
        with tempfile.TemporaryDirectory() as directory:
            args.evidence_dir = directory
            build = {'command': ['cargo', 'test'], 'exit_code': 17, 'duration_seconds': 0.1, 'output': 'compiler failure', 'status': None}
            captured = []
            def failing_report(suite, record, env, output):
                captured.append(record)
                raise OSError('injected report failure')
            with mock.patch.multiple(reporting, check_layout=lambda: None, suites=lambda: [row], environment=lambda: {},
                                     fingerprint=lambda: 'fixed', command=lambda *a: build, save_report=failing_report):
                code = reporting.run(args)
            self.assertEqual(code, 17)
            self.assertEqual(captured[0]['status'], 'COMPILE ERROR')
            self.assertEqual(captured[0]['results'], {})
            self.assertEqual(captured[0]['discovered'], [])


if __name__ == '__main__':
    unittest.main()
