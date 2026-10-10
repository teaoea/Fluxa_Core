import Foundation

private struct ServiceFixture: Decodable {
  let port: Int
  enum CodingKeys: String, CodingKey { case port = "dns_port" }
}

@main
struct DNSServiceFFICheck {
  static func main() throws {
    let fixture = try JSONDecoder().decode(
      ServiceFixture.self,
      from: Data(contentsOf: URL(fileURLWithPath: CommandLine.arguments[1]))
    )
    let server = try NativeDNSClient()
    let configuration = DNSConfiguration(
      servers: [DNSServer(transport: .udp, address: "127.0.0.1:\(fixture.port)")],
      timeoutMilliseconds: 1000,
      service: DNSServiceConfiguration()
    )
    try server.validate(configuration)
    try server.configure(configuration)
    let started = try server.startService()
    precondition(started.running && started.port > 0)
    do {
      try server.configure(configuration)
      preconditionFailure("监听期间不能静默替换配置")
    } catch NativeClientError.rejected {}
    try checkClients(port: started.port)
    let status = try server.serviceStatus()
    precondition(status.running && status.queries >= 4)
    let stopped = try server.stopService()
    precondition(!stopped.running && stopped.queries == status.queries)
    let idle = try server.serviceStatus()
    precondition(!idle.running)
    _ = try server.startService()
    try server.close()
    let closed = try server.serviceStatus()
    precondition(!closed.running)
    try checkFakeIP(server: server, upstream: fixture.port)
    for _ in 0 ..< 20 {
      try server.configure(configuration)
      _ = try server.startService()
      try server.close()
    }
    print("DNS 服务真实 Swift FFI：UDP/TCP、缓存、重配置拒绝、停止、Fake-IP、20 次生命周期通过")
  }

  private static func checkClients(port: UInt16) throws {
    for transport in [DNSServer.Transport.udp, .tcp] {
      let downstream = try NativeDNSClient()
      try downstream.configure(DNSConfiguration(
        servers: [DNSServer(transport: transport, address: "127.0.0.1:\(port)")],
        timeoutMilliseconds: 1000
      ))
      let answer = try downstream.query(name: "alias.example.test")
      precondition(answer.addresses == ["192.0.2.44"])
      let missing = try downstream.query(name: "nxdomain.example.test")
      precondition(missing.rcode == 3)
      try downstream.close()
    }
  }

  private static func checkFakeIP(server: NativeDNSClient, upstream: Int) throws {
    try server.configure(DNSConfiguration(
      servers: [DNSServer(transport: .udp, address: "127.0.0.1:\(upstream)")],
      fakeIP: FakeIPConfiguration(), service: DNSServiceConfiguration()
    ))
    let status = try server.startService()
    let downstream = try NativeDNSClient()
    try downstream.configure(DNSConfiguration(
      servers: [DNSServer(transport: .udp, address: "127.0.0.1:\(status.port)")],
      timeoutMilliseconds: 100
    ))
    let answer = try downstream.query(name: "fake.test")
    precondition(answer.addresses == ["198.18.0.1"])
    try checkDig(port: status.port)
    let name = try server.reverseFakeIP(address: "198.18.0.1")
    precondition(name == "fake.test")
    try server.close()
    try downstream.clearCache()
    do {
      _ = try downstream.query(name: "fake.test")
      preconditionFailure("关闭后端口不应继续服务")
    } catch NativeClientError.rejected {}
    try downstream.close()
  }

  private static func checkDig(port: UInt16) throws {
    for mode in ["+notcp", "+tcp"] {
      let process = Process()
      let output = Pipe()
      process.executableURL = URL(fileURLWithPath: "/usr/bin/dig")
      process.arguments = [
        "@127.0.0.1",
        "-p",
        String(port),
        "fake.test",
        "A",
        "+short",
        "+time=1",
        "+tries=1",
        mode,
      ]
      process.standardOutput = output
      process.standardError = output
      try process.run()
      let data = output.fileHandleForReading.readDataToEndOfFile()
      process.waitUntilExit()
      precondition(process.terminationStatus == 0)
      precondition(
        String(decoding: data, as: UTF8.self).trimmingCharacters(in: .whitespacesAndNewlines)
          == "198.18.0.1",
        "独立 dig 客户端应能通过 UDP/TCP 解析"
      )
    }
  }
}
