import Foundation
#if canImport(FluxaCore)
  import FluxaCore
#endif
#if canImport(libfluxa_core)
  import libfluxa_core
#endif

/// 仅编码 IPC 请求，不实现鉴权、规则匹配或内核生命周期决策。
public struct NativeDebugRule: Encodable, Sendable {
  public let rule: RoutingRule
  public let `protocol`: RoutingTransport?
  public let lifetimeSeconds: UInt64
  enum CodingKeys: String, CodingKey { case rule, `protocol`; case lifetimeSeconds = "lifetime_seconds" }
  public init(rule: RoutingRule, protocol: RoutingTransport? = nil, lifetimeSeconds: UInt64) {
    self.rule = rule
    self.protocol = `protocol`
    self.lifetimeSeconds = lifetimeSeconds
  }
}
public struct NativeDebugStatus: Decodable, Sendable {
  public let active: Bool
  public let count: Int
  public let version: UInt64?
  public let nextExpiration: UInt64?
  enum CodingKeys: String, CodingKey { case active, count, version; case nextExpiration = "next_expiration" }
}
public struct NativeDebugAudit: Decodable, Sendable {
  public let timestamp: UInt64
  public let uid: UInt32?
  public let pid: UInt32?
  public let permission: String?
  public let code: UInt16?
}
private struct DebugReply: Decodable {
  let success: Bool
  let errorInfo: NativeKernelError?
  let status: NativeDebugStatus?
  let audit: [NativeDebugAudit]?
  enum CodingKeys: String, CodingKey { case success, status, audit; case errorInfo = "error_info" }
}
/// 专用 Debug 句柄只封装 Rust IPC，不能作为普通业务引擎句柄使用。
/// 同步调用应放在后台执行；权限、TTL、身份和撤销全部由内核裁决。
public final class NativeDebugClient: @unchecked Sendable {
  private let handle: UInt64
  private let lock = NSLock()

  public init(endpoint: String, credential: String, serverUID: UInt32,
              serverRequirement: String, lifetimeSeconds: UInt64) throws {
    #if canImport(libfluxa_core) && os(macOS)
      guard fluxa_engine_capabilities() & (1 << 26) != 0 else { throw NativeClientError.unavailable }
      let data = try JSONSerialization.data(withJSONObject: [
        "endpoint": endpoint, "credential": credential, "server_uid": serverUID,
        "server_requirement": serverRequirement, "lifetime_seconds": lifetimeSeconds,
      ])
      let value = data.withUnsafeBytes {
        fluxa_engine_debug_ipc_connect($0.bindMemory(to: UInt8.self).baseAddress, $0.count)
      }
      guard value != 0 else { throw try Self.boundaryFailure() }
      handle = value
    #else
      throw NativeClientError.unavailable
    #endif
  }
  deinit {
    #if canImport(libfluxa_core) && os(macOS)
      _ = fluxa_engine_destroy(handle)
    #endif
  }
  public func replace(_ rules: [NativeDebugRule], outbound: String? = nil) throws -> NativeDebugStatus {
    let encoded = try JSONEncoder().encode(rules)
    var request: [String: Any] = ["operation": "replace", "rules": try JSONSerialization.jsonObject(with: encoded)]
    if let outbound { request["outbound"] = outbound }
    guard let status = try command(request).status else { throw NativeClientError.boundary }
    return status
  }
  public func clear() throws { _ = try command(["operation": "clear"]) }
  public func close() throws { _ = try command(["operation": "close"]) }
  public func status() throws -> NativeDebugStatus {
    guard let status = try command(["operation": "status"]).status else { throw NativeClientError.boundary }
    return status
  }
  public func audit(maximum: Int = 32) throws -> [NativeDebugAudit] {
    guard let audit = try command(["operation": "audit", "maximum": maximum]).audit else { throw NativeClientError.boundary }
    return audit
  }
  private func command(_ request: [String: Any]) throws -> DebugReply {
    lock.lock()
    defer { lock.unlock() }
    #if canImport(libfluxa_core) && os(macOS)
      let data = try JSONSerialization.data(withJSONObject: request)
      let response = data.withUnsafeBytes {
        fluxa_engine_debug_ipc_command(handle, $0.bindMemory(to: UInt8.self).baseAddress, $0.count)
      }
      guard response != 0 else { throw try Self.boundaryFailure() }
      defer { _ = fluxa_engine_destroy(response) }
      let length = fluxa_engine_buffer_length(response)
      guard length > 0, length <= 256 * 1024 else { throw NativeClientError.boundary }
      var bytes = Data(count: length)
      let copied = bytes.withUnsafeMutableBytes {
        fluxa_engine_buffer_copy(response, $0.bindMemory(to: UInt8.self).baseAddress, $0.count)
      }
      guard copied == length else { throw NativeClientError.boundary }
      let reply = try JSONDecoder().decode(DebugReply.self, from: bytes)
      guard reply.success else {
        guard let error = reply.errorInfo else { throw NativeClientError.boundary }
        throw NativeClientError.kernel(error)
      }
      return reply
    #else
      throw NativeClientError.unavailable
    #endif
  }
  #if canImport(libfluxa_core) && os(macOS)
    private static func boundaryFailure() throws -> NativeClientError {
      var bytes = Data(count: 2048)
      let length = bytes.withUnsafeMutableBytes {
        fluxa_engine_last_error_copy($0.bindMemory(to: UInt8.self).baseAddress, $0.count)
      }
      guard length > 0, length <= bytes.count else { return .boundary }
      return try .kernel(JSONDecoder().decode(NativeKernelError.self, from: bytes.prefix(length)))
    }
  #endif
}
