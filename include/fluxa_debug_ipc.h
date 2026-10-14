#ifndef FLUXA_DEBUG_IPC_H
#define FLUXA_DEBUG_IPC_H
#include <stdint.h>
#include <stddef.h>
#ifdef __cplusplus
extern "C" {
#endif
/* 专用 Debug IPC，仅能力位 26 可用时提供安全平台支持。
   与普通 command 分离；JSON 全部字段严格验证，不接受客户端自称身份/权限。
   start: root 管理的只读策略路径与私有目录；连接验证 OS 身份、代码签名和凭证。
   connect: endpoint/credential/server_uid/server_requirement/lifetime_seconds。
   command: 逐操作授权的 replace/clear/status/close/audit；响应缓冲区必须 destroy。
   禁止使用 TCP/UDP，禁止将 Debug 写入普通配置；客户端句柄 destroy 断开并撤销授权。 */
uint64_t fluxa_engine_debug_ipc_connect(const uint8_t *input, size_t length);
uint64_t fluxa_engine_debug_ipc_command(uint64_t handle, const uint8_t *input, size_t length);
int32_t fluxa_engine_debug_ipc_start(uint64_t engine, const uint8_t *input, size_t length);
#ifdef __cplusplus
}
#endif
#endif
