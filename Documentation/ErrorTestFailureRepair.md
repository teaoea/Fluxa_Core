# 错误体系测试失败修复记录

日期：2026-10-14。验证目录：`/private/tmp/fluxa-core-errors`。本记录只覆盖三项失败的修复，不代表统一错误体系的 L1–L4 已全部完成，也不代表已同步实际内核或已发布。

## 独立校验依据

改造前提交的 `src/ffi.rs::command` 首先执行 `engine(handle)?`，之后才解析命令 JSON。保持旧接口行为是已确认的兼容性约束。因此句柄无效与 JSON 非法同时出现时返回句柄失败；有效句柄的非法 JSON 返回输入失败。此前新增测试把前一种情况预期写成输入失败，是测试预期错误，不能通过改变生产校验顺序使它通过。

## 修改及复现

- `src/ffi.rs::legacy_validation_order_preserves_handle_before_json` 覆盖无效／有效句柄与非法输入／有效命令四种组合；断言分别为 1018、1018、1001、1013。保持旧 JSON 的失败状态及不含 error_info 的断言。
- 测试专用 `TestHandle` 在断言展开时释放已取得的句柄；正常路径仍逐个断言 destroy 成功。原失败测试未释放响应句柄，随后的配额填充又在中途失败并残留资源，造成连锁失败。
- `failed_test_assertion_returns_owned_response_capacity` 主动触发并捕获 panic，之后申请完整 64 个响应名额，验证第 65 个受控拒绝，再逐一验证释放成功。这项仅测试测试守卫，不改变业务注册表。
- 更新生产边界注释、错误协议说明和 `Tests/ffi_error_code.swift` 示例，真实跨语言验证两种非法输入的错误分类。

## 实际检查结果

| 命令或检查 | 结果 |
| --- | --- |
| cargo fmt --check | 通过 |
| cargo clippy --offline --all-targets --target-dir /private/tmp/fluxa-errors-target -- -D warnings | 通过 |
| cargo test --offline --target-dir /private/tmp/fluxa-errors-target | 206 通过、0 失败、1 忽略；忽略项为已有真实远端互通测试 |
| cargo run --offline --example error_contract --target-dir /private/tmp/fluxa-errors-target -- --check | 通过，20 个整数错误代码契约一致 |
| cargo build --offline --lib + swiftc 编译及执行 Tests/ffi_error_code.swift | 通过：旧格式、新整数协议、校验顺序、响应配额耗尽错误查询、成功清除旧错误 |

本轮日志分别位于 `/private/tmp/fluxa-errors-repair-final-tests.log`、`/private/tmp/fluxa-errors-repair-final-clippy.log`、`/private/tmp/fluxa-errors-repair-doc.log`。Swift 程序实际执行输出已核对。本轮没有发布、没有改规则优先级，也没有以编译和测试结果替代其余独立业务语义审查。
