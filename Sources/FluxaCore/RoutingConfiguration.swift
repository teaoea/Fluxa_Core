import Foundation

/// YAML 模式与未命中策略；默认拒绝，不把缺少代理出站转换为直连。
public struct RoutingSettings: Codable, Equatable, Sendable {
  public var mode: RoutingMode
  public var fallback: RoutingRule.Policy
  public var outbound: String?
  public init(
    mode: RoutingMode = .rules,
    fallback: RoutingRule.Policy = .reject,
    outbound: String? = nil
  ) {
    self.mode = mode
    self.fallback = fallback
    self.outbound = outbound
  }

  public init(from decoder: Decoder) throws {
    let container = try decoder.container(keyedBy: CodingKeys.self)
    mode = try container.decodeIfPresent(RoutingMode.self, forKey: .mode) ?? .rules
    fallback = try container.decodeIfPresent(RoutingRule.Policy.self, forKey: .fallback) ?? .reject
    outbound = try container.decodeIfPresent(String.self, forKey: .outbound)
  }

  enum CodingKeys: String, CodingKey { case mode, fallback, outbound }
}

/// 共用值类型，不在 Swift 中进行匹配或网络连接。
public struct RoutingConfiguration: Equatable, Sendable {
  public var settings: RoutingSettings
  public var rules: [RoutingRule]
  public var runtimeHandle: UInt64?
  public init(
    settings: RoutingSettings = RoutingSettings(),
    rules: [RoutingRule] = [],
    runtimeHandle: UInt64? = nil
  ) {
    self.settings = settings
    self.rules = rules
    self.runtimeHandle = runtimeHandle
  }
}

public enum RoutingTransport: String, Codable, CaseIterable, Sendable { case tcp, udp }

public struct RoutingTarget: Codable, Equatable, Sendable {
  public var domain: String?
  public var address: String?
  public var port: UInt16
  public var transport: RoutingTransport
  public init(
    domain: String? = nil,
    address: String? = nil,
    port: UInt16 = 443,
    transport: RoutingTransport = .tcp
  ) {
    self.domain = domain
    self.address = address
    self.port = port
    self.transport = transport
  }
}
