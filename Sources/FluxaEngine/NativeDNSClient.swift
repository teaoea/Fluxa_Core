import Foundation
#if canImport(FluxaCore)
import FluxaCore
#endif

public struct NativeDNSRecord: Decodable, Sendable {
  public let name: String
  public let recordType: String
  public let ttl: UInt32
  public let data: String
  enum CodingKeys: String, CodingKey { case name, ttl, data
    case recordType = "record_type"
  }
}

public struct NativeDNSAnswer: Decodable, Sendable {
  public let name: String
  public let queryType: String
  public let records: [NativeDNSRecord]
  public let addresses: [String]
  public let ttl: UInt32
  public let rcode: UInt16
  public let validation: String
  public let fromCache: Bool
  public let durationMilliseconds: UInt64
  enum CodingKeys: String, CodingKey {
    case name, records, addresses, ttl, rcode, validation
    case queryType = "query_type"
    case fromCache = "from_cache"
    case durationMilliseconds = "duration_ms"
  }
}

/// Swift 只传配置和问题，DNS 网络处理、缓存及签名验证全部由 Rust 完成。
public final class NativeDNSClient: Sendable {
  private let client: NativeCommandClient
  public init() throws {
    client = try NativeCommandClient(requiredCapabilities: 64)
  }

  public func configure(_ configuration: DNSConfiguration) throws {
    var command = try Self.configurationCommand(configuration)
    command["operation"] = "dns_configure"
    _ = try client.command(command)
  }

  public func validate(_ configuration: DNSConfiguration) throws {
    var command = try Self.configurationCommand(configuration)
    command["operation"] = "dns_validate"
    _ = try client.command(command)
  }

  public func query(name: String, type: String = "A") throws -> NativeDNSAnswer {
    let reply = try client.command(["operation": "dns_query", "name": name, "query_type": type])
    guard let answer = reply.dnsAnswer else { throw NativeClientError.boundary }
    return answer
  }

  public func reverseFakeIP(address: String) throws -> String? {
    try client.command(["operation": "dns_fake_ip_reverse", "address": address]).fakeIPName
  }

  /// 重置后旧地址不再可反查；同一实例中不会把它分配给其他域名。
  public func resetFakeIP() throws {
    _ = try client.command(["operation": "dns_fake_ip_reset"])
  }

  public func clearCache() throws {
    _ = try client.command(["operation": "dns_clear_cache"])
  }

  public func close() throws {
    _ = try client.command(["operation": "dns_close"])
  }

  /// 仅用于显式企业/测试信任；Rust 仍校验证书域名、有效期和签名。
  public func addTrustedRoot(certificatePEM: String) throws {
    _ = try client.command(["operation": "add_trusted_root", "certificate_pem": certificatePEM])
  }

  private static func configurationCommand(_ configuration: DNSConfiguration) throws
    -> [String: Any] {
    let data = try JSONEncoder().encode(configuration)
    guard var object = try JSONSerialization.jsonObject(with: data) as? [String: Any],
          var servers = object["servers"] as? [[String: Any]]
    else { throw NativeClientError.boundary }
    object["timeout_ms"] = object.removeValue(forKey: "timeout-ms")
    for index in servers.indices {
      if let name = servers[index]
        .removeValue(forKey: "server-name") {
        servers[index]["server_name"] = name
      }
    }
    if var fakeIP = object.removeValue(forKey: "fake-ip") as? [String: Any] {
      fakeIP["ipv4_range"] = fakeIP.removeValue(forKey: "ipv4-range")
      fakeIP["ipv6_range"] = fakeIP.removeValue(forKey: "ipv6-range")
      object["fake_ip"] = fakeIP
    }
    object["servers"] = servers
    return object
  }
}
