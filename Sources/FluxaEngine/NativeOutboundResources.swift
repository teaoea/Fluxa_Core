import Foundation
#if canImport(FluxaCore)
import FluxaCore
#endif

public typealias NativeOutboundLimits = OutboundLimits

public struct NativeOutboundSessionStatus: Decodable, Sendable {
  public let id: UInt64
  public let address: String
  public let transport: RoutingTransport
  public let revision: UInt64
  public let uploaded: UInt64
  public let downloaded: UInt64
  public let idleMilliseconds: UInt64
  public let idleTimeoutMilliseconds: UInt64
  enum CodingKeys: String, CodingKey {
    case id, address, transport, revision, uploaded, downloaded
    case idleMilliseconds = "idle_ms"
    case idleTimeoutMilliseconds = "idle_timeout_ms"
  }
}

public struct NativeOutboundStatus: Decodable, Sendable {
  public let revision: UInt64
  public let limits: NativeOutboundLimits
  public let sessions: [NativeOutboundSessionStatus]
}
