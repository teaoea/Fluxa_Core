#!/usr/bin/env python3
"""核验本机发布证据与确定源码提交绑定，不把未运行项目声明为通过。"""
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import test_reports


def verify(path):
    raw = Path(path).read_bytes()
    if len(raw) > 16384:
        raise ValueError('本机验证摘要超出上限')
    record = json.loads(raw)
    source = subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip()
    if record.get('source_commit') != source or record.get('fingerprint') != test_reports.fingerprint():
        raise ValueError('本机验证摘要与当前提交或源码指纹不一致')
    required = {'format', 'clippy', 'rust_tests', 'error_contract', 'swift_ffi'}
    checks = record.get('checks', {})
    if set(checks) != required or any(value != {'status': 'PASS', 'exit_code': 0} for value in checks.values()):
        raise ValueError('本机强制检查不完整或未通过')
    if not isinstance(record.get('unverified'), list):
        raise ValueError('必须明确记录未验证范围')
    return 'manual-local:' + hashlib.sha256(raw).hexdigest()


if __name__ == '__main__':
    if len(sys.argv) != 2:
        raise SystemExit('必须指定本机验证摘要')
    print(verify(sys.argv[1]))
