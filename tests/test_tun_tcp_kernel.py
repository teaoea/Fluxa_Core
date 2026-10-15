# 测试文件：tests/test_tun_tcp_kernel.py
# 测试类型：集成测试、真实 Linux 系统 TCP 互通
# 测试对象：src/tun/tcp/、src/tun/session.rs 与 examples/tun_tcp_kernel.rs
# 测试内容：IPv4/IPv6；1 MiB；握手丢包、数据丢包、乱序和半关闭。
# 测试依据：RFC 9293；Fake-IP 恢复及不改变用户系统网络的隔离约束。
# 前置条件：隔离 Linux netns、root、/dev/net/tun、iproute2；不是 macOS VPN 测试。
#!/usr/bin/env python3
"""隔离 Linux 网络命名空间中的真实内核 TCP 客户端，不用另一套自写 TCP 匹配器代替。"""
import fcntl
import collections
import select
import os
import selectors
import socket
import struct
import subprocess
import sys
import threading
import time


def run(binary):
    if sys.platform != "linux" or os.geteuid() != 0:
        raise SystemExit("必须在隔离 Linux 网络命名空间以 root 运行；此环境未测")
    if os.stat("/proc/self/ns/net").st_ino == os.stat("/proc/1/ns/net").st_ino:
        raise SystemExit("拒绝在初始网络命名空间运行，请使用 unshare --net")
    subprocess.run(["ip", "link", "set", "lo", "up"], check=True)
    tunnel = os.open("/dev/net/tun", os.O_RDWR | os.O_NONBLOCK)
    fcntl.ioctl(tunnel, 0x400454CA, struct.pack("16sH", b"fluxa-tcp-test", 0x0001 | 0x1000))
    for args in [["link", "set", "fluxa-tcp-test", "mtu", "1500", "up"], ["addr", "add", "198.19.0.1/32", "dev", "fluxa-tcp-test"], ["route", "add", "198.18.0.0/16", "dev", "fluxa-tcp-test"], ["-6", "addr", "add", "2001:db8:19::1/128", "dev", "fluxa-tcp-test", "nodad"], ["-6", "route", "add", "fd00:198:18::/64", "dev", "fluxa-tcp-test"]]:
        subprocess.run(["ip"] + args, check=True)
    dns = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    dns.bind(("127.0.0.1", 0))
    echo = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    echo.bind(("127.0.0.1", 0))
    echo.listen(16)
    port = echo.getsockname()[1]
    stopped = threading.Event()

    def resolve():
        while not stopped.is_set():
            data, peer = dns.recvfrom(4096)
            offset = 12
            while offset < len(data) and data[offset]:
                offset += 1 + data[offset]
            offset += 5
            kind = struct.unpack("!H", data[offset - 4:offset - 2])[0]
            question = data[12:offset]
            answer = b"\xc0\x0c" + struct.pack("!HHIH", 1, 1, 10, 4) + b"\x7f\x00\x00\x01" if kind == 1 else b""
            response = data[:2] + struct.pack("!HHHHH", 0x8180, 1, int(kind == 1), 0, 0) + question + answer
            dns.sendto(response, peer)

    def handle(peer):
        with peer:
            while True:
                data = peer.recv(65536)
                if not data:
                    peer.sendall(b"after-half-close")
                    peer.shutdown(socket.SHUT_WR)
                    break
                peer.sendall(data)

    def accept():
        while not stopped.is_set():
            peer, _ = echo.accept()
            threading.Thread(target=handle, args=(peer,), daemon=True).start()

    threading.Thread(target=resolve, daemon=True).start()
    threading.Thread(target=accept, daemon=True).start()
    child = subprocess.Popen([binary, str(dns.getsockname()[1])], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, bufsize=0)
    if not select.select([child.stderr], [], [], 10)[0]:
        child.kill()
        child.wait()
        raise RuntimeError("Rust 夹具启动超时")
    ready = child.stderr.readline().decode().strip().split()
    assert len(ready) == 3 and ready[0] == "READY", ready
    diagnostics = collections.deque(maxlen=32)
    def drain_diagnostics():
        for line in child.stderr:
            diagnostics.append(line.decode(errors="replace").strip())
    threading.Thread(target=drain_diagnostics, daemon=True).start()
    mappings = ready[1:]
    errors = []
    fault = {"syn": True, "data": False, "reorder": False, "held": None, "reordered": False}
    lock = threading.Lock()
    output = bytearray()
    active = threading.Event()
    active.set()
    counts = {"syn_dropped": 0, "data_dropped": 0, "reordered": 0}

    def write_packet(packet):
        # 一次 write 对应一个 TUN 包，不能拼接或部分交付。
        deadline = time.monotonic() + 5
        while True:
            try:
                assert os.write(tunnel, packet) == len(packet)
                return
            except BlockingIOError:
                if time.monotonic() >= deadline:
                    raise TimeoutError("TUN 返回包写入超时")
                select.select([], [tunnel], [], 0.1)

    def transport():
        poll = selectors.DefaultSelector()
        poll.register(tunnel, selectors.EVENT_READ, "tun")
        poll.register(child.stdout, selectors.EVENT_READ, "rust")
        try:
            while active.is_set():
                for key, _ in poll.select(0.05):
                    if key.data == "tun":
                        packet = os.read(tunnel, 9000)
                        framed = memoryview(struct.pack("!H", len(packet)) + packet)
                        while framed:
                            count = child.stdin.write(framed)
                            if not count:
                                raise RuntimeError("Rust 输入管道提前关闭")
                            framed = framed[count:]
                        continue
                    data = os.read(child.stdout.fileno(), 65536)
                    if not data:
                        raise RuntimeError("Rust 夹具提前退出")
                    output.extend(data)
                    while len(output) >= 2:
                        size = struct.unpack("!H", output[:2])[0]
                        assert 20 <= size <= 9000
                        if len(output) < size + 2:
                            break
                        packet = bytes(output[2:size + 2])
                        del output[:size + 2]
                        ip = 20 if packet[0] >> 4 == 4 else 40
                        flags = packet[ip + 13]
                        header = (packet[ip + 12] >> 4) * 4
                        with lock:
                            if flags & 0x12 == 0x12 and fault["syn"]:
                                fault["syn"] = False
                                counts["syn_dropped"] += 1
                                continue
                            if len(packet) > ip + header and fault["data"]:
                                fault["data"] = False
                                counts["data_dropped"] += 1
                                continue
                            if len(packet) > ip + header and fault["reorder"]:
                                if fault["held"] is None:
                                    fault["held"] = packet
                                    continue
                                write_packet(packet)
                                write_packet(fault["held"])
                                fault["held"] = None
                                fault["reorder"] = False
                                fault["reordered"] = True
                                counts["reordered"] += 1
                                continue
                        write_packet(packet)
        except BaseException as error:
            if active.is_set():
                errors.append(error)
        finally:
            poll.close()

    bridge = threading.Thread(target=transport, daemon=True)
    bridge.start()
    try:
        for index, (address, mode) in enumerate([(mappings[0], "normal"), (mappings[1], "normal"), (mappings[0], "loss"), (mappings[0], "reorder")]):
            with lock:
                fault["data"] = mode == "loss"
                fault["reorder"] = mode == "reorder"
            client = socket.socket(socket.AF_INET6 if ":" in address else socket.AF_INET, socket.SOCK_STREAM)
            client.settimeout(20)
            if ":" in address:
                client.bind(("2001:db8:19::1", 0))
            with client:
                client.connect((address, port))
                # 双向同时搬运，验证多次环回绕，而不是只验证首个小包。
                payload = bytes((i + index) % 251 for i in range(1024 * 1024))
                sender_errors = []

                def send_all():
                    try:
                        client.sendall(payload)
                        client.shutdown(socket.SHUT_WR)
                    except BaseException as error:
                        sender_errors.append(error)

                sender = threading.Thread(target=send_all)
                sender.start()
                received = bytearray()
                while True:
                    data = client.recv(65536)
                    if not data:
                        break
                    received.extend(data)
                sender.join(5)
                assert not sender.is_alive() and not sender_errors, sender_errors
                assert received == payload + b"after-half-close", (mode, len(received))
                assert not errors, (errors, list(diagnostics))
            print(f"真实 Linux TCP：{address}，{mode}，1 MiB 双向与半关闭通过", flush=True)
        assert counts == {"syn_dropped": 1, "data_dropped": 1, "reordered": 1}, counts
        print("握手丢包、数据丢包重传、乱序及系统客户端字节完整性通过", flush=True)
    finally:
        active.clear()
        bridge.join(2)
        child.stdin.close()
        try:
            child.wait(timeout=5)
        except subprocess.TimeoutExpired:
            child.kill()
            child.wait()
        os.close(tunnel)
        stopped.set()
        dns.close()
        echo.close()


if __name__ == "__main__":
    run(os.path.abspath(sys.argv[1]))
