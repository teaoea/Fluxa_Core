import Foundation
#if canImport(FluxaCore)
  import FluxaCore
#endif
#if canImport(libfluxa_core)
  import libfluxa_core
#endif

/// 直接保留内核整数协议及上下文，不根据错误文本推断类型。
public struct NativeKernelError: Error, Decodable, Sendable {
  public let code: UInt16
  public let params: NativeKernelErrorParams
}

/// 参数来自受限 Schema，不保存任意 JSON 或完整底层错误链。
public struct NativeKernelErrorParams: Decodable, Sendable {
  public let field: String?
  public let actual: UInt64?
  public let limit: UInt64?
  public let expected: UInt64?
}

public enum NativeClientError: LocalizedError {
  case unavailable, boundary, rejected(String)
  case kernel(NativeKernelError)
  public var errorDescription: String? {
    switch self {
    case .unavailable: "此平台尚未链接所需的 Rust 引擎能力。"
    case .boundary: "Rust FFI 响应无效或超过资源上限。"
    case let .rejected(message): message
    case let .kernel(error): "内核错误代码：\(error.code)"
    }
  }
}

struct NativeCommandReply: Decodable {
  let address: String?
  let geoip: NativeGeoIPLocation?
  let subscriptionRulesCount: Int?
  let subscriptionRules: [RoutingRule]?
  let subscriptionSchedule: NativeSubscriptionScheduleDecision?
  let runtimeRules: NativeRuntimeRulesSummary?
  let runtimeRulesJob: NativeRuntimeRulesJob?
  let networkRevision: UInt64?
  let outboundStatus: NativeOutboundStatus?
  let dnsGeneration: UInt64?
  let outboundPeer: String?
  let outboundID: UInt64?
  let outboundEOF: Bool?
  let outboundWritten: Int?
  let fakeIPName: String?
  let dnsObservation: NativeDNSObservation?
  let dnsAnswer: NativeDNSAnswer?
  let dnsServiceStatus: NativeDNSServiceStatus?
  let packetInfo: NativePacketInfo?
  let routeDecision: NativeRouteDecision?
  let scriptResultJSON: String?
  let success: Bool
  let error: String?
  let errorInfo: NativeKernelError?
  let warn: NativeGeoIPWarning?
  let geoipStatus: NativeGeoIPStatus?
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
    case success, error, status, headers, body, geoip, address, warn
    case port, capture
    case errorInfo = "error_info"
    case geoipStatus = "geoip_status"
    case subscriptionRulesCount = "subscription_rules_count"
    case subscriptionRules = "subscription_rules"
    case subscriptionSchedule = "subscription_schedule"
    case runtimeRules = "runtime_rules"
    case runtimeRulesJob = "runtime_rules_job"
    case networkRevision = "network_revision"
    case outboundStatus = "outbound_status"
    case dnsGeneration = "dns_generation"
    case outboundPeer = "outbound_peer"
    case outboundID = "outbound_id"
    case outboundEOF = "outbound_eof"
    case outboundWritten = "outbound_written"
    case fakeIPName = "fake_ip_name"
    case dnsObservation = "dns_observation"
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
      guard fluxa_engine_abi_version() == 2,
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

  @discardableResult
  func configureNetwork(
    dns: DNSConfiguration?, routing: RoutingConfiguration,
    limits: OutboundLimits, outbounds: [NativeProxyOutbound] = [], geoipURL: String? = nil
  ) throws -> UInt64 {
    var route = try NativeRoutingClient.configurationCommand(routing)
    route.removeValue(forKey: "operation")
    var dnsObject: [String: Any]?
    if let dns {
      dnsObject = try NativeDNSClient.configurationCommand(dns)
      dnsObject?.removeValue(forKey: "operation")
    }
    let object = try JSONSerialization.jsonObject(with: JSONEncoder().encode(limits))
    let reply = try command([
      "operation": "network_configure", "routing": route,
      "geoip_url": geoipURL as Any? ?? NSNull(),
      "dns": dnsObject as Any? ?? NSNull(), "limits": object,
      "outbounds": JSONSerialization.jsonObject(with: JSONEncoder().encode(outbounds)),
    ])
    guard let revision = reply.networkRevision else { throw NativeClientError.boundary }
    return revision
  }

  /// 数据面只传二进制，借用限于同步调用，不经过 JSON 或 Base64。
  func submitTunPacket(_ packet: Data, mappingGeneration: UInt64?) throws {
    #if canImport(libfluxa_core)
      guard !packet.isEmpty, packet.count <= 9000 else { throw NativeClientError.boundary }
      let status = packet.withUnsafeBytes {
        fluxa_engine_tun_submit(
          handle,
          $0.bindMemory(to: UInt8.self).baseAddress,
          $0.count,
          mappingGeneration ?? 0
        )
      }
      try checkTunStatus(status)
    #else
      throw NativeClientError.unavailable
    #endif
  }

  func receiveTunPacket() throws -> Data? {
    #if canImport(libfluxa_core)
      var buffer: UInt64 = 0
      let status = fluxa_engine_tun_receive(handle, &buffer)
      if status == 11 {
        return nil
      }
      try checkTunStatus(status)
      guard buffer != 0 else { throw NativeClientError.boundary }
      defer { _ = fluxa_engine_destroy(buffer) }
      let length = fluxa_engine_buffer_length(buffer)
      guard length > 0, length <= 9000 else { throw NativeClientError.boundary }
      var packet = Data(count: length)
      let copied = packet.withUnsafeMutableBytes {
        fluxa_engine_buffer_copy(buffer, $0.bindMemory(to: UInt8.self).baseAddress, $0.count)
      }
      guard copied == length else { throw NativeClientError.boundary }
      return packet
    #else
      throw NativeClientError.unavailable
    #endif
  }

  #if canImport(libfluxa_core)
    /// 在失败的同步线程、后续 ABI 操作前读取；固定 512 字节上限。
    private func readKernelError() throws -> NativeKernelError? {
      var bytes = Data(count: 512)
      let length = bytes.withUnsafeMutableBytes {
        fluxa_engine_last_error_copy($0.bindMemory(to: UInt8.self).baseAddress, $0.count)
      }
      if length == 0 {
        return nil
      }
      guard length <= bytes.count else { throw NativeClientError.boundary }
      return try JSONDecoder().decode(NativeKernelError.self, from: bytes.prefix(length))
    }
  #endif

  private func checkTunStatus(_ status: Int32) throws {
    guard status != 0 else { return }
    #if canImport(libfluxa_core)
      if let info = try readKernelError() {
        throw NativeClientError.kernel(info)
      }
    #endif
    let message = switch status {
    case 1201: "TUN 引擎句柄无效。"
    case 1021: "TUN 请求的协议或出站尚未支持。"
    case 1001: "TUN 数据包不合法。"
    case 8001, 8002: "TUN 超过资源或并发上限。"
    case 9001: "TUN 会话未打开或已关闭。"
    case 9002: "TUN 操作已取消。"
    case 5002, 5003, 5004, 6001, 6003: "TUN 网络或 DNS 处理失败。"
    case 1101: "TUN 请求被规则拒绝。"
    case 5001, 6002: "TUN 网络操作超时。"
    default: "TUN 内部资源错误。"
    }
    throw NativeClientError.rejected(message)
  }

  func command(_ command: [String: Any]) throws -> NativeCommandReply {
    #if canImport(libfluxa_core)
      let request = try JSONSerialization.data(withJSONObject: command)
      guard request.count <= 12 * 1024 * 1024 else { throw NativeClientError.boundary }
      // 借用范围覆盖整个同步调用；Rust 不保存 Swift 缓冲区指针。
      let responseHandle = request.withUnsafeBytes {
        fluxa_engine_command(handle, $0.bindMemory(to: UInt8.self).baseAddress, $0.count)
      }
      guard responseHandle != 0 else {
        if let info = try readKernelError() {
          throw NativeClientError.kernel(info)
        }
        throw NativeClientError.boundary
      }
      defer { _ = fluxa_engine_destroy(responseHandle) }
      let length = fluxa_engine_buffer_length(responseHandle)
      guard length > 0, length <= 24 * 1024 * 1024 else { throw NativeClientError.boundary }
      var bytes = Data(count: length)
      // 目标由 Swift 拥有，容量准确，复制结束后不保留任何借用。
      let copied = bytes.withUnsafeMutableBytes {
        fluxa_engine_buffer_copy(
          responseHandle,
          $0.bindMemory(to: UInt8.self).baseAddress,
          $0.count
        )
      }
      guard copied == length else { throw NativeClientError.boundary }
      let reply = try JSONDecoder().decode(NativeCommandReply.self, from: bytes)
      guard reply.success else {
        guard let info = reply.errorInfo else { throw NativeClientError.boundary }
        throw NativeClientError.kernel(info)
      }
      return reply
    #else
      throw NativeClientError.unavailable
    #endif
  }
}
