import Foundation

/// 仅展示 Rust 解析结果；不在 Swift 中处理 IP 或传输层协议。
public struct NativePacketInfo: Decodable, Equatable, Sendable {
  public let version: UInt8
  public let source: String
  public let destination: String
  public let `protocol`: UInt8
  public let sourcePort: UInt16
  public let destinationPort: UInt16
  public let packetBytes: Int
  public let payloadBytes: Int
  enum CodingKeys: String, CodingKey {
    case version, source, destination, `protocol`
    case sourcePort = "source_port"
    case destinationPort = "destination_port"
    case packetBytes = "packet_bytes"
    case payloadBytes = "payload_bytes"
  }
}

/// TUN 基础数据包检查会话，不代表系统 VPN 或转发引擎已可用。
public final class NativeTunPacketClient: Sendable {
  private let client: NativeCommandClient
  public init(mtu: Int = 1500) throws {
    client = try NativeCommandClient(requiredCapabilities: 32)
    _ = try client.command(["operation": "packet_open", "mtu": mtu])
  }

  public func inspect(_ packet: Data) throws -> NativePacketInfo {
    guard packet.count <= 9000 else { throw NativeClientError.boundary }
    let reply = try client.command([
      "operation": "packet_inspect",
      "packet": packet.base64EncodedString(),
    ])
    guard let info = reply.packetInfo else { throw NativeClientError.boundary }
    return info
  }

  public func close() throws {
    _ = try client.command(["operation": "packet_close"])
  }

  /// 未实现转发之前始终明确拒绝，不会触发系统网络设置。
  public func startTunnel() throws {
    _ = try client.command(["operation": "tun_start"])
  }
}
