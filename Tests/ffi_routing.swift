import Foundation

@main
struct RoutingFFICheck {
  static func main() throws {
    let client = try NativeRoutingClient()
    let config = try RoutingConfiguration(
      settings: RoutingSettings(outbound: "node-default"),
      rules: [
        RoutingRule(type: .domain, value: "api.example.com", policy: .reject),
        RoutingRule(
          type: .domainSuffix,
          value: "example.com",
          policy: .proxy,
          outbound: "node-specific"
        ),
        RoutingRule(type: .ipCIDR, value: "2001:db8::/32", policy: .direct),
        RoutingRule(type: .destinationPort, value: "80-443", policy: .direct),
        RoutingRule(type: .transport, value: "udp", policy: .reject),
      ]
    )
    try client.configure(config)
    let blocked = RoutingTarget(domain: "API.EXAMPLE.COM.")
    let rejected = try client.evaluate(blocked)
    precondition(rejected.policy == .reject && rejected.ruleIndex == 0)
    try expectRejection { _ = try client.checkSupport(blocked) }
    let proxy = RoutingTarget(domain: "a.example.com")
    let chosen = try client.evaluate(proxy)
    precondition(chosen.policy == .proxy && chosen.outbound == "node-specific")
    try expectRejection { _ = try client.checkSupport(proxy) }
    let ipv6 = try client.checkSupport(RoutingTarget(address: "2001:db8::1"))
    precondition(ipv6.policy == .direct && ipv6.ruleIndex == 2)
    let fallback = try client.evaluate(RoutingTarget(domain: "notexample.com", port: 444))
    precondition(fallback.policy == .reject && fallback.reason == .fallback)
    let port = try client.evaluate(RoutingTarget(address: "192.0.2.1", port: 80, transport: .udp))
    precondition(port.policy == .reject && port.ruleIndex == 4)
    try checkInvalidReplacement(client: client, configuration: config)
    try checkModes(client: client)
    for _ in 0 ..< 100 {
      try client.configure(config)
      _ = try client.evaluate(proxy)
      try client.close()
    }
    try expectRejection { _ = try client.evaluate(proxy) }
    try client.close()
    print("路由真实 Swift FFI：顺序、域名边界、IPv6、端口、模式、指定出站、失败保留和 100 次生命周期通过；不建立连接。")
  }

  private static func checkInvalidReplacement(
    client: NativeRoutingClient, configuration: RoutingConfiguration
  ) throws {
    let bad = try JSONDecoder().decode(
      RoutingRule.self,
      from: Data(#"{"type":"IP-CIDR","value":"192.0.2.1/24","policy":"direct"}"#.utf8)
    )
    try expectRejection { try client.configure(RoutingConfiguration(rules: [bad])) }
    let retained = try client.evaluate(RoutingTarget(domain: "api.example.com"))
    precondition(retained.policy == .reject && retained.ruleIndex == 0)
    try client.configure(configuration)
    let raw = try NativeCommandClient(requiredCapabilities: 512)
    try expectRejection {
      _ = try raw.command(["operation": "route_configure", "unknown": true])
    }
  }

  private static func checkModes(client: NativeRoutingClient) throws {
    let target = RoutingTarget(domain: "example.com")
    try client.configure(RoutingConfiguration(settings: RoutingSettings(mode: .direct)))
    let direct = try client.checkSupport(target)
    precondition(direct.policy == .direct && direct.reason == .mode && direct.outbound == nil)
    try client.configure(RoutingConfiguration(settings: RoutingSettings(
      mode: .global,
      outbound: "node-global"
    )))
    let global = try client.evaluate(target)
    precondition(global.policy == .proxy && global.reason == .mode && global
      .outbound == "node-global")
    try expectRejection { _ = try client.checkSupport(target) }
  }

  private static func expectRejection(_ operation: () throws -> Void) throws {
    do { try operation()
      preconditionFailure("应明确拒绝，不能伪造连接或回退直连")
    } catch NativeClientError.rejected {}
  }
}
