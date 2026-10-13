import Foundation
#if canImport(FluxaCore)
import FluxaCore
#endif

/// 仅封装 Rust UDP 数据包会话；不创建系统隧道，不在 Swift 中解析或生成包。
public final class NativeTunUDPClient: Sendable {
  private let client: NativeCommandClient
  public init(
    routing: RoutingConfiguration, dns: DNSConfiguration? = nil,
    limits: OutboundLimits = OutboundLimits(), outbounds: [NativeProxyOutbound] = [],
    mtu: Int = 1500, maximumFlows: Int = 16, outputCapacity: Int = 32,
    timeoutMilliseconds: UInt64 = 3000, idleTimeoutMilliseconds: UInt64 = 30000,
    dnsEndpoints: [String] = []
  ) throws {
    client =
      try NativeCommandClient(requiredCapabilities: (1 << 21) |
        (dnsEndpoints.isEmpty ? 0 : 1 << 24))
    try client.configureNetwork(dns: dns, routing: routing, limits: limits, outbounds: outbounds)
    _ = try client.command([
      "operation": "tun_udp_open", "mtu": mtu, "maximum_flows": maximumFlows,
      "output_capacity": outputCapacity, "timeout_ms": timeoutMilliseconds,
      "idle_timeout_ms": idleTimeoutMilliseconds, "dns_endpoints": dnsEndpoints,
    ])
  }

  public func submit(_ packet: Data, mappingGeneration: UInt64? = nil) throws {
    try client.submitTunPacket(packet, mappingGeneration: mappingGeneration)
  }

  /// 无返回包时为 nil；网络错误、超配额与关闭是明确错误。
  public func receive() throws -> Data? {
    try client.receiveTunPacket()
  }

  public func updateNetwork(
    routing: RoutingConfiguration, dns: DNSConfiguration? = nil,
    limits: OutboundLimits = OutboundLimits(), outbounds: [NativeProxyOutbound] = []
  ) throws {
    try client.configureNetwork(dns: dns, routing: routing, limits: limits, outbounds: outbounds)
  }

  public func close() throws {
    _ = try client.command(["operation": "tun_udp_close"])
  }
}
