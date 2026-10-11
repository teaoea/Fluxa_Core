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

/// 代理出站仅传递原始配置；协议校验、认证和网络读写全部由 Rust 完成。
public struct NativeProxyOutbound: Codable, Sendable {
  public let id: String
  public let uri: String

  public init(id: String, uri: String) {
    self.id = id
    self.uri = uri
  }

  #if canImport(FluxaCore)
  /// 节点选择、配置导入和调试共用同一标识与参数转换，不重建网络语义。
  public init(profile: RealityProfile) {
    self.init(id: profile.id.uuidString, uri: profile.uri)
  }
  #endif
}
