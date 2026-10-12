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
