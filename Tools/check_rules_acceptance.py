#!/usr/bin/env python3
"""独立进程重复测量；输出原始证据，不用预算计数代替 RSS，也不声称全平台保证。"""
import argparse
import json
import re
import statistics
import hashlib
import platform
import shutil
import subprocess
from pathlib import Path

COUNTS = (1000, 10000, 50000, 100000)
# 本轮 macOS 主机的保守验收目标，不是生产 SLA；在观察候选数据前确定。
MAX_UPDATE_MS = {1000: 300, 10000: 600, 50000: 1800, 100000: 3000}
MAX_RSS_BYTES = 512 * 1024 * 1024


def measure(binary, arguments, output):
    run = subprocess.run(["/usr/bin/time", "-l", str(binary), *arguments], capture_output=True, text=True)
    output.write_text(run.stdout + run.stderr, encoding="utf-8")
    if run.returncode:
        raise RuntimeError(f"程序失败，查看 {output}")
    match = re.search(r"(\d+)\s+maximum resident set size", run.stderr)
    if match is None:
        raise RuntimeError("无法获取 macOS RSS，禁止以缺失值通过验收")
    return run.stdout, int(match[1])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--bench", type=Path, required=True)
    parser.add_argument("--baseline", type=Path, required=True)
    parser.add_argument("--stress", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    rows = []
    for count in COUNTS:
        measured = {"baseline": [], "candidate": []}
        queries = {"baseline": [], "candidate": []}
        for repeat in range(3):
            # 三轮交替、独立进程测量，不能拿旧机器/旧语义历史表当本轮基线。
            for label, binary in [("baseline", args.baseline), ("candidate", args.bench)]:
                text, peak = measure(binary, [str(count), "--persist", "--keep-cache"], args.output / f"{label}-update-{count}-{repeat}.log")
                measured[label].append({"compile_ms": int(re.search(r"compile_ms=(\d+)", text)[1]), "updates_ms": int(re.search(r"two_atomic_updates_ms=(\d+)", text)[1]), "rss_bytes": peak})
                cache = Path(re.search(r"cache_directory=(.+)", text)[1])
                assert cache.is_absolute() and cache.name.startswith("fluxa-runtime-bench-")
                # 全新进程只恢复该兼容缓存；不额外驻留第二份普通编译树影响准入和 RSS。
                try:
                    query, query_peak = measure(binary, ["--load", str(cache)], args.output / f"{label}-query-{count}-{repeat}.log")
                finally:
                    shutil.rmtree(cache)
                cases = {case: int(ns) for case, ns in re.findall(r"case=(\w+).*?p99_ns=(\d+)", query)}
                assert set(cases) == {"first", "last", "miss"}
                queries[label].append({"p99_ns": cases, "query_process_peak_bytes": query_peak, "throughput": int(re.search(r"throughput_per_second=(\d+)", query)[1])})
        samples = measured["candidate"]
        assert max(s["updates_ms"] for s in samples) <= MAX_UPDATE_MS[count], samples
        assert max(s["rss_bytes"] for s in samples) < MAX_RSS_BYTES, samples
        medians = {label: {key: statistics.median(s[key] for s in values) for key in values[0]} for label, values in measured.items()}
        # 数量增长及功能扩充允许有限常数开销，不能出现异常超线性增长。
        assert medians["candidate"]["updates_ms"] <= 2 * medians["baseline"]["updates_ms"]
        background = []
        for repeat in range(3):
            text, peak = measure(args.bench, [str(count), "--persist", "--background"], args.output / f"background-{count}-{repeat}.log")
            elapsed = int(re.search(r"two_atomic_updates_ms=(\d+)", text)[1])
            submits = [int(value) for value in re.findall(r"background_submit_us=(\d+)", text)]
            assert len(submits) == 2 and elapsed <= MAX_UPDATE_MS[count] and peak < MAX_RSS_BYTES
            background.append({"updates_ms": elapsed, "submit_us": submits, "rss_bytes": peak})
        rows.append({"count": count, "samples": measured, "queries": queries, "median": medians, "background": background})
    stress = []
    for count, shape in [(n, "mixed") for n in COUNTS] + [(1000, "suffix"), (10000, "suffix"), (100000, "suffix"), (1000, "keyword"), (10000, "keyword"), (100000, "duplicate")]:
        text, peak = measure(args.stress, [str(count), shape], args.output / f"stress-{shape}-{count}.log")
        assert "phase=complete" in text
        assert peak < MAX_RSS_BYTES, (count, shape, peak)
        stress.append({"count": count, "shape": shape, "rss_bytes": peak, "result": text.splitlines()[-1]})
    fingerprints = {name: hashlib.sha256(path.read_bytes()).hexdigest() for name, path in [("baseline", args.baseline), ("candidate", args.bench), ("stress", args.stress)]}
    summary = {"environment": {"system": platform.platform(), "rust": subprocess.check_output(["rustc", "--version"], text=True).strip()}, "binary_sha256": fingerprints, "method": "macOS time -l peak bytes; three isolated processes per tier; four concurrent recovery/generation workers", "updates": rows, "stress": stress}
    (args.output / "results.json").write_text(json.dumps(summary, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps(summary, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
