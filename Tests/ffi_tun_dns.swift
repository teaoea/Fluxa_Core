import Foundation

struct TunDNSFixture: Decodable {
  let dnsPort: Int
  let udpPort: UInt16
  enum CodingKeys: String, CodingKey {
    case dnsPort = "dns_port"
    case udpPort = "outbound_udp_port"
  }
}

@main
struct TunDNSFFICheck {
  static func main() throws {
    let fixture = try JSONDecoder().decode(
      TunDNSFixture.self,
      from: Data(contentsOf: URL(fileURLWithPath: CommandLine.arguments[1]))
    )
    let dns = DNSConfiguration(
      servers: [DNSServer(transport: .udp, address: "127.0.0.1:\(fixture.dnsPort)")],
      fakeIP: FakeIPConfiguration()
    )
    let routing = RoutingConfiguration(settings: RoutingSettings(fallback: .direct))
    for _ in 0 ..< 40 {
      let client = try NativeTunUDPClient(
        routing: routing,
        dns: dns,
        outputCapacity: 1,
        dnsEndpoints: ["198.19.0.53:53"]
      )
      let query = question("connect.ipv4only.example.test")
      try client.submit(packet(address: [198, 19, 0, 53], port: 53, body: query))
      let response = try wait(client)
      precondition(response[28] == 0x12 && response[29] == 0x34 && response[31] & 15 == 0)
      // A 记录末尾是四字节合成地址；后续原始包不传映射代次。
      let address = Array(response.suffix(4))
      precondition(address.prefix(2) == [198, 18])
      let body = Data("fake-ip bridge".utf8)
      let target = packet(address: address, port: fixture.udpPort, body: body)
      try client.submit(target)
      let echoed = try wait(client)
      precondition(echoed.suffix(body.count) == body && Array(echoed[12 ..< 16]) == address)
      let foreign = try NativeDNSClient()
      try foreign.configure(dns)
      let foreignMapping = try foreign.query(name: "foreign.example.test")
      let foreignAddress = foreignMapping.addresses[0].split(separator: ".").map { UInt8($0)! }
      precondition(foreignAddress != address)
      try rejected { try client.submit(packet(
        address: foreignAddress,
        port: fixture.udpPort,
        body: body
      )) }
      try foreign.close()
      try client.updateNetwork(routing: routing, dns: dns)
      try rejected { try client.submit(packet(address: [198, 19, 0, 53], port: 53, body: query)) }
      try rejected { try client.submit(packet(
        address: address,
        port: fixture.udpPort + 1,
        body: body
      )) }
      try client.close()
      try rejected { try client.submit(target) }
    }
    let real = try NativeTunUDPClient(
      routing: routing,
      dns: DNSConfiguration(servers: dns.servers),
      dnsEndpoints: ["198.19.0.53:53"]
    )
    try real.submit(packet(address: [198, 19, 0, 53], port: 53, body: question("example.test")))
    let realResponse = try wait(real)
    precondition(Array(realResponse.suffix(4)) == [192, 0, 2, 44])
    try real.close()
    print("TUN DNS/Fake-IP 真实 Swift FFI：DNS 返回包、自动所属代次、实际 UDP 转发、替换拒绝及 40 次释放通过")
  }

  static func wait(_ client: NativeTunUDPClient) throws -> Data {
    let deadline = Date().addingTimeInterval(2)
    while Date() < deadline {
      if let packet = try client.receive() {
        return packet
      }
      Thread.sleep(forTimeInterval: 0.002)
    }
    fatalError("返回包超时")
  }

  static func rejected(_ operation: () throws -> Void) throws {
    do { try operation()
      preconditionFailure("必须明确拒绝")
    } catch NativeClientError.kernel, NativeClientError.rejected {}
  }

  static func question(_ name: String) -> Data {
    var bytes = Data([0x12, 0x34, 1, 0, 0, 1, 0, 0, 0, 0, 0, 0])
    for label in name.split(separator: ".") {
      bytes.append(UInt8(label.utf8.count))
      bytes.append(contentsOf: label.utf8)
    }
    bytes.append(contentsOf: [0, 0, 1, 0, 1])
    return bytes
  }

  /// 独立构造 IPv4 向量；不复用被测 Rust 编码器。
  static func packet(address: [UInt8], port: UInt16, body: Data) -> Data {
    var bytes = Data([0x45, 0, 0, 0, 0, 0, 0x40, 0, 64, 17, 0, 0, 192, 0, 2, 1])
    bytes.append(contentsOf: address)
    bytes.append(contentsOf: [0x30, 0x39, UInt8(port >> 8), UInt8(port & 255), 0, 0, 0, 0])
    bytes.append(body)
    func word(_ offset: Int, _ value: UInt16) {
      bytes[offset] = UInt8(value >> 8)
      bytes[offset + 1] = UInt8(value & 255)
    }
    word(2, UInt16(bytes.count))
    word(24, UInt16(body.count + 8))
    var sum: UInt32 = 0
    for index in stride(from: 0, to: 20, by: 2) {
      sum += UInt32(bytes[index]) << 8 | UInt32(bytes[index + 1])
    }
    while sum > 65535 {
      sum = (sum & 65535) + (sum >> 16)
    }
    word(10, ~UInt16(sum))
    return bytes
  }
}
