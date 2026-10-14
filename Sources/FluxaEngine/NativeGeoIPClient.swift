import Foundation
#if canImport(FluxaCore)
import FluxaCore
#endif

public struct NativeGeoIPLocation: Decodable, Sendable {
  public let ip: String
  public let countryCode: String
  public let city: String
  public let region: String
  enum CodingKeys: String, CodingKey {
    case ip, city, region
    case countryCode = "country_code"
  }
}

/// 结构化 warn 使用内核整数代码；自然语言不作为类别判断依据。
public struct NativeGeoIPWarning: Decodable, Sendable {
  public let level: String
  public let code: UInt16
  public let params: NativeKernelErrorParams
}

/// 查询成功、未匹配和加载成功的机器状态，不作为执行异常抛出。
public struct NativeGeoIPStatus: Decodable, Sendable {
  public let state: String
  public let level: String
  public let code: UInt16
  public let params: NativeKernelErrorParams
}
public struct NativeGeoIPResult: Sendable {
  public let location: NativeGeoIPLocation?
  public let status: NativeGeoIPStatus
}

/// 来源由配置显式传入；无默认地址，下载、MMDB 校验和查询由 Rust 执行。
public final class NativeGeoIPClient: Sendable {
  private let client: NativeCommandClient
  /// 同模块测试复用真实命令句柄与注入根证书，不暴露新的业务权限入口。
  init(client: NativeCommandClient) { self.client = client }

  public init() throws {
    client = try NativeCommandClient(requiredCapabilities: 1_048_576)
  }

  public func configureDNS(_ dns: DNSConfiguration) throws {
    _ = try client.configureNetwork(
      dns: dns, routing: RoutingConfiguration(settings: RoutingSettings(mode: .direct)),
      limits: OutboundLimits()
    )
  }

  /// 固定免费 IP 回显服务；正文限额、TLS 与地址校验均由 Rust 执行。
  public func currentAddress(ipv6: Bool = false) throws -> String {
    let reply = try client.command(["operation": "public_ip", "ipv6": ipv6])
    guard let address = reply.address else { throw NativeClientError.boundary }
    return address
  }

  public func lastDNSQuery() throws -> NativeDNSObservation? {
    try client.command(["operation": "dns_last_query"]).dnsObservation
  }

  /// 从主配置读取 geoip_url 后原样传入；nil 关闭，返回带整数代码的 warn。
  public func configureSource(url: String?) throws -> NativeGeoIPWarning? {
    try client.command(["operation": "geoip_configure", "geoip_url": url as Any? ?? NSNull()]).warn
  }

  /// 显式更新配置来源；缺失返回 Disabled，下载或校验失败抛出结构化错误。
  public func refresh() throws {
    _ = try client.command(["operation": "geoip_refresh"])
  }

  /// 仅从配置来源打开数据库，不接受宿主文件或默认库。
  public func open() throws { try refresh() }

  public func lookup(address: String, language: String = "en") throws -> NativeGeoIPLocation? {
    try lookupResult(address: address, language: language).location
  }

  /// 合法未匹配返回 no-match/2412，不能与未配置或数据库故障混淆。
  public func lookupResult(address: String, language: String = "en") throws -> NativeGeoIPResult {
    let reply = try client.command(["operation": "geoip_lookup", "address": address, "language": language])
    guard let status = reply.geoipStatus else { throw NativeClientError.boundary }
    return NativeGeoIPResult(location: reply.geoip, status: status)
  }

  public func close() throws {
    _ = try client.command(["operation": "geoip_close"])
  }
}
