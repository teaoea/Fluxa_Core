# 测试报告：test_flat_cache_round_trip

<!-- nodus-test-report:{"command": ["/Users/klee/Documents/Nodus Core/target/debug/deps/test_flat_cache_round_trip-ee63eb722693b6b8", "--test-threads=1"], "exit_code": 0, "duration_seconds": 0.017692167000006975, "status": "PASS", "discovered": ["cache_round_trip_preserves_independent_policy_matrix"], "results": {"cache_round_trip_preserves_independent_policy_matrix": "PASS"}, "fingerprint": "6570db01cac088692214174f3c064ba92f94de504c7ddbad674b6c9a5028d243", "time": "2026-10-24T13:41:42.348500+08:00", "attempt": 1, "suite": "test_flat_cache_round_trip", "file": "tests/test_flat_cache_round_trip.rs", "environment": {"os": "macOS-27.2-arm64-arm-64bit", "architecture": "arm64", "rust": "rustc 1.98.1 (48a229cea 2026-09-01)", "cargo": "cargo 1.98.1 (797e8a9bc 2026-08-05)", "commit": "2f6cd1e624851636840713246da2f51fa05c86fb", "dirty": true, "features": "Cargo 默认 features；未启用 --all-features", "optional_environments": {"NODUS_DEBUG_TEST_REQUIREMENT": false, "NODUS_DEBUG_TEST_SWIFT": false, "NODUS_PROTOCOL_PROFILE": false, "NODUS_PROTOCOL_FIXTURE": false}, "ci_run_id": null, "ci_run_attempt": null}} -->

## 基本信息

- 测试文件：`tests/test_flat_cache_round_trip.rs`
- 测试对象：src/routing/flat.rs、src/routing/cache.rs、src/routing/store.rs
- 执行机制：integration
- 测试范围：本文件登记且在当前平台实际发现的全部用例
- 实际命令：`/Users/klee/Documents/Nodus Core/target/debug/deps/test_flat_cache_round_trip-ee63eb722693b6b8 --test-threads=1`
- 执行时间：2026-10-24T13:41:42.348500+08:00
- 实际耗时：0.017692167000006975 秒
- 运行环境：`{"os": "macOS-27.2-arm64-arm-64bit", "architecture": "arm64", "rust": "rustc 1.98.1 (48a229cea 2026-09-01)", "cargo": "cargo 1.98.1 (797e8a9bc 2026-08-05)", "commit": "2f6cd1e624851636840713246da2f51fa05c86fb", "dirty": true, "features": "Cargo 默认 features；未启用 --all-features", "optional_environments": {"NODUS_DEBUG_TEST_REQUIREMENT": false, "NODUS_DEBUG_TEST_SWIFT": false, "NODUS_PROTOCOL_PROFILE": false, "NODUS_PROTOCOL_FIXTURE": false}, "ci_run_id": null, "ci_run_attempt": null}`
- 代码/测试/配置指纹：`6570db01cac088692214174f3c064ba92f94de504c7ddbad674b6c9a5028d243`
- 总体状态：**PASS**
- 原始进程退出状态：0

## 实际用例与结果

| 实际发现的用例 | 独立目标 | 状态 |
| --- | --- | --- |
| `cache_round_trip_preserves_independent_policy_matrix` | 对应同名函数中保留的契约、输入和独立断言；详见测试源文件 | PASS |

## 执行汇总

- 实际发现：1
- 实际执行完成：1
- 通过：1
- 失败：0
- 忽略/跳过：0
- 未取得完整执行结果：0

## 失败、异常与限制

本次未观察到断言失败不等于不存在缺陷；平台门控的未发现用例不计通过。
未覆盖范围以本文件实际断言和前置条件为界，不宣称全模块或全平台覆盖。
不自动重试；若手动重跑，旧报告及原始退出结果保存在证据目录。缺少执行结果时不推定成功。

## 独立 L1–L4 审查

- L1：本次测试构建结果见 build 日志；格式/Clippy 等需另行记录，不自动推断通过。
- L2：已登记用例按 Cargo 工件、发现列表与显式文件映射归属；实际结果见上表。
- L3：未由报告生成器验证；迁移保留断言不等于原业务预期已全面证明，需独立语义审查。
- L4：原始日志、机器结果、版本及指纹可在下述证据核对；未上传的本地证据不冒充 CI 工件。

## 证据及后续事项

- 原始证据：`/private/tmp/claude-501/-Users-klee-Documents-Fluxa/dd1504c4-f781-4692-b482-4e9c216c95e1/scratchpad/core-evidence/test_flat_cache_round_trip.json`、对应 `.log`（实际存在时）。
- CI 工件标识：`本地证据目录；未宣称已上传 CI`
- 最新报告被后续变更影响时必须重新执行；检查器会拒绝指纹不一致的旧结果。
- 未运行的 Swift/外部协议或夹具独立目标均保持 NOT RUN；本报告不代表系统 VPN、外部服务端及跨平台已经验收。

<!-- nodus-test-documentation:begin -->
## 测试目的与目标

本文件的功能测试验证规则缓存导出／只读恢复前后，双栈地址、精确域名、后缀代理和未命中的策略及出口保持独立预期。判断标准为 `cache_round_trip_preserves_independent_policy_matrix` 的逐项目标及断言；它本身不是吞吐基准，不以该功能测试通过宣称性能达标。

## 历史平铺缓存性能资料（非本次运行）

以下资料由原性能文档迁入。其执行程序为 `examples/runtime_rules_bench.rs`、`rules_acceptance.rs` 及历史辅助程序，覆盖相同缓存／规则跨模块流程，不能冒称由上述单个功能测试测出。当前 Core 只读消费文件；旧版写盘／fsync 测量只属于原版本，不证明当前文件职责或当前性能。

# 规则性能测试记录

作者：可莉唯一的狗。

本文仅整理 2026-10-15 已有的平铺缓存性能测量，没有在文档清理时重新运行基准，也不代表当前 dev 的性能验收。原始测量可通过 Git 提交 `240e6bfe9562226e5770ba026e6cde061713207e` 下的 `Documentation/规则组合与资源验收.md` 追溯。

## 平铺缓存测量方法与目标（2026-10-15）


该轮使用平铺缓存。修改前基线为已提交 e090b367d96fd6979ed383b007f1bae122c326cc 源码的独立优化程序；两方案采用同一混合基准数据与本机条件，每档三轮交替、独立进程。测量时没有并行编译或 FFI 测试。编译列是普通内存索引编译；两次原子更新包含编译/平铺、元数据编码、完整复核、文件与目录同步、快照切换及失败判断。RSS 为 macOS time -l 峰值，单位字节；不是纯索引大小。

目标在最终结果前写入 `Tools/check_rules_acceptance.py`：1000/10000/50000/100000 条两次更新每轮分别不超过 300/600/1800/3000 ms；中位数不超过同条件基线两倍；固定矩阵 RSS 不超过 512 MiB。该目标是本机工程验收标准，不是跨平台 SLA。查询用更新生成的各自兼容缓存，在完全独立的新进程只恢复后测首、尾和未命中各 20000 次及四线程 400000 次；纳秒 p99 受计时分辨率限制，不推出真实网络速度。

压力示例 `examples/rules_acceptance.rs` 包含八来源、精确/组合/高标签后缀、关键词、IPv4/IPv6、重复规则、三次更新保留旧 Arc，以及四线程同时恢复和生成。成功或资源准入拒绝均需检查；非资源错误导致失败。十万重复规则必须成功合并，不能把任意资源拒绝当作通过。压力结束必须池占用归零，释放旧引用后仍可恢复完整缓存，不以逻辑关闭提前归还实际存活内存。

最终数据与实际命令在下节登记；附件包含二进制 SHA-256、主机与编译器信息、所有三轮样本。各项测量仅按本轮环境与输入解释。

## 平铺缓存三轮更新与实际峰值


环境：macOS-27.2-arm64-arm-64bit；rustc 1.98.1 (48a229cea 2026-09-01)；release / thin LTO / 单代码生成单元。下表均为三轮中位数，峰值列为每个独立进程 time -l 的峰值再取中位数。

| 规则数 | 基线普通索引编译 ms | 本轮普通索引编译 ms | 基线两次更新 ms | 本轮两次更新 ms | 基线峰值 MiB | 本轮峰值 MiB | 本轮后台两次更新 ms |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 1,000 | 1 | 2 | 21 | 21 | 14.14 | 19.77 | 22 |
| 10,000 | 7 | 8 | 45 | 50 | 27.89 | 36.97 | 49 |
| 50,000 | 37 | 38 | 175 | 192 | 92.55 | 103.86 | 191 |
| 100,000 | 84 | 84 | 336 | 415 | 174.81 | 234.67 | 377 |

本轮 100000 条的两次更新中位数从 336 ms 变为 415 ms，约增加 23.5%；峰值中位数从 174.81 MiB 变为 234.67 MiB，约增加 34.2%。域名协议组合扩充了固定候选记录和缓存，并非本轮所有指标都改善；未通过放宽预算或改变既定语义追求更好数字。四档均达到预先登记的目标，10 倍规模没有出现异常超线性增长；这不是无限规模复杂度证明。

### 恢复后查询

| 规则数 | 首/尾/未命中 p99 ns 中位数 | 四线程查询万次/s 中位数 |
| --- | --- | --- |
| 1,000 | 125/167/167 | 2212.9 |
| 10,000 | 125/167/167 | 2164.3 |
| 50,000 | 125/167/167 | 2222.8 |
| 100,000 | 125/167/167 | 2164.3 |

这是固定后缀热查询，p99 接近计时分辨率，不代表 QUIC、DNS、TLS、真实代理吞吐或其他内核对比。

### 混合与极端形态并发压力

| 输入数量 | 形态 | 本轮进程峰值 MiB | 结果与边界 |
| --- | --- | --- | --- |
| 1,000 | mixed | 15.84 | 更新/并发/恢复与最终回收断言通过；并发资源不足允许受控拒绝 |
| 10,000 | mixed | 78.34 | 更新/并发/恢复与最终回收断言通过；并发资源不足允许受控拒绝 |
| 50,000 | mixed | 277.95 | 更新/并发/恢复与最终回收断言通过；并发资源不足允许受控拒绝 |
| 100,000 | mixed | 437.91 | 更新/并发/恢复与最终回收断言通过；并发资源不足允许受控拒绝 |
| 1,000 | suffix | 124.44 | 更新/并发/恢复与最终回收断言通过；并发资源不足允许受控拒绝 |
| 10,000 | suffix | 414.41 | 更新/并发/恢复与最终回收断言通过；并发资源不足允许受控拒绝 |
| 100,000 | suffix | 53.42 | 在准入阶段拒绝，未发布新方案 |
| 1,000 | keyword | 252.58 | 更新/并发/恢复与最终回收断言通过；并发资源不足允许受控拒绝 |
| 10,000 | keyword | 123.20 | 在准入阶段拒绝，未发布新方案 |
| 100,000 | duplicate | 123.06 | 十万输入合并后更新与恢复成功；不误报资源失败 |

这些受控拒绝不等于支持任意形态十万条。普通混合四档同步和后台两次更新都实际成功；极端形态测试允许在共享池/单次构建额度不足时保持现有方案。固定压力矩阵观测最大 RSS 为 437.91 MiB；它与共享预算高水位为不同指标，不宣称整个进程有 512 MiB 硬上限。

## 性能原始证据与复测边界

下方“原始机器样本”保留三轮原始数值及目标程序指纹。原始输出当时保存于 `/private/tmp/fluxa-rules-measurements/delivery`；临时目录不保证长期存在，应以已提交 JSON 和固定版本源码为追溯依据。

当时执行的性能命令如下，仅对应上述历史源码和二进制，不保证当前分支仍有同名示例；复测须先核对当前工具入口，不能将历史命令或结果当作当前运行证据。

```sh
cargo build --offline --release --example runtime_rules_bench --example rules_acceptance --example runtime_rules_memory
python3 Tools/check_rules_acceptance.py --baseline /private/tmp/fluxa-formal-baseline-bench --bench /private/tmp/fluxa-h1-session-lease/target/release/examples/runtime_rules_bench --stress /private/tmp/fluxa-h1-session-lease/target/release/examples/rules_acceptance --output /private/tmp/fluxa-rules-measurements/delivery
```

512 MiB 是估算准入，不是进程 RSS 硬限制；高复杂度输入可能在数量上限之前受控拒绝。历史校准候选在 10000 条高标签后缀并发压力下曾达到 607928320 字节，未达当时 RSS 目标；提高估算系数后，该形态后续更新受控拒绝，旧缓存和快照保留。失败候选不能算作最终通过数据。

真实系统 VPN、私有节点、Intel、最低 macOS、移动平台、长期随机压力、分配故障穷尽，以及分配器和线程栈的内存硬限制未由上述测量验证。数据面正确性、规则语义和安全性不能由吞吐或 RSS 数值证明。

## 原始机器样本

下列 JSON 保留原文件全部内容；未重新运行或改写任何数值。

```json
{
  "environment": {
    "system": "macOS-27.2-arm64-arm-64bit",
    "rust": "rustc 1.98.1 (48a229cea 2026-09-01)"
  },
  "binary_sha256": {
    "baseline": "51d37a7c6b02730f84aeb17d1198cdbba32def246aad99c42cb771cce15de5a1",
    "candidate": "ca30c0a8aff3d55c5dc64bd1634e36d3c4fb50465a913412a38c78b582cee469",
    "stress": "fc831a7efeed1dc581c34344eb8ae9e5ce72e906e7485d007dcb621b18987666"
  },
  "method": "macOS time -l peak bytes; three isolated processes per tier; four concurrent recovery/generation workers",
  "updates": [
    {
      "count": 1000,
      "samples": {
        "baseline": [
          {
            "compile_ms": 2,
            "updates_ms": 29,
            "rss_bytes": 14827520
          },
          {
            "compile_ms": 1,
            "updates_ms": 20,
            "rss_bytes": 14974976
          },
          {
            "compile_ms": 1,
            "updates_ms": 21,
            "rss_bytes": 14811136
          }
        ],
        "candidate": [
          {
            "compile_ms": 2,
            "updates_ms": 24,
            "rss_bytes": 20725760
          },
          {
            "compile_ms": 2,
            "updates_ms": 21,
            "rss_bytes": 20725760
          },
          {
            "compile_ms": 2,
            "updates_ms": 21,
            "rss_bytes": 20758528
          }
        ]
      },
      "queries": {
        "baseline": [
          {
            "p99_ns": {
              "first": 125,
              "last": 167,
              "miss": 167
            },
            "query_process_peak_bytes": 8257536,
            "throughput": 21848919
          },
          {
            "p99_ns": {
              "first": 125,
              "last": 167,
              "miss": 167
            },
            "query_process_peak_bytes": 8060928,
            "throughput": 23315572
          },
          {
            "p99_ns": {
              "first": 125,
              "last": 125,
              "miss": 167
            },
            "query_process_peak_bytes": 8060928,
            "throughput": 24185380
          }
        ],
        "candidate": [
          {
            "p99_ns": {
              "first": 125,
              "last": 167,
              "miss": 167
            },
            "query_process_peak_bytes": 8404992,
            "throughput": 21916757
          },
          {
            "p99_ns": {
              "first": 167,
              "last": 167,
              "miss": 167
            },
            "query_process_peak_bytes": 8175616,
            "throughput": 22132871
          },
          {
            "p99_ns": {
              "first": 125,
              "last": 167,
              "miss": 167
            },
            "query_process_peak_bytes": 8159232,
            "throughput": 22129096
          }
        ]
      },
      "median": {
        "baseline": {
          "compile_ms": 1,
          "updates_ms": 21,
          "rss_bytes": 14827520
        },
        "candidate": {
          "compile_ms": 2,
          "updates_ms": 21,
          "rss_bytes": 20725760
        }
      },
      "background": [
        {
          "updates_ms": 22,
          "submit_us": [
            22,
            63
          ],
          "rss_bytes": 20807680
        },
        {
          "updates_ms": 20,
          "submit_us": [
            20,
            15
          ],
          "rss_bytes": 20791296
        },
        {
          "updates_ms": 22,
          "submit_us": [
            20,
            20
          ],
          "rss_bytes": 20840448
        }
      ]
    },
    {
      "count": 10000,
      "samples": {
        "baseline": [
          {
            "compile_ms": 7,
            "updates_ms": 45,
            "rss_bytes": 29392896
          },
          {
            "compile_ms": 7,
            "updates_ms": 45,
            "rss_bytes": 29196288
          },
          {
            "compile_ms": 7,
            "updates_ms": 45,
            "rss_bytes": 29245440
          }
        ],
        "candidate": [
          {
            "compile_ms": 8,
            "updates_ms": 48,
            "rss_bytes": 38699008
          },
          {
            "compile_ms": 8,
            "updates_ms": 50,
            "rss_bytes": 38764544
          },
          {
            "compile_ms": 8,
            "updates_ms": 50,
            "rss_bytes": 38846464
          }
        ]
      },
      "queries": {
        "baseline": [
          {
            "p99_ns": {
              "first": 167,
              "last": 167,
              "miss": 166
            },
            "query_process_peak_bytes": 12910592,
            "throughput": 22718614
          },
          {
            "p99_ns": {
              "first": 125,
              "last": 167,
              "miss": 167
            },
            "query_process_peak_bytes": 12877824,
            "throughput": 24497420
          },
          {
            "p99_ns": {
              "first": 125,
              "last": 167,
              "miss": 167
            },
            "query_process_peak_bytes": 12664832,
            "throughput": 23700719
          }
        ],
        "candidate": [
          {
            "p99_ns": {
              "first": 166,
              "last": 167,
              "miss": 167
            },
            "query_process_peak_bytes": 13533184,
            "throughput": 22792510
          },
          {
            "p99_ns": {
              "first": 125,
              "last": 167,
              "miss": 167
            },
            "query_process_peak_bytes": 13598720,
            "throughput": 18143058
          },
          {
            "p99_ns": {
              "first": 125,
              "last": 167,
              "miss": 166
            },
            "query_process_peak_bytes": 13549568,
            "throughput": 21642729
          }
        ]
      },
      "median": {
        "baseline": {
          "compile_ms": 7,
          "updates_ms": 45,
          "rss_bytes": 29245440
        },
        "candidate": {
          "compile_ms": 8,
          "updates_ms": 50,
          "rss_bytes": 38764544
        }
      },
      "background": [
        {
          "updates_ms": 49,
          "submit_us": [
            88,
            131
          ],
          "rss_bytes": 38699008
        },
        {
          "updates_ms": 49,
          "submit_us": [
            87,
            90
          ],
          "rss_bytes": 38879232
        },
        {
          "updates_ms": 49,
          "submit_us": [
            100,
            70
          ],
          "rss_bytes": 38928384
        }
      ]
    },
    {
      "count": 50000,
      "samples": {
        "baseline": [
          {
            "compile_ms": 39,
            "updates_ms": 175,
            "rss_bytes": 97222656
          },
          {
            "compile_ms": 37,
            "updates_ms": 175,
            "rss_bytes": 97042432
          },
          {
            "compile_ms": 37,
            "updates_ms": 171,
            "rss_bytes": 96681984
          }
        ],
        "candidate": [
          {
            "compile_ms": 38,
            "updates_ms": 192,
            "rss_bytes": 108904448
          },
          {
            "compile_ms": 39,
            "updates_ms": 189,
            "rss_bytes": 108281856
          },
          {
            "compile_ms": 38,
            "updates_ms": 192,
            "rss_bytes": 109395968
          }
        ]
      },
      "queries": {
        "baseline": [
          {
            "p99_ns": {
              "first": 166,
              "last": 167,
              "miss": 167
            },
            "query_process_peak_bytes": 33914880,
            "throughput": 23448721
          },
          {
            "p99_ns": {
              "first": 125,
              "last": 167,
              "miss": 167
            },
            "query_process_peak_bytes": 33947648,
            "throughput": 24417539
          },
          {
            "p99_ns": {
              "first": 125,
              "last": 167,
              "miss": 167
            },
            "query_process_peak_bytes": 34062336,
            "throughput": 22897323
          }
        ],
        "candidate": [
          {
            "p99_ns": {
              "first": 125,
              "last": 167,
              "miss": 167
            },
            "query_process_peak_bytes": 37781504,
            "throughput": 22250292
          },
          {
            "p99_ns": {
              "first": 167,
              "last": 167,
              "miss": 167
            },
            "query_process_peak_bytes": 37699584,
            "throughput": 20026242
          },
          {
            "p99_ns": {
              "first": 125,
              "last": 167,
              "miss": 167
            },
            "query_process_peak_bytes": 37666816,
            "throughput": 22228192
          }
        ]
      },
      "median": {
        "baseline": {
          "compile_ms": 37,
          "updates_ms": 175,
          "rss_bytes": 97042432
        },
        "candidate": {
          "compile_ms": 38,
          "updates_ms": 192,
          "rss_bytes": 108904448
        }
      },
      "background": [
        {
          "updates_ms": 191,
          "submit_us": [
            224,
            301
          ],
          "rss_bytes": 109756416
        },
        {
          "updates_ms": 187,
          "submit_us": [
            216,
            304
          ],
          "rss_bytes": 108986368
        },
        {
          "updates_ms": 208,
          "submit_us": [
            207,
            239
          ],
          "rss_bytes": 109150208
        }
      ]
    },
    {
      "count": 100000,
      "samples": {
        "baseline": [
          {
            "compile_ms": 84,
            "updates_ms": 342,
            "rss_bytes": 181764096
          },
          {
            "compile_ms": 90,
            "updates_ms": 336,
            "rss_bytes": 183304192
          },
          {
            "compile_ms": 77,
            "updates_ms": 334,
            "rss_bytes": 184172544
          }
        ],
        "candidate": [
          {
            "compile_ms": 90,
            "updates_ms": 415,
            "rss_bytes": 246071296
          },
          {
            "compile_ms": 83,
            "updates_ms": 384,
            "rss_bytes": 247857152
          },
          {
            "compile_ms": 84,
            "updates_ms": 420,
            "rss_bytes": 240107520
          }
        ]
      },
      "queries": {
        "baseline": [
          {
            "p99_ns": {
              "first": 125,
              "last": 167,
              "miss": 125
            },
            "query_process_peak_bytes": 71860224,
            "throughput": 22304367
          },
          {
            "p99_ns": {
              "first": 125,
              "last": 166,
              "miss": 125
            },
            "query_process_peak_bytes": 71794688,
            "throughput": 24344473
          },
          {
            "p99_ns": {
              "first": 125,
              "last": 167,
              "miss": 125
            },
            "query_process_peak_bytes": 71909376,
            "throughput": 22845991
          }
        ],
        "candidate": [
          {
            "p99_ns": {
              "first": 292,
              "last": 167,
              "miss": 167
            },
            "query_process_peak_bytes": 79331328,
            "throughput": 20053141
          },
          {
            "p99_ns": {
              "first": 125,
              "last": 167,
              "miss": 167
            },
            "query_process_peak_bytes": 79249408,
            "throughput": 22188016
          },
          {
            "p99_ns": {
              "first": 125,
              "last": 167,
              "miss": 167
            },
            "query_process_peak_bytes": 79233024,
            "throughput": 21643070
          }
        ]
      },
      "median": {
        "baseline": {
          "compile_ms": 84,
          "updates_ms": 336,
          "rss_bytes": 183304192
        },
        "candidate": {
          "compile_ms": 84,
          "updates_ms": 415,
          "rss_bytes": 246071296
        }
      },
      "background": [
        {
          "updates_ms": 373,
          "submit_us": [
            557,
            525
          ],
          "rss_bytes": 248528896
        },
        {
          "updates_ms": 380,
          "submit_us": [
            551,
            476
          ],
          "rss_bytes": 248578048
        },
        {
          "updates_ms": 377,
          "submit_us": [
            617,
            439
          ],
          "rss_bytes": 240205824
        }
      ]
    }
  ],
  "stress": [
    {
      "count": 1000,
      "shape": "mixed",
      "rss_bytes": 16613376,
      "result": "phase=complete count=1000 shape=mixed total_us=50071 peak_reserved_bytes=88445798 limit_bytes=536870912"
    },
    {
      "count": 10000,
      "shape": "mixed",
      "rss_bytes": 82149376,
      "result": "phase=complete count=10000 shape=mixed total_us=127990 peak_reserved_bytes=261436003 limit_bytes=536870912"
    },
    {
      "count": 50000,
      "shape": "mixed",
      "rss_bytes": 291454976,
      "result": "phase=complete count=50000 shape=mixed total_us=567115 peak_reserved_bytes=534518275 limit_bytes=536870912"
    },
    {
      "count": 100000,
      "shape": "mixed",
      "rss_bytes": 459177984,
      "result": "phase=complete count=100000 shape=mixed total_us=654372 peak_reserved_bytes=524353744 limit_bytes=536870912"
    },
    {
      "count": 1000,
      "shape": "suffix",
      "rss_bytes": 130482176,
      "result": "phase=complete count=1000 shape=suffix total_us=101966 peak_reserved_bytes=265906810 limit_bytes=536870912"
    },
    {
      "count": 10000,
      "shape": "suffix",
      "rss_bytes": 434536448,
      "result": "phase=complete count=10000 shape=suffix total_us=432373 peak_reserved_bytes=529904964 limit_bytes=536870912"
    },
    {
      "count": 100000,
      "shape": "suffix",
      "rss_bytes": 56016896,
      "result": "phase=complete count=100000 shape=suffix admission=rejected peak_reserved_bytes=0"
    },
    {
      "count": 1000,
      "shape": "keyword",
      "rss_bytes": 264847360,
      "result": "phase=complete count=1000 shape=keyword total_us=310056 peak_reserved_bytes=451535522 limit_bytes=536870912"
    },
    {
      "count": 10000,
      "shape": "keyword",
      "rss_bytes": 129187840,
      "result": "phase=complete count=10000 shape=keyword admission=rejected peak_reserved_bytes=285189376"
    },
    {
      "count": 100000,
      "shape": "duplicate",
      "rss_bytes": 129040384,
      "result": "phase=complete count=100000 shape=duplicate total_us=280942 peak_reserved_bytes=492295387 limit_bytes=536870912"
    }
  ]
}
```
<!-- nodus-test-documentation:end -->
