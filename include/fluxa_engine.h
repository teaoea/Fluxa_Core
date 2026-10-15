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
/* 能力位：0 HTTPS，1 MITM，2 同步 JSON 脚本，3 通用 HTTP 调试，4 内置流量脚本；不代表代理协议已实现。 */
// 位 11 代表配置 DNS 的域名出站，不代表 Fake-IP 连接或代理出站协议。
// 位 9 代表纯路由决策，位 10 代表数字地址 TCP/UDP 出站，不代表代理协议或 TUN 转发。
// 位 7 代表 Fake-IP 双向映射，不代表 TUN 转发可用。
// 位 6 代表 DNS 客户端；位 8 代表有访问控制的 UDP/TCP DNS 服务。
// 位 5 仅代表 IP 数据包检查，不代表系统 TUN 或 TCP/UDP 转发可用。
// 位 12 代表带映射代次的 Fake-IP 连接，不代表 TUN 转发。
// 位 13 代表路由决策实际执行，不代表逻辑代理出站协议可用。
// 位 14 代表原子网络配置、有界会话状态和自动空闲回收。
// 位 15 代表 HTTP/HTTPS/MITM 使用共用连接链，不代表系统 TUN 可用。
// 位 16 代表来源感知运行时规则生成、严格 YAML、原子更新和索引共享。
// 位 17 代表有界后台规则编译、状态查询与提交前取消，编译期间沿用旧快照。
// 位 18 代表严格 UTC 每日订阅计划；未设置时仅允许手动更新。
// 位 19 代表 VLESS TCP REALITY Vision 出站（含固定浏览器指纹模板），不代表 TUN 或系统代理已实现。
// 位 20 代表有界 MMDB 离线 GeoIP 查询与固定免费公网 IP 回显，不代表系统 TUN 可用。
// 位 21 代表有界 UDP/IP 转发与返回包，不代表 TCP 或系统隧道已实现。
// 位 24 代表显式虚拟 DNS 端点及所属解析器 Fake-IP 自动代次衔接，不代表系统 VPN。
// 位 22 代表单目标 XUDP 代理报文；不代表任意 Mux、空报文或系统 TUN。
// 位 23 代表 TCP/UDP 规则及 QUIC 报文/连接关联拦截，不代表所有短头或迁移均可识别。
// 位 25 代表 u16 错误代码、同线程查询及 command_v2 结构化错误扩展。
// 位 27 代表正式 DOMAIN / DOMAIN-SUFFIX 与 TCP/UDP 的 AND 条件，旧能力位不改义。
uint64_t fluxa_engine_capabilities(void);
/* 返回零表示失败；非零句柄应调用 destroy 一次，不可复用。 */
uint64_t fluxa_engine_create(void);
int32_t fluxa_engine_destroy(uint64_t handle);
/* 系统隧道及入口启动尚未实现；有效句柄返回 1021，不会伪造已连接。 */
int32_t fluxa_engine_start(uint64_t handle);
/* input 仅在调用期间借用，须指向 length 个可读且无并发写入的字节。
   返回响应缓冲区句柄，使用后调用 destroy；零表示边界或资源错误。
   响应名额/预算不足在执行前返回零，读数据尚未消费；释放旧响应后可重试。 */
uint64_t fluxa_engine_command(uint64_t handle, const uint8_t *input, size_t length);
/* 与 command 共用执行、借用及预算，均返回统一 error_info。
   成功/失败仍由 success 表示；响应句柄释放约定不变。 */
uint64_t fluxa_engine_command_v2(uint64_t handle, const uint8_t *input, size_t length);
/* 当前调用线程的最后结构化错误；返回所需长度，无错误为零。
   空目标/容量不足不写；查询不分配、不清除。后续 ABI 操作会覆盖。
   容量足够时 output 须可写；格式及代码见 Documentation/错误代码查询表.md。 */
size_t fluxa_engine_last_error_copy(uint8_t *output, size_t capacity);
/* 最低开销查询：同线程固定 u16 代码，零表示无错误；不分配、不加锁、不清除。 */
uint16_t fluxa_engine_last_error_code(void);
size_t fluxa_engine_buffer_length(uint64_t handle);
/* 目标须在调用期可写，不能与 Rust 内存重叠；容量不足不写，返回所需长度。 */
size_t fluxa_engine_buffer_copy(uint64_t handle, uint8_t *output, size_t capacity);
/* UDP 数据面：输入为一个完整 IP 包，最大 9000 字节；仅同步借用。
   generation=0 表示无 Fake-IP 代次，合成地址不能作为真实目标放行。
   状态：0 成功、11 队列空；全部失败使用 error.rs 登记的四位类别错误代码。
   查询返回受限的真实上下文；旧错误数值不再使用，客户端须同步迁移。
   receive 的 output 须可写、对齐、无并发访问；成功句柄用 destroy 释放。
   错误或队列为空时 output 为零；响应配额不足不会取走队列中的返回包。 */
int32_t fluxa_engine_tun_submit(uint64_t handle, const uint8_t *input, size_t length, uint64_t generation);
int32_t fluxa_engine_tun_receive(uint64_t handle, uint64_t *output);

#ifdef __cplusplus
}
#endif
#endif
