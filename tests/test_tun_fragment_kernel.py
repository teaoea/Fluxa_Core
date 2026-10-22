#!/usr/bin/env python3
# 测试文件：tests/test_tun_fragment_kernel.py
# 测试类型：集成测试、真实 Linux TUN 与系统 IP 重组
# 测试对象：src/tun/fragment.rs、src/tun/udp.rs、examples/tun_tcp_kernel.rs
# 测试内容：双栈 8001 字节 UDP 返回、逆序重组、缺片不交付、后续数据报恢复及完整字节校验。
# 测试依据：RFC 791、8200；由系统协议栈独立重组，不用生产重组器充当正确性标准。
# 前置条件：隔离 Linux netns、root、/dev/net/tun、iproute2；不接管用户网络。
import fcntl
import os
import select
import socket
import struct
import subprocess
import sys
import threading
import time


def run(binary):
    if sys.platform != 'linux' or os.geteuid() != 0:
        raise SystemExit('必须在 Linux 隔离网络命名空间以 root 运行；此环境未测')
    if os.stat('/proc/self/ns/net').st_ino == os.stat('/proc/1/ns/net').st_ino:
        raise SystemExit('拒绝在初始网络命名空间测试')
    subprocess.run(['ip', 'link', 'set', 'lo', 'up'], check=True)
    tun = os.open('/dev/net/tun', os.O_RDWR | os.O_NONBLOCK)
    child = None
    active = threading.Event()
    active.set()
    errors = []
    sockets = []
    try:
        fcntl.ioctl(tun, 0x400454CA, struct.pack('16sH', b'nodus-frag-test', 0x1001))
        for args in [['link', 'set', 'nodus-frag-test', 'mtu', '1500', 'up'], ['addr', 'add', '198.19.0.1/32', 'dev', 'nodus-frag-test'], ['route', 'add', '198.18.0.0/16', 'dev', 'nodus-frag-test'], ['-6', 'addr', 'add', '2001:db8:19::1/128', 'dev', 'nodus-frag-test', 'nodad'], ['-6', 'route', 'add', 'fd00:198:18::/64', 'dev', 'nodus-frag-test']]:
            subprocess.run(['ip'] + args, check=True)
        dns = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        dns.bind(('127.0.0.1', 0))
        echo = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        echo.bind(('127.0.0.1', 0))
        sockets += [dns, echo]
        body = bytes(i % 251 for i in range(8001))

        def resolve():
            while active.is_set():
                data, peer = dns.recvfrom(4096)
                end = 12
                while data[end]:
                    end += 1 + data[end]
                end += 5
                kind = struct.unpack('!H', data[end-4:end-2])[0]
                answer = b'\xc0\x0c' + struct.pack('!HHIH', 1, 1, 10, 4) + socket.inet_aton('127.0.0.1') if kind == 1 else b''
                dns.sendto(data[:2] + struct.pack('!HHHHH', 0x8180, 1, int(kind == 1), 0, 0) + data[12:end] + answer, peer)

        def respond():
            while active.is_set():
                data, peer = echo.recvfrom(1024)
                assert data in (b'large', b'next'), data
                echo.sendto(body if data == b'large' else b'after-fragment', peer)

        threading.Thread(target=resolve, daemon=True).start()
        threading.Thread(target=respond, daemon=True).start()
        child = subprocess.Popen([binary, str(dns.getsockname()[1])], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, bufsize=0)
        if not select.select([child.stderr], [], [], 10)[0]:
            raise TimeoutError('Rust 夹具未就绪')
        ready = child.stderr.readline().decode().split()
        assert len(ready) == 3 and ready[0] == 'READY', ready
        diagnostic = []
        def diagnostics():
            for line in child.stderr:
                diagnostic.append(line.decode(errors='replace').strip())
        threading.Thread(target=diagnostics, daemon=True).start()
        mode = ['normal']
        count = [0]
        batches = []
        output = bytearray()

        def write(packet):
            deadline = time.monotonic() + 3
            while True:
                try:
                    assert os.write(tun, packet) == len(packet)
                    return
                except BlockingIOError:
                    if time.monotonic() > deadline:
                        raise TimeoutError('TUN 写入超时')
                    select.select([], [tun], [], .01)

        def transport():
            try:
                while active.is_set():
                    readers, _, _ = select.select([tun, child.stdout], [], [], .05)
                    if tun in readers:
                        packet = os.read(tun, 9000)
                        # 系统接口会产生 IPv6 邻居/路由发现；本夹具只转交本次受测 UDP。
                        if packet[9 if packet[0] >> 4 == 4 else 6] != 17:
                            continue
                        framed = memoryview(struct.pack('!H', len(packet)) + packet)
                        while framed:
                            n = child.stdin.write(framed)
                            if not n: raise RuntimeError('Rust 输入管道关闭')
                            framed = framed[n:]
                    if child.stdout in readers:
                        data = os.read(child.stdout.fileno(), 65536)
                        if not data: raise RuntimeError('Rust 输出管道关闭')
                        output.extend(data)
                        while len(output) >= 2:
                            size = struct.unpack('!H', output[:2])[0]
                            assert 20 <= size <= 1500, size
                            if len(output) < size + 2: break
                            packet = bytes(output[2:size+2])
                            del output[:size+2]
                            v6 = packet[0] >> 4 == 6
                            fragmented = packet[6] == 44 if v6 else struct.unpack('!H', packet[6:8])[0] & 0x3fff != 0
                            if not fragmented:
                                write(packet)
                                continue
                            field = struct.unpack('!H', packet[42:44] if v6 else packet[6:8])[0]
                            more = field & (1 if v6 else 0x2000)
                            batches.append(packet)
                            if not more:
                                assert len(batches) == 6, len(batches)
                                count[0] += 1
                                packets = list(reversed(batches)) if mode[0] == 'reverse' else list(batches)
                                if mode[0] == 'drop': packets = packets[1:]
                                for fragment in packets: write(fragment)
                                batches.clear()
            except BaseException as error:
                if active.is_set(): errors.append(error)

        bridge = threading.Thread(target=transport, daemon=True)
        bridge.start()
        for address in ready[1:]:
            family = socket.AF_INET6 if ':' in address else socket.AF_INET
            with socket.socket(family, socket.SOCK_DGRAM) as client:
                client.settimeout(5)
                if family == socket.AF_INET6: client.bind(('2001:db8:19::1', 0))
                client.connect((address, echo.getsockname()[1]))
                for scenario in ['normal', 'reverse', 'drop']:
                    mode[0] = scenario
                    client.send(b'large')
                    if scenario == 'drop':
                        client.settimeout(.3)
                        try:
                            client.recv(9000)
                            raise AssertionError('缺片数据报不得交付')
                        except socket.timeout: pass
                        client.settimeout(5)
                    else:
                        assert client.recv(9000) == body, scenario
                    client.send(b'next')
                    assert client.recv(9000) == b'after-fragment', scenario
                    assert not errors, (errors, diagnostic)
            print(f'真实 Linux 出站分片：{address}，顺序/逆序重组、缺片拒绝及后续报文通过', flush=True)
        assert count[0] == 6, count
        assert not diagnostic, diagnostic
        active.clear()
        bridge.join(2)
    finally:
        active.clear()
        if child:
            child.stdin.close()
            try: child.wait(timeout=5)
            except subprocess.TimeoutExpired:
                child.kill()
                child.wait()
        os.close(tun)
        for sock in sockets: sock.close()


if __name__ == '__main__':
    run(os.path.abspath(sys.argv[1]))
