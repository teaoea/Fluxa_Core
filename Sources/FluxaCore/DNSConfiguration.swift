import Foundation

/// 共享配置值类型；YAML 使用固定字段名，网络语义由 Rust 校验。
public struct DNSConfiguration: Codable, Equatable, Sendable {
  public var servers: [DNSServer]
  public var timeoutMilliseconds: UInt64
  public var dnssec: Bool
  public init(servers: [DNSServer], timeoutMilliseconds: UInt64 = 5000, dnssec: Bool = false) {
    self.servers = servers
    self.timeoutMilliseconds = timeoutMilliseconds
    self.dnssec = dnssec
  }

  public init(from decoder: Decoder) throws {
    let container = try decoder.container(keyedBy: CodingKeys.self)
    servers = try container.decode([DNSServer].self, forKey: .servers)
    timeoutMilliseconds = try container
      .decodeIfPresent(UInt64.self, forKey: .timeoutMilliseconds) ?? 5000
    dnssec = try container.decodeIfPresent(Bool.self, forKey: .dnssec) ?? false
  }

  enum CodingKeys: String, CodingKey {
    case servers, dnssec
    case timeoutMilliseconds = "timeout-ms"
  }
}

public struct DNSServer: Codable, Equatable, Sendable {
  public enum Transport: String, Codable, CaseIterable, Sendable { case udp, tcp, dot, doh }
  public var transport: Transport
  public var address: String?
  public var serverName: String?
  public var url: String?
  public var bootstrap: String?
  public init(
    transport: Transport, address: String? = nil, serverName: String? = nil,
    url: String? = nil, bootstrap: String? = nil
  ) {
    self.transport = transport
    self.address = address
    self.serverName = serverName
    self.url = url
    self.bootstrap = bootstrap
  }

  enum CodingKeys: String, CodingKey {
    case transport, address, url, bootstrap
    case serverName = "server-name"
  }
}
