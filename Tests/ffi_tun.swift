import Foundation

/// 独立 RFC 数据包向量通过真实 Swift → Rust 静态库验证。
@main
struct TunFFICheck {
  struct Vector {
    let version: UInt8
    let transport: UInt8
    let encoded: String
  }

  static func main() throws {
    let vectors: [Vector] = [
      Vector(
        version: 4,
        transport: 6,
        encoded: "RQAAKwAAQABABk6WwAACAcYzZAIwOQA1AAAAAQAAAABQAv//ztYAAGFiYw=="
      ),
      Vector(version: 4, transport: 17, encoded: "RQAAHwAAQABAEU6XwAACAcYzZAIwOQA1AAse0GFiYw=="),
      Vector(
        version: 6,
        transport: 6,
        encoded: "YAAAAAAXBkAgAQ24AAAAAAAAAAAAAAABIAENuAAAAAAAAAAAAAAAAjA5ADUAAAABAAAAAFAC//9fmQAAYWJj"
      ),
      Vector(
        version: 6,
        transport: 17,
        encoded: "YAAAAAALEUAgAQ24AAAAAAAAAAAAAAABIAENuAAAAAAAAAAAAAAAAjA5ADUAC6+SYWJj"
      ),
    ]
    for _ in 0 ..< 100 {
      let client = try NativeTunPacketClient()
      for vector in vectors {
        let bytes = Data(base64Encoded: vector.encoded)!
        let info = try client.inspect(bytes)
        precondition(info.version == vector.version && info.protocol == vector.transport)
        precondition(info.sourcePort == 12345 && info.destinationPort == 53)
        precondition(info.payloadBytes == 3 && info.packetBytes == bytes.count)
        for length in 0 ..< bytes.count {
          do { _ = try client.inspect(bytes.prefix(length))
            preconditionFailure("应拒绝截断包")
          } catch NativeClientError.kernel, NativeClientError.rejected {}
        }
        var corrupt = bytes
        corrupt[corrupt.count - 1] ^= 1
        do { _ = try client.inspect(corrupt)
          preconditionFailure("应拒绝校验错误")
        } catch NativeClientError.kernel, NativeClientError.rejected {}
      }
      do { try client.startTunnel()
        preconditionFailure("应拒绝未实现转发")
      } catch NativeClientError.kernel, NativeClientError.rejected {}
      try client.close()
      try client.close()
      do { _ = try client.inspect(Data(base64Encoded: vectors[0].encoded)!)
        preconditionFailure("关闭后应拒绝")
      } catch NativeClientError.kernel, NativeClientError.rejected {}
    }
    for mtu in [-1, 1279, 9001] {
      do { _ = try NativeTunPacketClient(mtu: mtu)
        preconditionFailure("应拒绝无效 MTU")
      } catch NativeClientError.kernel, NativeClientError.rejected {}
    }
    let raw = try NativeCommandClient(requiredCapabilities: 32)
    _ = try raw.command(["operation": "packet_open", "mtu": 1500])
    for command: [String: Any] in [
      ["operation": "packet_open", "mtu": 1500],
      ["operation": "packet_inspect", "packet": "invalid base64"],
      ["operation": "packet_inspect", "packet": String(repeating: "A", count: 12004)],
      ["operation": "packet_open", "mtu": 1500, "unknown": true],
    ] {
      do { _ = try raw.command(command)
        preconditionFailure("应拒绝无效命令")
      } catch NativeClientError.kernel, NativeClientError.rejected {}
    }
    _ = try raw.command(["operation": "packet_close"])
    print("Swift TUN 基础 FFI 通过：IPv4/IPv6、TCP/UDP、截断、校验、关闭及 100 次释放；转发明确未实现。")
  }
}
