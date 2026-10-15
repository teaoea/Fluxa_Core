# 测试文件：tests/test_fixture_quic_packet.py
# 测试类型：单元/集成测试共享夹具，非独立测试目标
# 测试对象：独立 QUIC Initial 报文夹具
# 测试内容：夹具支持函数，不计独立断言覆盖
# 测试依据：《测试文件与报告统一管理原则》及原文件所列公开契约；不反推业务预期。
# 前置条件：Swift 目标需 macOS/固定二进制或源码静态库；夹具需本地测试权限，详见实际执行命令。
# 固定 aioquic 1.2.0 仅作为独立测试工具；生成真实加密 Initial，不建立系统代理。
import sys
from pathlib import Path
from aioquic.quic.configuration import QuicConfiguration
from aioquic.quic.connection import QuicConnection

connection = QuicConnection(configuration=QuicConfiguration(
    is_client=True, server_name="localhost", alpn_protocols=["h3"]
))
connection.connect(("127.0.0.1", 443), now=0)
packet = connection.datagrams_to_send(now=0)[0][0]
if len(packet) != 1200:
    raise RuntimeError("独立 QUIC 测试报文长度无效")
Path(sys.argv[1]).write_bytes(packet)
