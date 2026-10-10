import Foundation

/// 参数验证在 Rust；空闲时间只由成功收发刷新，不由失败探测刷新。
public struct OutboundLimits: Codable, Equatable, Sendable {
  public var maximumConnections: Int
  public var idleTimeoutMilliseconds: UInt64
  public init(maximumConnections: Int = 64, idleTimeoutMilliseconds: UInt64 = 300_000) {
    self.maximumConnections = maximumConnections
    self.idleTimeoutMilliseconds = idleTimeoutMilliseconds
  }

  enum CodingKeys: String, CodingKey {
    case maximumConnections = "maximum-connections"
    case idleTimeoutMilliseconds = "idle-timeout-ms"
  }
}
