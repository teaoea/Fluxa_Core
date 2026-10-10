import Foundation
#if canImport(FluxaCore)
import FluxaCore
#endif

public struct NativeRouteDecision: Decodable, Equatable, Sendable {
  public enum Reason: String, Decodable, Sendable { case mode, rule, fallback }
  public let policy: RoutingRule.Policy
  public let outbound: String?
  public let ruleIndex: Int?
  public let reason: Reason
  enum CodingKeys: String, CodingKey {
    case policy, outbound, reason
    case ruleIndex = "rule_index"
  }
}

/// 编译、匹配和可用性检查全部交给 Rust；决策成功不代表连接成功。
public final class NativeRoutingClient: Sendable {
  private let client: NativeCommandClient
  public init() throws {
    client = try NativeCommandClient(requiredCapabilities: 512)
  }

  public func configure(_ configuration: RoutingConfiguration) throws {
    _ = try client.command(Self.configurationCommand(configuration))
  }

  static func configurationCommand(_ configuration: RoutingConfiguration) throws -> [String: Any] {
    var command = try object(configuration.settings)
    command["rules"] = try JSONSerialization
      .jsonObject(with: JSONEncoder().encode(configuration.rules))
    command["operation"] = "route_configure"
    return command
  }

  public func evaluate(_ target: RoutingTarget) throws -> NativeRouteDecision {
    try decision(target, operation: "route_evaluate")
  }

  /// 拒绝策略和未实现代理明确失败，不发送网络流量。
  public func checkSupport(_ target: RoutingTarget) throws -> NativeRouteDecision {
    try decision(target, operation: "route_check")
  }

  public func close() throws {
    _ = try client.command(["operation": "route_close"])
  }

  private func decision(_ target: RoutingTarget, operation: String) throws -> NativeRouteDecision {
    var command = try Self.object(target)
    command["operation"] = operation
    guard let value = try client.command(command).routeDecision
    else { throw NativeClientError.boundary }
    return value
  }

  private static func object(_ value: some Encodable) throws -> [String: Any] {
    guard let object = try JSONSerialization
      .jsonObject(with: JSONEncoder().encode(value)) as? [String: Any]
    else { throw NativeClientError.boundary }
    return object
  }
}
