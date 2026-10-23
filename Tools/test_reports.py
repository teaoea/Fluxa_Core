#!/usr/bin/env python3
"""按显式 Cargo/模块映射执行测试并保存同名报告；保留失败状态与原始证据。"""
import argparse
import collections
import datetime
import fcntl
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import signal
import subprocess
import sys
import tempfile
import time

ROOT = Path(__file__).resolve().parents[1]
MANIFEST = ROOT / 'Tools/test-suites.json'
FIELDS = ('测试文件', '测试类型', '测试对象', '测试内容', '测试依据', '前置条件')
STATES = {'PASS', 'FAIL', 'IGNORED/SKIPPED', 'NOT RUN', 'COMPILE ERROR', 'TIMEOUT', 'INTERRUPTED', 'PARTIAL', 'STALE', 'UNKNOWN'}


def suites():
    return json.loads(MANIFEST.read_text())['suites']


def fingerprint():
    """报告自身不参与指纹；源码、断言、夹具、构建、AGENTS 规范和执行工具均参与。"""
    paths = set()
    for directory in ('src', 'tests', 'Tools', 'include', '.github', 'AGENTS', 'Documentation', 'examples', 'Sources'):
        for path in (ROOT / directory).rglob('*'):
            if path.is_file() and '__pycache__' not in path.parts:
                if directory == 'tests' and path.suffix == '.md':
                    continue
                paths.add(path)
    for name in ('Cargo.toml', 'Cargo.lock', 'Package.swift', 'example_config.yaml', 'default_config.yaml', 'default_config_en.yaml', 'AGENTS.md'):
        if (ROOT / name).exists():
            paths.add(ROOT / name)
    for path in ROOT.glob('*.sh'):
        paths.add(path)
    digest = hashlib.sha256()
    for path in sorted(paths):
        digest.update(path.relative_to(ROOT).as_posix().encode() + b'\0' + path.read_bytes() + b'\0')
    return digest.hexdigest()


def rust_cases(text):
    return re.findall(r'#\[(?:tokio::)?test(?:\([^\]]*\))?\]\s*(?:#\[[^\]]*\]\s*)*(?:pub\s+)?(?:async\s+)?fn\s+(\w+)', text)


def check_layout(check_reports=False):
    errors = []
    rows = suites()
    # Swift 平台适配只属于 Fluxa；同时检查未跟踪的新文件，防止重新混入内核。
    candidates = subprocess.check_output(['git', 'ls-files', '--cached', '--others', '--exclude-standard', '*.swift'], cwd=ROOT, text=True).splitlines()
    if any((ROOT / name).is_file() for name in candidates):
        errors.append('内核禁止 Swift 源码；请迁至 Fluxa 适配层')
    ids = [row['id'] for row in rows]
    files = [row['file'] for row in rows]
    if len(ids) != len(set(ids)) or len(files) != len(set(files)):
        errors.append('套件标识或文件路径重复')
    for directory in ROOT.iterdir():
        if directory.is_dir() and directory.name in ('Tests', 'test', 'Reports'):
            errors.append('非规范测试目录：' + directory.name)
    cargo = (ROOT / 'Cargo.toml').read_text()
    package = cargo.split('[package]', 1)[1].split('\n[', 1)[0]
    if not re.search(r'^autotests\s*=\s*false\s*$', package, re.M):
        errors.append('必须显式关闭 Cargo 自动发现并登记集成目标')
    targets = {}
    for block in cargo.split('[[test]]')[1:]:
        block = block.split('\n[', 1)[0]
        name = re.search(r'^name = "([^"\n]+)"', block, re.M)
        path = re.search(r'^path = "([^"\n]+)"', block, re.M)
        if not name or not path or name[1] in targets:
            errors.append('Cargo 测试目标登记不完整或重复')
        else:
            targets[name[1]] = path[1]
    expected = {x['target']: x['file'] for x in rows if x['kind'] == 'integration'}
    if targets != expected:
        errors.append('Cargo 显式目标与套件映射不一致')
    for path in (ROOT / 'src').rglob('*.rs'):
        if rust_cases(path.read_text()):
            errors.append('src 中仍包含测试实现：' + str(path.relative_to(ROOT)))
    for path in (ROOT / 'tests').rglob('*'):
        if path.suffix in ('.rs', '.swift', '.py') and path.is_file():
            if path.parent != ROOT / 'tests' or not path.name.startswith('test_') or path.name != path.name.lower():
                errors.append('测试源文件位置或命名非法：' + str(path.relative_to(ROOT)))
            if path.relative_to(ROOT).as_posix() not in files:
                errors.append('未登记的测试/夹具：' + str(path.relative_to(ROOT)))
    if check_reports:
        expected_reports = {str(Path(x).with_suffix('.md')) for x in files}
        for report in (ROOT / 'tests').glob('*.md'):
            if report.relative_to(ROOT).as_posix() not in expected_reports:
                errors.append('孤立报告无对应测试/夹具：' + str(report.relative_to(ROOT)))
    source_fingerprint = fingerprint() if check_reports else None
    for row in rows:
        path = ROOT / row['file']
        if not path.is_file():
            errors.append('套件文件不存在：' + row['file'])
            continue
        head = path.read_text().splitlines()[:max(18, len(row.get('declared_cases', [])) + 12)]
        comment = '\n'.join(head)
        if any(field + '：' not in comment for field in FIELDS):
            errors.append('文件头必填字段不全：' + row['file'])
        if path.suffix == '.rs':
            actual = collections.Counter(rust_cases(path.read_text()))
            declared = collections.Counter(row.get('declared_cases', []))
            if actual != declared:
                errors.append('真实函数与登记不一致：' + row['file'])
            if any(name not in comment for name in actual):
                errors.append('文件头遗漏真实函数：' + row['file'])
        if row['kind'] == 'unit':
            source = ROOT / row['sources'][0]
            canonical = 'test_' + row['sources'][0].removeprefix('src/').removesuffix('.rs').replace('/', '_')
            if row['id'] != canonical or row['module'] != canonical:
                errors.append('源码映射命名不一致：' + row['id'])
            if not source.exists() or ('mod ' + canonical + ';') not in source.read_text():
                errors.append('单元模块未注册：' + row['id'])
        if check_reports:
            report = path.with_suffix('.md')
            if not report.exists():
                errors.append('缺少同名报告：' + row['id'])
                continue
            match = re.search(r'<!-- (?:nodus|fluxa)-test-report:(.*) -->', report.read_text())
            try:
                record = json.loads(match[1]) if match else {}
            except json.JSONDecodeError:
                record = {}
            if record.get('fingerprint') != source_fingerprint:
                errors.append('STALE：报告不对应当前源码/断言/配置：' + row['id'])
            if record.get('status') not in STATES:
                errors.append('报告状态无效：' + row['id'])
    if errors:
        raise ValueError('\n'.join(errors))
    return len(rows)


def command(args, timeout=600):
    """返回真实进程退出状态；超时/中断先终止进程，再记录 wrapper 状态。"""
    started = time.monotonic()
    process = subprocess.Popen(args, cwd=ROOT, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True,
                               start_new_session=os.name == "posix")
    state = None
    try:
        output, _ = process.communicate(timeout=timeout)
    except subprocess.TimeoutExpired:
        state = 'TIMEOUT'
        if os.name == 'posix':
            os.killpg(process.pid, signal.SIGKILL)
        else:
            process.kill()
        output, _ = process.communicate()
    except KeyboardInterrupt:
        state = 'INTERRUPTED'
        if os.name == 'posix':
            os.killpg(process.pid, signal.SIGTERM)
        else:
            process.terminate()
        try:
            output, _ = process.communicate(timeout=2)
        except subprocess.TimeoutExpired:
            if os.name == 'posix':
                os.killpg(process.pid, signal.SIGKILL)
            else:
                process.kill()
            output, _ = process.communicate()
    return {'command': args, 'exit_code': process.returncode, 'duration_seconds': time.monotonic() - started,
            'output': output, 'status': state}


def parse_cases(output, discovered):
    """归属来自 Cargo 工件、--list 和显式映射；只匹配已发现的完整用例名。"""
    results = {}
    for name in discovered:
        match = re.search(r'^test ' + re.escape(name) + r' \.\.\. (ok|FAILED|ignored[^\n]*)$', output, re.M)
        if match:
            results[name] = 'PASS' if match[1] == 'ok' else 'FAIL' if match[1] == 'FAILED' else 'IGNORED/SKIPPED'
    return results


def parse_named_external(output, declared, mechanism):
    results = {}
    for name in declared:
        if mechanism == 'swift_xctest':
            pattern = r'^Test Case .*\b' + re.escape(name) + r'\b.* (passed|failed|skipped)\b'
        else:
            pattern = r'^' + re.escape(name) + r'\b.* \.\.\. (ok|FAIL|ERROR|skipped[^\n]*)$'
        match = re.search(pattern, output, re.M)
        if match:
            state = match[1]
            results[name] = 'PASS' if state in ('passed', 'ok') else 'IGNORED/SKIPPED' if state.startswith('skipped') else 'FAIL'
    return results


def overall(exit_code, results, discovered, special=None):
    if special:
        return special
    if exit_code != 0:
        return 'FAIL'
    if len(results) != len(discovered) or not discovered:
        return 'UNKNOWN'
    if any(x != 'PASS' for x in results.values()):
        return 'PARTIAL' if all(x != 'FAIL' for x in results.values()) else 'FAIL'
    return 'PASS'


def environment():
    def output(args):
        return subprocess.check_output(args, cwd=ROOT, text=True).strip()
    return {'os': platform.platform(), 'architecture': platform.machine(), 'rust': output(['rustc', '--version']),
            'cargo': output(['cargo', '--version']), 'commit': output(['git', 'rev-parse', 'HEAD']),
            'dirty': bool(output(['git', 'status', '--porcelain'])), 'features': 'Cargo 默认 features；未启用 --all-features',
            'optional_environments': {key: bool(os.environ.get(key)) for key in ('NODUS_DEBUG_TEST_REQUIREMENT', 'NODUS_DEBUG_TEST_SWIFT', 'NODUS_PROTOCOL_PROFILE', 'NODUS_PROTOCOL_FIXTURE')}, 'ci_run_id': os.environ.get('GITHUB_RUN_ID'), 'ci_run_attempt': os.environ.get('GITHUB_RUN_ATTEMPT')}


def save_report(row, record, env, directory):
    # Rust 收尾与 Swift 外部执行可能并发；同一报告锁下检查/写入，不能把实际执行覆盖成 NOT RUN。
    key = hashlib.sha256((str(ROOT) + row['id']).encode()).hexdigest()
    lock = Path(tempfile.gettempdir()) / ('nodus-test-report-' + key + '.lock')
    with lock.open('a') as stream:
        fcntl.flock(stream, fcntl.LOCK_EX)
        report = ROOT / Path(row['file']).with_suffix('.md')
        if record.get('_preserve_current') and report.exists():
            matched = re.search(r'<!-- (?:nodus|fluxa)-test-report:(.*) -->', report.read_text())
            try:
                previous = json.loads(matched[1]) if matched else {}
            except json.JSONDecodeError:
                previous = {}
            same_ci = not env.get('ci_run_id') or (previous.get('environment', {}).get('ci_run_id') == env['ci_run_id'] and previous.get('environment', {}).get('ci_run_attempt') == env.get('ci_run_attempt'))
            if previous.get('fingerprint') == record['fingerprint'] and same_ci:
                return
        _save_report_locked(row, record, env, directory)


def _save_report_locked(row, record, env, directory):
    names = record.get('discovered', [])
    results = record.get('results', {})
    counts = collections.Counter(results.values())
    report = ROOT / Path(row['file']).with_suffix('.md')
    # 同名文档也承载手工目的、验收标准及历史性能资料；仅刷新执行报告。
    manual = ''
    if report.exists():
        previous_text = report.read_text()
        begin = '<!-- nodus-test-documentation:begin -->'
        end = '<!-- nodus-test-documentation:end -->'
        if begin in previous_text or end in previous_text:
            if previous_text.count(begin) != 1 or previous_text.count(end) != 1:
                raise ValueError('测试文档保留区标记数量错误，拒绝覆盖')
            start = previous_text.index(begin)
            finish = previous_text.index(end) + len(end)
            if previous_text.index(end) < start:
                raise ValueError('测试文档保留区标记顺序错误，拒绝覆盖')
            manual = previous_text[start:finish]
    # 更新前保存真实旧报告，不删除已知失败和历史证据。
    old = directory / (row['id'] + '.previous.md')
    if report.exists() and not old.exists():
        old.write_bytes(report.read_bytes())
    attempt = len(list(directory.glob(row['id'] + '.attempt-*.json'))) + 1
    meta = {key: value for key, value in record.items() if key != 'output' and not key.startswith('_')}
    meta['attempt'] = attempt
    meta.update({'suite': row['id'], 'file': row['file'], 'environment': env})
    (directory / (row['id'] + '.json')).write_text(json.dumps(meta, ensure_ascii=False, indent=2) + '\n')
    (directory / (row['id'] + '.attempt-' + str(attempt) + '.json')).write_text(json.dumps(meta, ensure_ascii=False, indent=2) + '\n')
    if 'output' in record:
        (directory / (row['id'] + '.log')).write_text(record['output'])
        (directory / (row['id'] + '.attempt-' + str(attempt) + '.log')).write_text(record['output'])
    if report.exists():
        (directory / (row['id'] + '.before-' + str(attempt) + '.md')).write_bytes(report.read_bytes())
    evidence = str(directory)
    artifact = ('nodus-test-evidence-' + str(env['ci_run_id']) + '-' + str(env['ci_run_attempt'])) if env['ci_run_id'] else '本地证据目录；未宣称已上传 CI'
    failure_note = ('本次观察到断言失败；实际错误及期望/实际输出见下方和原始日志。' if counts['FAIL'] else '本次未观察到断言失败不等于不存在缺陷；平台门控的未发现用例不计通过。')
    lines = ['# 测试报告：' + row['id'], '', '<!-- nodus-test-report:' + json.dumps(meta, ensure_ascii=False) + ' -->', '',
             '## 基本信息', '', '- 测试文件：`' + row['file'] + '`', '- 测试对象：' + '、'.join(row.get('sources') or ['共享测试夹具；不是独立覆盖承诺']),
             '- 执行机制：' + row['kind'], '- 测试范围：' + record.get('scope', '本文件登记且在当前平台实际发现的全部用例'),
             '- 实际命令：`' + ' '.join(record.get('command', [])) + '`', '- 执行时间：' + record['time'],
             '- 实际耗时：' + str(record.get('duration_seconds', '未执行')) + ' 秒',
             '- 运行环境：`' + json.dumps(env, ensure_ascii=False) + '`', '- 代码/测试/配置指纹：`' + record['fingerprint'] + '`',
             '- 总体状态：**' + record['status'] + '**', '- 原始进程退出状态：' + str(record.get('exit_code', '未执行')),
             '', '## 实际用例与结果', '', '| 实际发现的用例 | 独立目标 | 状态 |', '| --- | --- | --- |']
    for name in names:
        lines.append('| `' + name + '` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | ' + results.get(name, 'NOT RUN') + ' |')
    lines += ['', '## 执行汇总', '', '- 实际发现：' + str(len(names)),
              '- 实际执行完成：' + str(counts['PASS'] + counts['FAIL']), '- 通过：' + str(counts['PASS']),
              '- 失败：' + str(counts['FAIL']), '- 忽略/跳过：' + str(counts['IGNORED/SKIPPED']),
              '- 未取得完整执行结果：' + str(len(names) - len(results)), '', '## 失败、异常与限制', '',
              record.get('note', failure_note), row.get('limitations', '未覆盖范围以本文件实际断言和前置条件为界，不宣称全模块或全平台覆盖。'),
              '不自动重试；若手动重跑，旧报告及原始退出结果保存在证据目录。缺少执行结果时不推定成功。', '',
              '## 独立 L1–L4 审查', '', '- L1：' + record.get('l1', '本次测试构建结果见 build 日志；格式/Clippy 等需另行记录，不自动推断通过。'),
              '- L2：已登记用例按 Cargo 工件、发现列表与显式文件映射归属；实际结果见上表。' if names else '- L2：未取得本文件独立用例执行证据，不计覆盖。',
              '- L3：未由报告生成器验证；迁移保留断言不等于原业务预期已全面证明，需独立语义审查。',
              '- L4：原始日志、机器结果、版本及指纹可在下述证据核对；未上传的本地证据不冒充 CI 工件。', '',
              '## 证据及后续事项', '', '- 原始证据：`' + evidence + '/' + row['id'] + '.json`、对应 `.log`（实际存在时）。',
              '- CI 工件标识：`' + artifact + '`', '- 最新报告被后续变更影响时必须重新执行；检查器会拒绝指纹不一致的旧结果。',
              '- 未运行的 Swift/外部协议或夹具独立目标均保持 NOT RUN；本报告不代表系统 VPN、外部服务端及跨平台已经验收。', '']
    if record.get('status') in ('FAIL', 'COMPILE ERROR', 'TIMEOUT', 'INTERRUPTED'):
        lines += ['## 异常原始输出节选', '', '最小重现：`python3 Tools/test_reports.py --run --suite ' + row['id'] + '`（Rust 套件）；外部套件使用上方真实命令。', '', '```text', record.get('output', '未取得输出；不能推断失败原因。')[-12000:], '```', '', '节选不替代完整日志；输入及独立预期可追踪到上表同名函数。', '']
    if manual:
        lines += [manual, '']
    temporary = report.with_suffix('.md.tmp')
    temporary.write_text('\n'.join(lines))
    temporary.replace(report)


def run(args):
    check_layout()
    rows = suites()
    selected = [x for x in rows if x['kind'] in ('unit', 'integration') and (not args.suite or x['id'] in args.suite)]
    if args.suite and set(args.suite) - {x['id'] for x in selected}:
        raise ValueError('指定的 Rust 套件不存在或执行机制不符')
    env = environment()
    digest = fingerprint()
    now = datetime.datetime.now(datetime.timezone.utc).astimezone().isoformat()
    directory = Path(args.evidence_dir or tempfile.mkdtemp(prefix='nodus-test-evidence-')).resolve()
    directory.mkdir(parents=True, exist_ok=True)
    print('测试证据：' + str(directory), flush=True)
    records = {}
    exit_code = 0
    try:
        build = command(['cargo', 'test', '--offline', '--locked', '--no-run', '--message-format=json'], args.timeout)
        (directory / 'build.log').write_text(build['output'])
        if build['exit_code'] != 0 or build['status']:
            exit_code = build['exit_code'] if build['exit_code'] > 0 else 124 if build['status'] == 'TIMEOUT' else 130
            for row in selected:
                records[row['id']] = dict(build, status=build['status'] or 'COMPILE ERROR', discovered=[], results={},
                    note='测试构建失败或中断；测试函数未执行。不得解释为断言失败若干项。')
            raise RuntimeError("测试构建失败，已为目标保留构建状态")
        artifacts = []
        for line in build['output'].splitlines():
            try:
                item = json.loads(line)
            except json.JSONDecodeError:
                continue
            if item.get('reason') == 'compiler-artifact' and item.get('executable') and item.get('profile', {}).get('test'):
                artifacts.append(item)
        binaries = {x['target']['name']: x['executable'] for x in artifacts}
        lib = next((x['executable'] for x in artifacts if set(x['target']['kind']) & {'lib', 'rlib', 'staticlib'}), None)
        if not lib:
            raise ValueError('未找到真实库单元测试工件')
        discovery = {}
        for binary in set(binaries.values()):
            listed = command([binary, '--list', '--format', 'terse'], args.timeout)
            if listed['exit_code'] != 0:
                raise ValueError('实际测试发现失败：' + binary)
            names = [line[:-6] for line in listed['output'].splitlines() if line.endswith(': test')]
            discovery[binary] = names
        ownership = collections.Counter()
        for row in selected:
            binary = lib if row['kind'] == 'unit' else binaries.get(row['target'])
            if not binary:
                raise ValueError('未找到集成测试工件：' + row['id'])
            names = discovery[binary]
            if row['kind'] == 'unit':
                names = [n for n in names if ('::' + row['module'] + '::') in ('::' + n)]
            if not names:
                # 例如平台限定的 Unix/macOS 单元模块，在不支持平台上明确未执行。
                records[row['id']] = dict(status='NOT RUN', discovered=[], results={}, note='当前平台/特性未发现该套件；不计通过。')
                continue
            for name in names:
                ownership[(binary, name)] += 1
            argv = [binary]
            if row['kind'] == 'unit':
                argv += ['::' + row['module'] + '::']
            if args.filter:
                # libtest 多个筛选条件是 OR，不能叠加 module 与用户 filter；改为完整用例精确执行。
                raise ValueError('局部筛选请按 --suite 选择文件；不允许以 OR 筛选误扩大范围')
            argv += ['--test-threads=1']
            result = command(argv, args.timeout)
            results = parse_cases(result['output'], names)
            result.update(discovered=names, results=results, status=overall(result['exit_code'], results, names, result['status']))
            records[row['id']] = result
            print(row['id'] + ': ' + result['status'] + ' (' + str(len(results)) + '/' + str(len(names)) + ')', flush=True)
            if result['status'] == 'INTERRUPTED':
                exit_code = 130
                break
            if result['status'] not in ('PASS', 'PARTIAL'):
                exit_code = result['exit_code'] if result['exit_code'] > 0 else 124 if result['status'] == 'TIMEOUT' else 1
        if not args.suite and exit_code != 130:
            for binary, names in discovery.items():
                for name in names:
                    if ownership[(binary, name)] != 1:
                        raise ValueError('测试发现存在遗漏或重复归属：' + name)
    except KeyboardInterrupt:
        exit_code = 130
    except Exception as error:
        exit_code = exit_code or 2
        (directory / 'runner-error.log').write_text(repr(error))
        print('测试执行器失败：' + str(error), file=sys.stderr)
    finally:
        current = fingerprint()
        if current != digest:
            exit_code = exit_code or 3
        report_errors = []
        for row in rows:
            # 局部运行不覆盖未选择套件的已有报告；第一次迁移可为未运行目标明确建 NOT RUN。
            if args.suite and row['id'] not in records and (ROOT / row['file']).with_suffix('.md').exists():
                continue
            record = records.get(row['id'], dict(status='NOT RUN', discovered=[], results={},
                note='本次 Rust 执行未运行此目标；夹具不作为独立测试用例，外部/Swift 需对应执行器。'))
            if row['id'] not in records:
                record['_preserve_current'] = True
            record.update(fingerprint=digest, time=now)
            if current != digest:
                record['status'] = 'STALE'
                record['note'] = '执行期间源码/断言/工具配置变化；保留实际结果，但不得视为当前版本通过。'
            try:
                save_report(row, record, env, directory)
            except Exception as error:
                report_errors.append(row['id'] + ': ' + repr(error))
        (directory / 'run.json').write_text(json.dumps(dict(environment=env, fingerprint=digest, time=now,
            selected=[x['id'] for x in selected], exit_code=exit_code, report_errors=report_errors), ensure_ascii=False, indent=2))
        if report_errors:
            print('报告生成失败：' + '\n'.join(report_errors), file=sys.stderr)
            # finally 不能把原始测试失败变为成功；报告失败必须使质量门禁失败。
            if not exit_code:
                exit_code = 4
    return exit_code


def external(args):
    row = next((x for x in suites() if x['id'] == args.external), None)
    if not row or row['kind'] not in ('swift_ffi', 'swift_xctest', 'python_unit', 'python_integration'):
        raise ValueError('外部执行器只能运行登记的 Swift/Python 套件')
    argv = args.command[1:] if args.command and args.command[0] == '--' else args.command
    if not argv:
        raise ValueError('外部执行器需要真实命令')
    directory = Path(args.evidence_dir or os.environ.get('NODUS_TEST_EVIDENCE_DIR') or tempfile.mkdtemp(prefix='nodus-test-evidence-')).resolve()
    directory.mkdir(parents=True, exist_ok=True)
    env = environment()
    digest = fingerprint()
    timestamp = datetime.datetime.now(datetime.timezone.utc).astimezone().isoformat()
    if row['kind'] == 'python_unit' and args.phase == 'run' and '-v' not in argv:
        argv = [*argv, '-v']
    if row['kind'] == 'python_unit' and args.phase == 'run' and '-b' not in argv:
        # unittest 缓冲用例自身输出，避免其日志打断框架的逐用例结果行。
        argv = [*argv, '-b']
    result = command(argv, args.timeout)
    name = row['id'] + '::process'
    if result['status']:
        status = result['status']
    elif args.phase == 'compile':
        status = 'NOT RUN' if result['exit_code'] == 0 else 'COMPILE ERROR'
    else:
        status = 'PASS' if result['exit_code'] == 0 else 'FAIL'
    result.update(status=status, discovered=[name] if args.phase == 'run' else [],
        results={name: status} if args.phase == 'run' and status in ('PASS', 'FAIL') else {},
        fingerprint=digest, time=timestamp,
        scope='单个已登记外部测试程序整体；不声称统计其内部断言或 XCTest 函数数量。',
        note='以真实外部进程退出状态验证程序整体；编译成功不代表测试已运行。内部断言数量未知，不与 Rust 函数数量合并。')
    if args.phase == 'run' and row['kind'] in ('swift_xctest', 'python_unit'):
        declared = row.get('declared_cases', [])
        observed = parse_named_external(result['output'], declared, row['kind'])
        result.update(discovered=declared, results=observed,
                      status=overall(result['exit_code'], observed, declared, result.get('status') if result.get('status') in ('TIMEOUT', 'INTERRUPTED') else None),
                      scope='逐项核对实际测试框架输出与本文件登记函数；零用例或缺少实际结果不计通过。')
    if fingerprint() != digest:
        result['status'] = 'STALE'
    save_report(row, result, env, directory)
    print(row['id'] + ': ' + result['status'], flush=True)
    if status == 'TIMEOUT':
        return 124
    if status == 'INTERRUPTED':
        return 130
    if result['exit_code'] < 0:
        return 128 - result['exit_code']
    if result['status'] == 'STALE':
        return result['exit_code'] or 3
    if result['status'] in ('UNKNOWN', 'FAIL') and result['exit_code'] == 0:
        return 1
    return result['exit_code']


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--external')
    parser.add_argument('--phase', choices=('compile', 'run'), default='run')
    parser.add_argument('--check-layout', action='store_true')
    parser.add_argument('--check-reports', action='store_true')
    parser.add_argument('--run', action='store_true')
    parser.add_argument('--suite', action='append')
    parser.add_argument('--filter')
    parser.add_argument('--timeout', type=int, default=600)
    parser.add_argument('--evidence-dir')
    parser.add_argument('command', nargs=argparse.REMAINDER)
    args = parser.parse_args()
    if hasattr(signal, 'SIGTERM'):
        def interrupted(signum, frame):
            raise KeyboardInterrupt
        signal.signal(signal.SIGTERM, interrupted)
    if args.timeout < 1:
        parser.error('timeout 必须为正数')
    try:
        if args.external:
            return external(args)
        if args.run:
            return run(args)
        print(str(check_layout(args.check_reports)) + ' 个测试/夹具映射符合目录、命名与登记检查')
        return 0
    except Exception as error:
        print(str(error), file=sys.stderr)
        return 2


if __name__ == '__main__':
    sys.exit(main())
