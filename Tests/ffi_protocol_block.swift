import Foundation

@main
struct ProtocolBlockFFICheck {
  static func main() throws {
    let fixture = try JSONDecoder().decode(
      Fixture.self,
      from: Data(contentsOf: URL(fileURLWithPath: CommandLine.arguments[1]))
    )
    let rule = try RoutingRule(type: .transport, value: "quic", policy: .reject)
    let client = try NativeOutboundClient()
    let blocked = RoutingConfiguration(settings: RoutingSettings(fallback: .direct), rules: [rule])
    try client.configureNetwork(dns: nil, routing: blocked)
    let target = RoutingTarget(address: "127.0.0.1", port: fixture.port, transport: .udp)
    let opened = try client.openRouted(target)
    let ordinary = Data("ordinary UDP".utf8)
    try client.write(ordinary, id: opened.id)
    let ordinaryResponse = try client.read(id: opened.id)
    precondition(ordinaryResponse.data == ordinary)
    var packet = Data([0xC0, 0, 0, 0, 1, 1, 1, 1, 2, 0, 17])
    packet.append(Data(repeating: 0, count: 17))
    do { try client.write(packet, id: opened.id)
      throw NativeClientError.boundary
    } catch NativeClientError.rejected {}
    if let path = ProcessInfo.processInfo.environment["FLUXA_QUIC_PACKET_BIN"] {
      let input = try Data(contentsOf: URL(fileURLWithPath: path))
      guard input.count == 1200 else { throw NativeClientError.boundary }
      let actual = try client.openRouted(target)
      do { try client.write(input, id: actual.id)
        throw NativeClientError.boundary
      } catch NativeClientError.rejected {}
    }
    try client.closeAll()
    for _ in 0 ..< 40 {
      try client.configureNetwork(
        dns: nil,
        routing: RoutingConfiguration(settings: RoutingSettings(mode: .direct))
      )
      let allowed = try client.openRouted(target)
      try client.write(packet, id: allowed.id)
      let response = try client.read(id: allowed.id)
      precondition(response.data == packet)
      try client.close(id: allowed.id)
    }
    print("协议拦截真实 Swift FFI：普通 UDP 放行、QUIC 拒绝、配置移除恢复与 40 次回收通过")
  }

  struct Fixture: Decodable {
    let port: UInt16
    enum CodingKeys: String, CodingKey { case port = "outbound_udp_port" }
  }
}
