import Foundation

struct TunUDPFixture: Decodable { let udpPort: UInt16
  enum CodingKeys: String, CodingKey { case udpPort = "outbound_udp_port" }
}

@main
struct TunUDPFFICheck {
  static func main() throws {
    let fixture = try JSONDecoder().decode(
      TunUDPFixture.self,
      from: Data(contentsOf: URL(fileURLWithPath: CommandLine.arguments[1]))
    )
    let direct = RoutingConfiguration(settings: RoutingSettings(fallback: .direct))
    for _ in 0 ..< 40 {
      let client = try NativeTunUDPClient(routing: direct, outputCapacity: 2)
      for body in [Data([0, 1, 255, 128, 4]), Data()] {
        let request = packet(port: fixture.udpPort, body: body)
        try client.submit(request)
        var response: Data?
        let deadline = Date().addingTimeInterval(2)
        while response == nil, Date() < deadline {
          response = try client.receive()
          if response == nil {
            Thread.sleep(forTimeInterval: 0.002)
          }
        }
        guard let response else { fatalError("UDP 返回包未到达") }
        precondition(response.count == 28 + body.count && response[0] == 0x45)
        precondition(Array(response[12 ..< 16]) == [127, 0, 0, 1])
        precondition(Array(response[16 ..< 20]) == [192, 0, 2, 1])
        precondition(response.suffix(body.count) == body)
        precondition(checksum(response.prefix(20)) == 0)
        let pseudo = response[12 ..< 20] + Data([0, 17]) + response[24 ..< 26]
        precondition(checksum(pseudo + response.dropFirst(20)) == 0)
      }
      try client.close()
      try client.close()
      try rejected { try client.submit(packet(port: fixture.udpPort, body: Data())) }
      try rejected { _ = try client.receive() }
    }
    let blocked = try NativeTunUDPClient(routing: RoutingConfiguration())
    try rejected { try blocked.submit(packet(port: fixture.udpPort, body: Data([1]))) }
    let raw = try NativeCommandClient(requiredCapabilities: 1 << 21)
    try rejected { _ = try raw.command([
      "operation": "tun_udp_open",
      "mtu": 1500,
      "maximum_flows": 33,
      "output_capacity": 1,
      "timeout_ms": 100,
      "idle_timeout_ms": 100,
    ]) }
    print("TUN UDP 真实 Swift FFI：二进制往返、独立校验和、空报文、规则拒绝、40 次生命周期通过")
  }

  static func rejected(_ action: () throws -> Void) throws {
    do { try action()
      preconditionFailure("应明确拒绝")
    } catch NativeClientError.rejected {}
  }

  /// 独立生成 RFC IPv4 UDP 向量，不使用被测 Rust 编码器。
  static func packet(port: UInt16, body: Data) -> Data {
    var bytes = Data([
      0x45,
      0,
      0,
      0,
      0,
      0,
      0x40,
      0,
      64,
      17,
      0,
      0,
      192,
      0,
      2,
      1,
      127,
      0,
      0,
      1,
      0x30,
      0x39,
      0,
      0,
      0,
      0,
      0,
      0,
    ])
    func word(_ offset: Int, _ value: UInt16) {
      bytes[offset] = UInt8(value >> 8)
      bytes[offset + 1] = UInt8(value & 255)
    }
    bytes.append(body)
    word(2, UInt16(bytes.count))
    word(22, port)
    word(24, UInt16(body.count + 8))
    word(10, checksum(bytes.prefix(20)))
    let pseudo = bytes[12 ..< 20] + Data([0, 17]) + bytes[24 ..< 26]
    let sum = checksum(pseudo + bytes.dropFirst(20))
    word(26, sum == 0 ? 65535 : sum)
    return bytes
  }

  static func checksum(_ bytes: Data) -> UInt16 {
    let bytes = Array(bytes)
    var sum: UInt32 = 0
    for index in stride(from: 0, to: bytes.count, by: 2) {
      sum += UInt32(bytes[index]) << 8
      if index + 1 < bytes.count {
        sum += UInt32(bytes[index + 1])
      }
    }
    while sum > 65535 {
      sum = (sum & 65535) + (sum >> 16)
    }
    return ~UInt16(sum)
  }
}
