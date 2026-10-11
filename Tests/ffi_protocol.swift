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
    try rejection {
      _ = try client.openRouted(RoutingTarget(address: "127.0.0.1", port: port, transport: .udp))
    }
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
    print("协议真实 Swift FFI：REALITY/VLESS/Vision、64 KiB 往返、HTTPS、重复配置回滚、UDP 明确拒绝及资源释放通过。")
  }

  static func rejection(_ body: () throws -> Void) throws {
    do { try body() } catch { return }
    throw NativeClientError.boundary
  }
}

private struct ProtocolFixture: Decodable {
  let dnsPort: Int
  let port: Int
  let certificatePEM: String
  enum CodingKeys: String, CodingKey {
    case dnsPort = "dns_port"
    case port
    case certificatePEM = "certificate_pem"
  }
}
