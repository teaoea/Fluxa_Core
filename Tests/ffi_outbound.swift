import Foundation

struct OutboundFixture: Decodable {
  let dns: UInt16
  let dot: UInt16
  let doh: UInt16
  let certificatePEM: String
  let tcp: UInt16
  let udp: UInt16
  enum CodingKeys: String, CodingKey {
    case dns = "dns_port"
    case dot = "dot_port"
    case doh = "doh_port"
    case certificatePEM = "dns_certificate_pem"
    case tcp = "outbound_tcp_port"
    case udp = "outbound_udp_port"
  }
}

@main
struct OutboundFFICheck {
  static func main() throws {
    let fixture = try JSONDecoder().decode(
      OutboundFixture.self,
      from: Data(contentsOf: URL(fileURLWithPath: CommandLine.arguments[1]))
    )
    let client = try NativeOutboundClient()
    let tcp = try client.open(address: "127.0.0.1:\(fixture.tcp)", transport: .tcp)
    try client.write(Data([0, 1, 2, 255]), id: tcp)
    var reply = Data()
    while reply.count < 4 {
      let read = try client.read(id: tcp, maximum: 2)
      precondition(!read.eof)
      reply.append(read.data)
    }
    precondition(reply == Data([0, 1, 2, 255]))
    try client.shutdownWrite(id: tcp)
    let eof = try client.read(id: tcp)
    precondition(eof.eof)
    try client.close(id: tcp)
    try rejection { _ = try client.read(id: tcp) }
    let udp = try client.open(address: "127.0.0.1:\(fixture.udp)", transport: .udp)
    for data in [Data("udp".utf8), Data()] {
      try client.write(data, id: udp)
      let read = try client.read(id: udp)
      precondition(read.data == data && !read.eof)
    }
    try client.write(Data("too-large".utf8), id: udp)
    try rejection { _ = try client.read(id: udp, maximum: 2) }
    try rejection { try client.shutdownWrite(id: udp) }
    try client.close(id: udp)
    for policy in [RoutingRule.Policy.proxy, .reject] {
      try rejection {
        _ = try client.open(address: "127.0.0.1:\(fixture.tcp)", transport: .tcp, policy: policy)
      }
    }
    try rejection { _ = try client.open(address: "example.com:443", transport: .tcp) }
    try checkCancellation(client: client, port: fixture.tcp)
    for _ in 0 ..< 100 {
      let id = try client.open(address: "127.0.0.1:\(fixture.udp)", transport: .udp)
      try client.close(id: id)
    }
    try client.closeAll()
    try client.closeAll()
    try checkDNS(fixture: fixture)
    try checkFakeIP(fixture: fixture)
    try checkResources(fixture: fixture)
    try checkRouting(fixture: fixture)
    try checkRoutedCancellation(fixture: fixture)
    print(
      "共用出站真实 Swift FFI：TCP/UDP、半关闭、EOF、报文边界、取消、100 次生命周期；DNS、Fake-IP、路由、原子配置、统计与空闲回收通过。"
    )
  }

  static func checkResources(fixture: OutboundFixture) throws {
    let client = try NativeOutboundClient()
    let dns = DNSConfiguration(servers: [dnsServers(fixture)[0]], fakeIP: FakeIPConfiguration())
    let routing = RoutingConfiguration(settings: RoutingSettings(mode: .direct))
    let revision = try client.configureNetwork(
      dns: dns, routing: routing,
      limits: NativeOutboundLimits(maximumConnections: 2, idleTimeoutMilliseconds: 5000)
    )
    let mapped = try client.fakeIP(domain: "connect.example.test")
    let opened = try client.openRouted(RoutingTarget(
      address: mapped.address, port: fixture.udp, transport: .udp
    ), family: .ipv4, mappingGeneration: mapped.generation)
    try client.write(Data("stats".utf8), id: opened.id)
    let read = try client.read(id: opened.id)
    precondition(read.data == Data("stats".utf8))
    let status = try client.status()
    precondition(status.revision == revision && status.sessions.count == 1)
    precondition(status.sessions[0].revision == revision)
    precondition(status.sessions[0].uploaded == 5 && status.sessions[0].downloaded == 5)
    var invalidDNS = dns
    invalidDNS.servers = []
    try rejection { _ = try client.configureNetwork(
      dns: invalidDNS, routing: RoutingConfiguration(),
      limits: NativeOutboundLimits(maximumConnections: 1, idleTimeoutMilliseconds: 100)
    ) }
    let afterInvalidDNS = try client.status()
    precondition(afterInvalidDNS.revision == revision)
    try rejection { _ = try client.configureLimits(NativeOutboundLimits(maximumConnections: 0)) }
    let afterInvalidLimits = try client.status()
    precondition(afterInvalidLimits.revision == revision)
    let changed = try client.configureNetwork(dns: dns, routing: routing)
    precondition(changed != revision)
    try rejection { _ = try client.openRouted(
      RoutingTarget(address: mapped.address, port: fixture.udp, transport: .udp),
      mappingGeneration: mapped.generation
    ) }
    try client.write(Data("old".utf8), id: opened.id)
    let oldRead = try client.read(id: opened.id)
    precondition(oldRead.data == Data("old".utf8))
    let other = try NativeOutboundClient()
    let otherID = try other.open(address: "127.0.0.1:\(fixture.udp)", transport: .udp)
    precondition(otherID != opened.id)
    try rejection { try other.close(id: opened.id) }
    try other.closeAll()
    try client.closeAll()
    try client.configureLimits(NativeOutboundLimits(
      maximumConnections: 1,
      idleTimeoutMilliseconds: 100
    ))
    let idle = try client.open(address: "127.0.0.1:\(fixture.tcp)", transport: .tcp)
    let finished = DispatchSemaphore(value: 0)
    DispatchQueue.global().async {
      do {
        _ = try client.read(id: idle, timeoutMilliseconds: 60000)
        preconditionFailure("空闲连接必须取消阻塞读")
      } catch { finished.signal() }
    }
    precondition(finished.wait(timeout: .now() + 2) == .success)
    let expiredStatus = try client.status()
    precondition(expiredStatus.sessions.isEmpty)
    try rejection { try client.close(id: idle) }
    let next = try client.open(address: "127.0.0.1:\(fixture.udp)", transport: .udp)
    precondition(next != idle)
    try client.close(id: next)
  }

  static func checkDNS(fixture: OutboundFixture) throws {
    let client = try NativeOutboundClient()
    try rejection { _ = try client.open(
      domain: "connect.example.test",
      port: fixture.tcp,
      transport: .tcp
    ) }
    try client.addTrustedRoot(certificatePEM: fixture.certificatePEM)
    let servers = dnsServers(fixture)
    for server in servers {
      try client.configureDNS(DNSConfiguration(servers: [server]))
      let opened = try client.open(
        domain: "alias.connect.example.test",
        port: fixture.tcp,
        transport: .tcp
      )
      precondition(opened.address == "127.0.0.1:\(fixture.tcp)")
      try client.write(Data("dns".utf8), id: opened.id)
      let read = try client.read(id: opened.id)
      precondition(read.data == Data("dns".utf8))
      try client.close(id: opened.id)
      let udp = try client.open(
        domain: "connect.example.test",
        port: fixture.udp,
        transport: .udp,
        family: .ipv4
      )
      try client.write(Data("udp".utf8), id: udp.id)
      let datagram = try client.read(id: udp.id)
      precondition(datagram.data == Data("udp".utf8))
      try client.close(id: udp.id)
    }
    for policy in [RoutingRule.Policy.reject, .proxy] {
      try rejection { _ = try client.open(
        domain: "connect.example.test",
        port: fixture.tcp,
        transport: .tcp,
        policy: policy
      ) }
    }
    try client.configureDNS(DNSConfiguration(servers: [servers[0]], fakeIP: FakeIPConfiguration()))
    try rejection { _ = try client.open(
      domain: "connect.example.test",
      port: fixture.tcp,
      transport: .tcp
    ) }
    try client.closeAll()
  }

  static func checkFakeIP(fixture: OutboundFixture) throws {
    let client = try NativeOutboundClient()
    try client.addTrustedRoot(certificatePEM: fixture.certificatePEM)
    for server in dnsServers(fixture) {
      let configuration = DNSConfiguration(servers: [server], fakeIP: FakeIPConfiguration())
      try client.configureDNS(configuration)
      for ipv6 in [false, true] {
        let mapped = try client.fakeIP(domain: "connect.example.test", ipv6: ipv6)
        let endpoint = ipv6 ? "[\(mapped.address)]:\(fixture.tcp)" : "\(mapped.address):\(fixture.tcp)"
        try rejection { _ = try client.open(address: endpoint, transport: .tcp) }
        for transport in [RoutingTransport.tcp, .udp] {
          let opened = try client.open(
            fakeIP: mapped,
            port: transport == .tcp ? fixture.tcp : fixture.udp,
            transport: transport,
            family: .ipv4
          )
          try client.write(Data("fake".utf8), id: opened.id)
          let read = try client.read(id: opened.id)
          precondition(read.data == Data("fake".utf8))
          try client.close(id: opened.id)
        }
        try client.configureDNS(configuration)
        let replacement = try client.fakeIP(domain: "connect.example.test", ipv6: ipv6)
        precondition(replacement.generation != mapped.generation)
        try rejection { _ = try client.open(fakeIP: mapped, port: fixture.tcp, transport: .tcp) }
      }
    }
    try client.configureDNS(DNSConfiguration(
      servers: [DNSServer(transport: .udp, address: "127.0.0.1:\(fixture.dns)")],
      fakeIP: FakeIPConfiguration(exclude: ["connect.example.test"])
    ))
    for policy in [RoutingRule.Policy.reject, .proxy] {
      try rejection { _ = try client.fakeIP(domain: "connect.example.test", policy: policy) }
    }
    var excluded = false
    do {
      _ = try client.fakeIP(domain: "connect.example.test")
    } catch NativeClientError.rejected {
      excluded = true
    }
    precondition(excluded)
    try client.closeAll()
  }

  static func checkRouting(fixture: OutboundFixture) throws {
    let client = try NativeOutboundClient()
    let target = RoutingTarget(domain: "connect.example.test", port: fixture.tcp, transport: .tcp)
    let allow = try RoutingRule(type: .domainSuffix, value: "example.test", policy: .direct)
    let deny = try RoutingRule(type: .ipCIDR, value: "127.0.0.0/8", policy: .reject)
    let reject = try RoutingRule(type: .domainSuffix, value: "example.test", policy: .reject)
    let proxy = try RoutingRule(
      type: .domainSuffix,
      value: "example.test",
      policy: .proxy,
      outbound: "unimplemented"
    )
    try client.configureRouting(RoutingConfiguration(rules: [reject]))
    try rejection { _ = try client.openRouted(target) }
    try client.configureRouting(RoutingConfiguration(rules: [proxy]))
    try rejection { _ = try client.openRouted(target) }
    try client.addTrustedRoot(certificatePEM: fixture.certificatePEM)
    for server in dnsServers(fixture) {
      try client.configureDNS(DNSConfiguration(servers: [server]))
      try client.configureRouting(RoutingConfiguration(rules: [deny, allow]))
      try rejection { _ = try client.openRouted(target, family: .ipv4) }
      try client.configureRouting(RoutingConfiguration(rules: [allow, deny]))
      let opened = try client.openRouted(target)
      precondition(opened.decision.ruleIndex == 0 && opened.decision.policy == .direct)
      try client.write(Data([42]), id: opened.id)
      let read = try client.read(id: opened.id)
      precondition(read.data == Data([42]))
      try client.close(id: opened.id)
      try client.configureRouting(RoutingConfiguration(settings: RoutingSettings(mode: .direct)))
      let udp = try client.openRouted(RoutingTarget(
        address: "127.0.0.1",
        port: fixture.udp,
        transport: .udp
      ))
      precondition(udp.decision.reason == .mode)
      try client.write(Data([43]), id: udp.id)
      let datagram = try client.read(id: udp.id)
      precondition(datagram.data == Data([43]))
      try client.close(id: udp.id)
    }
    try client.configureDNS(DNSConfiguration(
      servers: [dnsServers(fixture)[0]],
      fakeIP: FakeIPConfiguration()
    ))
    try client.configureRouting(RoutingConfiguration(rules: [
      RoutingRule(type: .ipCIDR, value: "198.18.0.0/15", policy: .reject),
      allow,
    ]))
    let mapped = try client.fakeIP(domain: "connect.example.test")
    let fakeTarget = RoutingTarget(address: mapped.address, port: fixture.tcp, transport: .tcp)
    try rejection { _ = try client.openRouted(fakeTarget) }
    let opened = try client.openRouted(
      fakeTarget,
      family: .ipv4,
      mappingGeneration: mapped.generation
    )
    precondition(opened.decision.ruleIndex == 1)
    try client.close(id: opened.id)
    try client.closeAll()
  }

  static func checkRoutedCancellation(fixture: OutboundFixture) throws {
    let client = try NativeOutboundClient()
    try client.configureDNS(DNSConfiguration(servers: [dnsServers(fixture)[0]]))
    try client.configureRouting(RoutingConfiguration(settings: RoutingSettings(mode: .direct)))
    let finished = DispatchSemaphore(value: 0)
    DispatchQueue.global().async {
      do {
        _ = try client.openRouted(
          RoutingTarget(domain: "timeout.cancel.example.test", port: fixture.tcp, transport: .tcp),
          timeoutMilliseconds: 60000
        )
        preconditionFailure("路由解析被取消后不能返回成功")
      } catch { finished.signal() }
    }
    Thread.sleep(forTimeInterval: 0.05)
    try client.closeAll()
    precondition(finished.wait(timeout: .now() + 1) == .success)
  }

  static func dnsServers(_ fixture: OutboundFixture) -> [DNSServer] {
    [
      DNSServer(transport: .udp, address: "127.0.0.1:\(fixture.dns)"),
      DNSServer(transport: .tcp, address: "127.0.0.1:\(fixture.dns)"),
      DNSServer(transport: .dot, address: "127.0.0.1:\(fixture.dot)", serverName: "localhost"),
      DNSServer(
        transport: .doh,
        url: "https://localhost:\(fixture.doh)/dns-query",
        bootstrap: "127.0.0.1:\(fixture.doh)"
      ),
    ]
  }

  static func checkCancellation(client: NativeOutboundClient, port: UInt16) throws {
    let id = try client.open(address: "127.0.0.1:\(port)", transport: .tcp)
    let finished = DispatchSemaphore(value: 0)
    DispatchQueue.global().async {
      do {
        _ = try client.read(id: id, timeoutMilliseconds: 60000)
        preconditionFailure("取消后不能返回成功")
      } catch { finished.signal() }
    }
    Thread.sleep(forTimeInterval: 0.05)
    try client.close(id: id)
    precondition(finished.wait(timeout: .now() + 1) == .success)
  }

  static func rejection(_ action: () throws -> Void) throws {
    var rejected = false
    do { try action() } catch { rejected = true }
    precondition(rejected)
  }
}
