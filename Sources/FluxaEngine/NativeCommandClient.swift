import Foundation
#if canImport(libfluxa_core)
import libfluxa_core
#endif

public enum NativeClientError: LocalizedError {
  case unavailable, boundary, rejected(String)
  public var errorDescription: String? {
    switch self {
    case .unavailable: "此平台尚未链接所需的 Rust 引擎能力。"
    case .boundary: "Rust FFI 响应无效或超过资源上限。"
    case let .rejected(message): message
    }
  }
}

struct NativeCommandReply: Decodable {
  let dnsGeneration: UInt64?
  let outboundPeer: String?
  let outboundID: UInt64?
  let outboundEOF: Bool?
  let outboundWritten: Int?
  let fakeIPName: String?
  let dnsAnswer: NativeDNSAnswer?
  let dnsServiceStatus: NativeDNSServiceStatus?
  let packetInfo: NativePacketInfo?
  let routeDecision: NativeRouteDecision?
  let scriptResultJSON: String?
  let success: Bool
  let error: String?
  let status: Int?
  let headers: [NativeHeader]
  let body: String
  let port: UInt16?
  let certificatePEM: String?
  let certificateDER: String?
  let privateKeyPEM: String?
  let capture: NativeCapture?
  let droppedCaptures: UInt64?
  let durationMilliseconds: UInt64
  enum CodingKeys: String, CodingKey {
    case success, error, status, headers, body
    case port, capture
    case dnsGeneration = "dns_generation"
    case outboundPeer = "outbound_peer"
    case outboundID = "outbound_id"
    case outboundEOF = "outbound_eof"
    case outboundWritten = "outbound_written"
    case fakeIPName = "fake_ip_name"
    case dnsAnswer = "dns_answer"
    case dnsServiceStatus = "dns_service_status"
    case packetInfo = "packet_info"
    case routeDecision = "route_decision"
    case scriptResultJSON = "script_result_json"
    case certificatePEM = "certificate_pem"
    case certificateDER = "certificate_der"
    case privateKeyPEM = "private_key_pem"
    case droppedCaptures = "dropped_captures"
    case durationMilliseconds = "duration_ms"
  }
}

/// 仅管理 FFI 生命周期，不在 Swift 中实现 TLS 或网络读写。
/// 出站读写支持受限并发和取消；其他业务拒绝重入，耗时调用应放在后台执行。
final class NativeCommandClient: Sendable {
  private let handle: UInt64

  init(requiredCapabilities: UInt64) throws {
    #if canImport(libfluxa_core)
    guard fluxa_engine_abi_version() == 1,
          fluxa_engine_capabilities() & requiredCapabilities == requiredCapabilities
    else { throw NativeClientError.unavailable }
    let value = fluxa_engine_create()
    guard value != 0 else { throw NativeClientError.boundary }
    handle = value
    #else
    throw NativeClientError.unavailable
    #endif
  }

  deinit {
    #if canImport(libfluxa_core)
    _ = fluxa_engine_destroy(handle)
    #endif
  }

  func command(_ command: [String: Any]) throws -> NativeCommandReply {
    #if canImport(libfluxa_core)
    let request = try JSONSerialization.data(withJSONObject: command)
    guard request.count <= 12 * 1024 * 1024 else { throw NativeClientError.boundary }
    // 借用范围覆盖整个同步调用；Rust 不保存 Swift 缓冲区指针。
    let responseHandle = request.withUnsafeBytes {
      fluxa_engine_command(handle, $0.bindMemory(to: UInt8.self).baseAddress, $0.count)
    }
    guard responseHandle != 0 else { throw NativeClientError.boundary }
    defer { _ = fluxa_engine_destroy(responseHandle) }
    let length = fluxa_engine_buffer_length(responseHandle)
    guard length > 0, length <= 24 * 1024 * 1024 else { throw NativeClientError.boundary }
    var bytes = Data(count: length)
    // 目标由 Swift 拥有，容量准确，复制结束后不保留任何借用。
    let copied = bytes.withUnsafeMutableBytes {
      fluxa_engine_buffer_copy(responseHandle, $0.bindMemory(to: UInt8.self).baseAddress, $0.count)
    }
    guard copied == length else { throw NativeClientError.boundary }
    let reply = try JSONDecoder().decode(NativeCommandReply.self, from: bytes)
    guard reply.success else { throw NativeClientError.rejected(reply.error ?? "Rust 请求失败。") }
    return reply
    #else
    throw NativeClientError.unavailable
    #endif
  }
}
