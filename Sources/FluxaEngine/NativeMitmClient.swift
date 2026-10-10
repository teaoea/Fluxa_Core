import Foundation
#if canImport(FluxaCore)
import FluxaCore
#endif

/// 仅给平台安全存储使用，包含私钥，禁止导出为用户分享文件或加入日志。
public struct NativeMitmIdentity: Sendable {
  public let certificatePEM: String
  public let privateKeyPEM: String
  public init(certificatePEM: String, privateKeyPEM: String) {
    self.certificatePEM = certificatePEM
    self.privateKeyPEM = privateKeyPEM
  }
}

public struct NativeCapture: Decodable, Sendable {
  public let id: UInt64
  public let method: String
  public let url: String
  public let requestHeaders: [NativeHeader]
  public let responseHeaders: [NativeHeader]
  public let requestBody: Data
  public let responseBody: Data
  public let requestBodyLength: Int
  public let responseBodyLength: Int
  public let status: Int?
  public let intercepted: Bool
  public let durationMilliseconds: UInt64
  public let error: String?
  enum CodingKeys: String, CodingKey {
    case id, method, url, status, intercepted, error
    case requestHeaders = "request_headers"
    case responseHeaders = "response_headers"
    case requestBody = "request_body"
    case responseBody = "response_body"
    case requestBodyLength = "request_body_length"
    case responseBodyLength = "response_body_length"
    case durationMilliseconds = "duration_ms"
  }
}

/// 原生 ABI 的规则传输值；用户配置仍为 YAML，匹配和执行由 Rust 完成。
public struct NativeRewriteRule: Encodable, Sendable {
  public let host: String
  public let pathPrefix: String
  public let action: String
  public let header: String?
  public let value: String?
  public let match: String?
  public let script: String?
  public let jsonKeys: [String]?
  enum CodingKeys: String, CodingKey {
    case host, action, header, value, match, script
    case jsonKeys = "json_keys"
    case pathPrefix = "path_prefix"
  }

  public init(
    host: String,
    pathPrefix: String,
    action: String,
    header: String? = nil,
    value: String? = nil,
    match: String? = nil,
    script: String? = nil,
    jsonKeys: [String]? = nil
  ) {
    self.host = host
    self.pathPrefix = pathPrefix
    self.action = action
    self.header = header
    self.value = value
    self.match = match
    self.script = script
    self.jsonKeys = jsonKeys
  }
}

/// 与 HTTPS 客户端复用同一 FFI 门面，Swift 不实现证书签发、代理或 TLS。
public final class NativeMitmClient: Sendable {
  private let client: NativeCommandClient
  public init() throws {
    client = try NativeCommandClient(requiredCapabilities: 32770)
  }

  /// DNS、路由及资源参数共用原子配置；失败保持原配置。
  @discardableResult
  public func configureNetwork(
    dns: DNSConfiguration?, routing: RoutingConfiguration,
    limits: OutboundLimits = OutboundLimits()
  ) throws -> UInt64 {
    try client.configureNetwork(dns: dns, routing: routing, limits: limits)
  }

  public func outboundStatus() throws -> NativeOutboundStatus {
    guard let status = try client.command(["operation": "outbound_status"]).outboundStatus
    else { throw NativeClientError.boundary }
    return status
  }

  /// 可在其他线程取消正在等待的连接、TLS 或读取；后续请求仍可使用当前配置。
  public func cancelConnections() throws {
    _ = try client.command(["operation": "outbound_close_all"])
  }

  public func addTrustedUpstreamRoot(_ certificatePEM: String) throws {
    _ = try client.command(["operation": "add_trusted_root", "certificate_pem": certificatePEM])
  }

  public func rootCertificate() throws -> String {
    guard let pem = try client.command(["operation": "mitm_certificate"]).certificatePEM else {
      throw NativeClientError.boundary
    }
    return pem
  }

  /// 只导出公开证书，私钥仍由安全存储路径管理。
  public func rootCertificateDER() throws -> Data {
    guard let value = try client.command(["operation": "mitm_certificate"]).certificateDER,
          let bytes = Data(base64Encoded: value) else { throw NativeClientError.boundary }
    return bytes
  }

  public func exportIdentityForSecureStorage() throws -> NativeMitmIdentity {
    let reply = try client.command(["operation": "mitm_export_identity"])
    guard let certificate = reply.certificatePEM, let key = reply.privateKeyPEM else {
      throw NativeClientError.boundary
    }
    return NativeMitmIdentity(certificatePEM: certificate, privateKeyPEM: key)
  }

  public func restoreIdentity(_ identity: NativeMitmIdentity) throws {
    _ = try client.command([
      "operation": "mitm_restore_identity", "certificate_pem": identity.certificatePEM,
      "private_key_pem": identity.privateKeyPEM,
    ])
  }

  public func start(
    port: UInt16 = 0,
    hosts: [String],
    captureBodies: Bool = false,
    allowLAN: Bool = false,
    upstreamProxyPort: UInt16? = nil,
    rewrites: [NativeRewriteRule] = []
  ) throws -> UInt16 {
    let rules = try JSONSerialization.jsonObject(with: JSONEncoder().encode(rewrites))
    var command: [String: Any] = [
      "operation": "mitm_start", "port": port, "hosts": hosts, "capture_bodies": captureBodies,
      "allow_lan": allowLAN, "rewrites": rules,
    ]
    if let upstreamProxyPort {
      command["upstream_proxy_port"] = upstreamProxyPort
    }
    let reply = try client.command(command)
    guard let port = reply.port, port > 0 else { throw NativeClientError.boundary }
    return port
  }

  public func stop() throws {
    _ = try client.command(["operation": "mitm_stop"])
  }

  /// 返回日志与累计淘汰数量；先读取再确认，复制失败不会消费 Rust 中的记录。
  public func peekCapture() throws -> (capture: NativeCapture?, dropped: UInt64) {
    let reply = try client.command(["operation": "mitm_peek_capture"])
    return (reply.capture, reply.droppedCaptures ?? 0)
  }

  public func acknowledgeCapture(_ id: UInt64) throws {
    _ = try client.command(["operation": "mitm_acknowledge_capture", "id": id])
  }
}
