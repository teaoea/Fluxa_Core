#ifndef FLUXA_ENGINE_H
#define FLUXA_ENGINE_H
#include <stdint.h>
#include <stddef.h>
#ifdef __cplusplus
extern "C" {
#endif
/* ABI v1：值类型边界；不转移任何指针或缓冲区所有权。 */
/* 状态码：0 成功，1 无效句柄，2 能力未实现，3 内部错误。 */
uint32_t fluxa_engine_abi_version(void);
/* 能力位：0 HTTPS，1 MITM，2 同步 JSON 脚本，3 通用 HTTP 调试；不代表代理协议已实现。 */
uint64_t fluxa_engine_capabilities(void);
/* 返回零表示失败；非零句柄应调用 destroy 一次，不可复用。 */
uint64_t fluxa_engine_create(void);
int32_t fluxa_engine_destroy(uint64_t handle);
/* 代理协议尚未实现；有效句柄返回 2，不会降级直连。 */
int32_t fluxa_engine_start(uint64_t handle);
/* input 仅在调用期间借用，须指向 length 个可读且无并发写入的字节。
   返回响应缓冲区句柄，使用后调用 destroy；零表示边界或资源错误。 */
uint64_t fluxa_engine_command(uint64_t handle, const uint8_t *input, size_t length);
size_t fluxa_engine_buffer_length(uint64_t handle);
/* 目标须在调用期可写，不能与 Rust 内存重叠；容量不足不写，返回所需长度。 */
size_t fluxa_engine_buffer_copy(uint64_t handle, uint8_t *output, size_t capacity);
#ifdef __cplusplus
}
#endif
#endif
