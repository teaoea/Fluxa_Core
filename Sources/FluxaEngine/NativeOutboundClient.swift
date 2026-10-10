import Foundation
#if canImport(FluxaCore)
import FluxaCore
#endif

/// 域名解析使用显式配置的 DNS，地址族由内核筛选。
public enum NativeOutboundFamily: String, Codable, CaseIterable, Sendable {
  case auto, ipv4, ipv6
}

/// 映射令牌仅在原客户端及当前进程有效，不用于存储或跨解析器传递。
public struct NativeFakeIPAddress: Sendable {
  public let address: String
  public let generation: UInt64
}

public struct NativeRoutedOpened: Sendable {
  public let id: UInt64
  public let address: String
  public let decision: NativeRouteDecision
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
    client = try NativeCommandClient(requiredCapabilities: 31744)
  }

  /// DNS、规则及资源参数全部验证成功才替换，返回本进程配置修订号。
  @discardableResult
  public func configureNetwork(
    dns: DNSConfiguration?, routing: RoutingConfiguration,
    limits: NativeOutboundLimits = NativeOutboundLimits()
  ) throws -> UInt64 {
    var route = try NativeRoutingClient.configurationCommand(routing)
    route.removeValue(forKey: "operation")
    var dnsObject: [String: Any]?
    if let dns {
      dnsObject = try NativeDNSClient.configurationCommand(dns)
      dnsObject?.removeValue(forKey: "operation")
    }
    let object = try JSONSerialization.jsonObject(with: JSONEncoder().encode(limits))
    let reply = try client.command([
      "operation": "network_configure", "routing": route,
      "dns": dnsObject as Any? ?? NSNull(), "limits": object,
    ])
    guard let revision = reply.networkRevision else { throw NativeClientError.boundary }
    return revision
  }

  @discardableResult
  public func configureLimits(_ limits: NativeOutboundLimits) throws -> UInt64 {
    guard var command = try JSONSerialization.jsonObject(
      with: JSONEncoder().encode(limits)
    ) as? [String: Any] else { throw NativeClientError.boundary }
    command["operation"] = "outbound_limits"
    guard let revision = try client.command(command).networkRevision
    else { throw NativeClientError.boundary }
    return revision
  }

  public func status() throws -> NativeOutboundStatus {
    guard let status = try client.command(["operation": "outbound_status"]).outboundStatus
    else { throw NativeClientError.boundary }
    return status
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

  public func configureRouting(_ configuration: RoutingConfiguration) throws {
    _ = try client.command(NativeRoutingClient.configurationCommand(configuration))
  }

  public func openRouted(
    _ target: RoutingTarget, family: NativeOutboundFamily = .auto,
    mappingGeneration: UInt64? = nil, timeoutMilliseconds: UInt64 = 5000
  ) throws -> NativeRoutedOpened {
    let object = try JSONSerialization.jsonObject(with: JSONEncoder().encode(target))
    var command: [String: Any] = [
      "operation": "outbound_route", "target": object, "family": family.rawValue,
      "timeout_ms": timeoutMilliseconds,
    ]
    if let mappingGeneration {
      command["mapping_generation"] = mappingGeneration
    }
    let reply = try client.command(command)
    guard let id = reply.outboundID, let address = reply.outboundPeer,
          let decision = reply.routeDecision
    else { throw NativeClientError.boundary }
    return NativeRoutedOpened(id: id, address: address, decision: decision)
  }

  public func fakeIP(
    domain: String,
    ipv6: Bool = false,
    policy: RoutingRule.Policy = .direct
  ) throws -> NativeFakeIPAddress {
    let reply = try client.command([
      "operation": "outbound_fake_ip", "domain": domain,
      "ipv6": ipv6, "policy": policy.rawValue,
    ])
    guard let answer = reply.dnsAnswer else { throw NativeClientError.boundary }
    guard answer.validation == "synthetic" else {
      throw NativeClientError.rejected("此域名未生成 Fake-IP 映射，请检查配置及排除项。")
    }
    guard let address = answer.addresses.first, let generation = reply.dnsGeneration
    else { throw NativeClientError.boundary }
    return NativeFakeIPAddress(address: address, generation: generation)
  }

  public func open(
    fakeIP: NativeFakeIPAddress, port: UInt16, transport: RoutingTransport,
    family: NativeOutboundFamily = .auto, policy: RoutingRule.Policy = .direct,
    timeoutMilliseconds: UInt64 = 5000
  ) throws -> NativeOutboundOpened {
    let address = fakeIP.address.contains(":") ? "[\(fakeIP.address)]:\(port)" : "\(fakeIP.address):\(port)"
    let reply = try client.command([
      "operation": "outbound_connect_fake_ip", "address": address,
      "mapping_generation": fakeIP.generation, "transport": transport.rawValue,
      "family": family.rawValue, "policy": policy.rawValue, "timeout_ms": timeoutMilliseconds,
    ])
    guard let id = reply.outboundID,
          let peer = reply.outboundPeer else { throw NativeClientError.boundary }
    return NativeOutboundOpened(id: id, address: peer)
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
