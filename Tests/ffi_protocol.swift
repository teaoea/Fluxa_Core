import Foundation

/// 独立 REALITY 服务端提供临时配置；Swift 只操作真实 C ABI，不创建网络连接。
@main
struct ProtocolFFICheck {
  static func main() throws {
    let arguments = CommandLine.arguments
    guard arguments.count == 4 else { throw NativeClientError.boundary }
    let profile = try JSONDecoder().decode(
      NativeProxyOutbound.self, from: Data(contentsOf: URL(fileURLWithPath: arguments[1]))
    )
    let fixture = try JSONDecoder().decode(
      ProtocolFixture.self, from: Data(contentsOf: URL(fileURLWithPath: arguments[2]))
    )
    guard let port = UInt16(arguments[3]) else { throw NativeClientError.boundary }
    let routing = RoutingConfiguration(settings: RoutingSettings(
      mode: .global,
      outbound: profile.id
    ))
    let client = try NativeOutboundClient()
    let revision = try client.configureNetwork(dns: nil, routing: routing, outbounds: [profile])
    let opened = try client.openRouted(RoutingTarget(address: "127.0.0.1", port: port))
    precondition(opened.decision.policy == .proxy)
    let payload = Data((0 ..< 65536).map { UInt8($0 % 251) })
    try client.write(payload, id: opened.id)
    var response = Data()
    while response.count < payload.count {
      let read = try client.read(id: opened.id)
      precondition(!read.eof)
      response.append(read.data)
    }
    precondition(response == payload)
    try client.shutdownWrite(id: opened.id)
    try client.close(id: opened.id)
    let afterClose = try client.status()
    precondition(afterClose.sessions.isEmpty)
    try rejection {
      _ = try client.configureNetwork(dns: nil, routing: routing, outbounds: [profile, profile])
    }
    let afterInvalid = try client.status()
    precondition(afterInvalid.revision == revision)
    let udp = try client.openRouted(RoutingTarget(
      address: "127.0.0.1",
      port: UInt16(fixture.udpPort),
      transport: .udp
    ))
    try rejection { _ = try client.read(id: udp.id, timeoutMilliseconds: 20) }
    for payload in [Data([0, 255, 1]), Data(repeating: 7, count: 1472)] {
      try client.write(payload, id: udp.id)
      let read = try client.read(id: udp.id)
      precondition(read.data == payload && !read.eof)
    }
    try rejection { try client.write(Data(), id: udp.id) }
    try rejection { try client.write(Data(repeating: 0, count: 7527), id: udp.id) }
    try rejection { try client.shutdownWrite(id: udp.id) }
    try client.write(Data([3, 2, 1]), id: udp.id)
    try rejection { _ = try client.read(id: udp.id, maximum: 1) }
    try client.write(Data([9]), id: udp.id)
    let recovered = try client.read(id: udp.id)
    precondition(recovered.data == Data([9]))
    try client.close(id: udp.id)
    let blockedRouting = try RoutingConfiguration(
      settings: routing.settings,
      rules: [RoutingRule(type: .transport, value: "quic", policy: .reject)]
    )
    try client.configureNetwork(dns: nil, routing: blockedRouting, outbounds: [profile])
    let blockedUDP = try client.openRouted(RoutingTarget(
      address: "127.0.0.1",
      port: UInt16(fixture.udpPort),
      transport: .udp
    ))
    var quic = Data([0xC0, 0, 0, 0, 1, 1, 1, 1, 2, 0, 17])
    quic.append(Data(repeating: 0, count: 17))
    try rejection { try client.write(quic, id: blockedUDP.id) }
    try client.close(id: blockedUDP.id)
    let tun = try NativeTunUDPClient(routing: routing, outbounds: [profile])
    var packet = Data([
      0x45,
      0,
      0,
      31,
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
      11,
      0,
      0,
      1,
      255,
      0,
    ])
    packet[22] = UInt8(fixture.udpPort >> 8)
    packet[23] = UInt8(fixture.udpPort & 255)
    var checksum: UInt32 = 0
    for offset in stride(from: 0, to: 20, by: 2) {
      checksum += UInt32(packet[offset]) << 8 | UInt32(packet[offset + 1])
    }
    while checksum > 65535 {
      checksum = (checksum & 65535) + (checksum >> 16)
    }
    let check = ~UInt16(checksum)
    packet[10] = UInt8(check >> 8)
    packet[11] = UInt8(check & 255)
    try tun.submit(packet)
    var returned: Data?
    let deadline = Date().addingTimeInterval(3)
    while returned == nil, Date() < deadline {
      returned = try tun.receive()
      if returned == nil {
        Thread.sleep(forTimeInterval: 0.002)
      }
    }
    guard let returned else { throw NativeClientError.boundary }
    precondition(returned.suffix(3) == packet.suffix(3))
    let packetClient = try NativeTunPacketClient()
    let packetInfo = try packetClient.inspect(returned)
    precondition(packetInfo.sourcePort == UInt16(fixture.udpPort) && packetInfo
      .destinationPort == 12345)
    try packetClient.close()
    try tun.close()
    let pending = try client.openRouted(RoutingTarget(address: "127.0.0.1", port: port))
    let started = Date()
    DispatchQueue.global().asyncAfter(deadline: .now() + 0.1) { try? client.closeAll() }
    try rejection { _ = try client.read(id: pending.id, timeoutMilliseconds: 5000) }
    precondition(Date().timeIntervalSince(started) < 3)
    let afterCancel = try client.status()
    precondition(afterCancel.sessions.isEmpty)
    let server = DNSServer(
      transport: .udp, address: "127.0.0.1:\(fixture.dnsPort)"
    )
    let dns = DNSConfiguration(servers: [server])
    let https = try NativeHttpsClient()
    try https.addTrustedRoot(fixture.certificatePEM)
    try https.configureNetwork(dns: dns, routing: routing, outbounds: [profile])
    let result = try https.execute(
      method: "GET", url: "https://localhost:\(fixture.port)/"
    )
    precondition(result.status == 200)
    print("协议真实 Swift FFI：REALITY/VLESS/Vision、64 KiB 往返、HTTPS、重复配置回滚、XUDP 报文往返、边界拒绝及资源释放通过。")
  }

  static func rejection(_ body: () throws -> Void) throws {
    do { try body() } catch { return }
    throw NativeClientError.boundary
  }
}

private struct ProtocolFixture: Decodable {
  let udpPort: Int
  let dnsPort: Int
  let port: Int
  let certificatePEM: String
  enum CodingKeys: String, CodingKey {
    case udpPort = "outbound_udp_port"
    case dnsPort = "dns_port"
    case port
    case certificatePEM = "certificate_pem"
  }
}
