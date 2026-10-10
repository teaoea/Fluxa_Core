import Foundation
#if canImport(FluxaCore)
import FluxaCore
#endif

/// 域名解析使用显式配置的 DNS，地址族由内核筛选。
public enum NativeOutboundFamily: String, Codable, CaseIterable, Sendable {
  case auto, ipv4, ipv6
}

public struct NativeOutboundOpened: Sendable {
  public let id: UInt64
  public let address: String
}

public struct NativeOutboundRead: Sendable {
  public let data: Data
  public let eof: Bool
}

/// 共用出站 FFI 门面；只传递数据与不可复用会话标识，不在 Swift 中读写网络。
public final class NativeOutboundClient: Sendable {
  private let client: NativeCommandClient
  public init() throws {
    client = try NativeCommandClient(requiredCapabilities: 3072)
  }

  public func configureDNS(_ configuration: DNSConfiguration) throws {
    var command = try NativeDNSClient.configurationCommand(configuration)
    command["operation"] = "dns_configure"
    _ = try client.command(command)
  }

  /// 只添加调用者明确提供的信任根，不修改系统信任。
  public func addTrustedRoot(certificatePEM: String) throws {
    _ = try client.command([
      "operation": "add_trusted_root", "certificate_pem": certificatePEM,
    ])
  }

  public func open(
    domain: String,
    port: UInt16,
    transport: RoutingTransport,
    family: NativeOutboundFamily = .auto,
    policy: RoutingRule.Policy = .direct,
    timeoutMilliseconds: UInt64 = 5000
  ) throws -> NativeOutboundOpened {
    let reply = try client.command([
      "operation": "outbound_connect", "domain": domain, "port": port,
      "transport": transport.rawValue, "family": family.rawValue,
      "policy": policy.rawValue, "timeout_ms": timeoutMilliseconds,
    ])
    guard let id = reply.outboundID, let address = reply.outboundPeer
    else { throw NativeClientError.boundary }
    return NativeOutboundOpened(id: id, address: address)
  }

  public func open(
    address: String,
    transport: RoutingTransport,
    policy: RoutingRule.Policy = .direct,
    timeoutMilliseconds: UInt64 = 5000
  ) throws -> UInt64 {
    let reply = try client.command([
      "operation": "outbound_open", "address": address, "transport": transport.rawValue,
      "policy": policy.rawValue, "timeout_ms": timeoutMilliseconds,
    ])
    guard let id = reply.outboundID else { throw NativeClientError.boundary }
    return id
  }

  @discardableResult
  public func write(_ data: Data, id: UInt64, timeoutMilliseconds: UInt64 = 5000) throws -> Int {
    guard data.count <= 65536 else { throw NativeClientError.boundary }
    let reply = try client.command([
      "operation": "outbound_write", "id": id, "body": data.base64EncodedString(),
      "timeout_ms": timeoutMilliseconds,
    ])
    guard let written = reply.outboundWritten else { throw NativeClientError.boundary }
    return written
  }

  public func read(
    id: UInt64,
    maximum: Int = 65536,
    timeoutMilliseconds: UInt64 = 5000
  ) throws -> NativeOutboundRead {
    let reply = try client.command([
      "operation": "outbound_read", "id": id, "maximum": maximum,
      "timeout_ms": timeoutMilliseconds,
    ])
    guard let eof = reply.outboundEOF, let data = Data(base64Encoded: reply.body)
    else { throw NativeClientError.boundary }
    return NativeOutboundRead(data: data, eof: eof)
  }

  public func shutdownWrite(id: UInt64, timeoutMilliseconds: UInt64 = 5000) throws {
    _ = try client.command([
      "operation": "outbound_shutdown_write", "id": id, "timeout_ms": timeoutMilliseconds,
    ])
  }

  /// 可以从另一线程取消阻塞读写；关闭单会话两次会明确拒绝过期标识。
  public func close(id: UInt64) throws {
    _ = try client.command(["operation": "outbound_close", "id": id])
  }

  /// 批量关闭可以重复，后续新会话标识不会复用；析构也会取消并释放全部会话。
  public func closeAll() throws {
    _ = try client.command(["operation": "outbound_close_all"])
  }
}
