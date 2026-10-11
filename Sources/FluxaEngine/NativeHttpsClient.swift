import Foundation
#if canImport(FluxaCore)
import FluxaCore
#endif

/// FFI 传输模型；不会将内部 JSON 当作用户配置格式。
public struct NativeHeader: Codable, Sendable {
  public let name: String
  public let value: String
  public init(_ name: String, _ value: String) {
    self.name = name
    self.value = value
  }
}

public struct NativeHttpsResponse: Sendable {
  public let status: Int
  public let headers: [NativeHeader]
  public let body: Data
  public let duration: TimeInterval
}

/// HTTPS 专用门面；句柄与缓冲区生命周期由共用 FFI 客户端负责。
public final class NativeHttpsClient: Sendable {
  private let client: NativeCommandClient

  public init() throws {
    client = try NativeCommandClient(requiredCapabilities: 32769)
  }

  /// 添加用户明确提供的根证书，仍检查上游域名、证书签名与有效期。
  /// DNS、路由及资源参数共用原子配置；失败保持原配置。
  @discardableResult
  public func configureNetwork(
    dns: DNSConfiguration?, routing: RoutingConfiguration,
    limits: OutboundLimits = OutboundLimits(),
    outbounds: [NativeProxyOutbound] = []
  ) throws -> UInt64 {
    try client.configureNetwork(dns: dns, routing: routing, limits: limits, outbounds: outbounds)
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

  public func addTrustedRoot(_ certificatePEM: String) throws {
    _ = try client.command(["operation": "add_trusted_root", "certificate_pem": certificatePEM])
  }

  public func execute(
    method: String,
    url: String,
    headers: [NativeHeader] = [],
    body: Data = Data(),
    proxyPort: UInt16? = nil
  ) throws -> NativeHttpsResponse {
    try executeCommand(
      operation: "https",
      method: method,
      url: url,
      headers: headers,
      payload: (body, proxyPort)
    )
  }

  /// 通用 HTTP/HTTPS 请求；TLS、分帧和网络读写仍全部由 Rust 执行。
  public func executeRequest(
    method: String,
    url: String,
    headers: [NativeHeader] = [],
    body: Data = Data(),
    proxyPort: UInt16? = nil
  ) throws -> NativeHttpsResponse {
    try executeCommand(
      operation: "request",
      method: method,
      url: url,
      headers: headers,
      payload: (body, proxyPort)
    )
  }

  private func executeCommand(
    operation: String,
    method: String,
    url: String,
    headers: [NativeHeader],
    payload: (body: Data, proxyPort: UInt16?)
  ) throws -> NativeHttpsResponse {
    var request: [String: Any] = [
      "operation": operation, "method": method, "url": url,
      "headers": headers.map { ["name": $0.name, "value": $0.value] },
      "body": payload.body.base64EncodedString(),
    ]
    if let proxyPort = payload.proxyPort {
      request["proxy_port"] = proxyPort
    }
    let reply = try client.command(request)
    guard let status = reply.status, let body = Data(base64Encoded: reply.body) else {
      throw NativeClientError.boundary
    }
    return NativeHttpsResponse(
      status: status, headers: reply.headers, body: body,
      duration: Double(reply.durationMilliseconds) / 1000
    )
  }
}
