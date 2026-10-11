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

/// MMDB 读取、查询和资源限额均由 Rust 执行；数据库只驻留于本句柄。
public final class NativeGeoIPClient: Sendable {
  private let client: NativeCommandClient
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

  public func open(database: URL) throws {
    _ = try client.command(["operation": "geoip_open", "path": database.path])
  }

  public func lookup(address: String, language: String = "en") throws -> NativeGeoIPLocation? {
    try client.command(["operation": "geoip_lookup", "address": address, "language": language])
      .geoip
  }

  public func close() throws {
    _ = try client.command(["operation": "geoip_close"])
  }
}
