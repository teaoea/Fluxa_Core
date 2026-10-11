import Foundation
#if canImport(FluxaCore)
import FluxaCore
#endif

public struct NativeSubscriptionScheduleDecision: Decodable, Sendable {
  public let due: Bool
  public let nextUpdate: UInt64?
  enum CodingKeys: String, CodingKey {
    case due
    case nextUpdate = "next_update"
  }
}

/// 宿主传入 UTC 时间戳和持久化状态；计划校验与到期判断只由 Rust 执行。
public final class NativeSubscriptionClient: Sendable {
  private let client: NativeCommandClient
  public init() throws {
    client = try NativeCommandClient(requiredCapabilities: 262_144)
  }

  /// HTTPS、重定向、读取上限及 TLS 校验全部由 Rust 共用网络层执行。
  public func configureNetwork(dns: DNSConfiguration) throws {
    _ = try client.configureNetwork(
      dns: dns, routing: RoutingConfiguration(settings: RoutingSettings(mode: .direct)),
      limits: OutboundLimits()
    )
  }

  public func addTrustedRoot(_ certificatePEM: String) throws {
    _ = try client.command(["operation": "add_trusted_root", "certificate_pem": certificatePEM])
  }

  public func cancelConnections() throws {
    _ = try client.command(["operation": "outbound_close_all"])
  }

  public func fetch(url: String) throws -> Data {
    let reply = try client.command(["operation": "subscription_fetch", "url": url])
    guard let data = Data(base64Encoded: reply.body) else { throw NativeClientError.boundary }
    return data
  }

  /// Clash payload 的校验和规则转换全部由 Rust 完成。
  public func parseRules(
    yaml: String,
    policy: RoutingRule.Policy = .proxy,
    outbound: String? = nil
  ) throws -> [RoutingRule] {
    let reply = try client.command([
      "operation": "subscription_rules",
      "yaml": yaml,
      "policy": policy.rawValue,
      "outbound": outbound as Any? ?? NSNull(),
    ])
    guard let rules = reply.subscriptionRules else { throw NativeClientError.boundary }
    return rules
  }

  /// 大订阅仅返回数量，避免把全部规则再次复制到 Swift 内存。
  public func validateRules(
    yaml: String,
    policy: RoutingRule.Policy = .proxy,
    outbound: String? = nil
  ) throws -> Int {
    let reply = try client.command([
      "operation": "subscription_rules",
      "yaml": yaml,
      "policy": policy.rawValue,
      "outbound": outbound as Any? ?? NSNull(),
      "validate_only": true,
    ])
    guard let count = reply.subscriptionRulesCount else { throw NativeClientError.boundary }
    return count
  }

  public func evaluate(
    updateTime: String?, now: UInt64, lastSuccess: UInt64? = nil,
    lastAttempt: UInt64? = nil, enabled: Bool = true, manual: Bool = false
  ) throws -> NativeSubscriptionScheduleDecision {
    let reply = try client.command([
      "operation": "subscription_schedule",
      "schedule": ["update-time": updateTime as Any? ?? NSNull()],
      "now": now, "last_success": lastSuccess as Any? ?? NSNull(),
      "last_attempt": lastAttempt as Any? ?? NSNull(), "enabled": enabled, "manual": manual,
    ])
    guard let result = reply.subscriptionSchedule else { throw NativeClientError.boundary }
    return result
  }
}
