import Foundation

/// 共享配置值类型；YAML 使用固定字段名，网络语义由 Rust 校验。
public struct DNSConfiguration: Codable, Equatable, Sendable {
  public var servers: [DNSServer]
  public var timeoutMilliseconds: UInt64
  public var dnssec: Bool
  public var fakeIP: FakeIPConfiguration?
  public var service: DNSServiceConfiguration?
  public init(
    servers: [DNSServer],
    timeoutMilliseconds: UInt64 = 5000,
    dnssec: Bool = false,
    fakeIP: FakeIPConfiguration? = nil,
    service: DNSServiceConfiguration? = nil
  ) {
    self.servers = servers
    self.timeoutMilliseconds = timeoutMilliseconds
    self.dnssec = dnssec
    self.fakeIP = fakeIP
    self.service = service
  }

  public init(from decoder: Decoder) throws {
    let container = try decoder.container(keyedBy: CodingKeys.self)
    servers = try container.decode([DNSServer].self, forKey: .servers)
    timeoutMilliseconds = try container
      .decodeIfPresent(UInt64.self, forKey: .timeoutMilliseconds) ?? 5000
    fakeIP = try container.decodeIfPresent(FakeIPConfiguration.self, forKey: .fakeIP)
    service = try container.decodeIfPresent(DNSServiceConfiguration.self, forKey: .service)
    dnssec = try container.decodeIfPresent(Bool.self, forKey: .dnssec) ?? false
  }

  enum CodingKeys: String, CodingKey {
    case servers, dnssec, service
    case fakeIP = "fake-ip"
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

/// 合成地址池值类型；范围、容量和地址映射全部由 Rust 处理。
public struct FakeIPConfiguration: Codable, Equatable, Sendable {
  public var ipv4Range: String
  public var ipv6Range: String
  public var capacity: UInt32
  public var exclude: [String]
  public init(
    ipv4Range: String = "198.18.0.0/16",
    ipv6Range: String = "fd00:198:18::/64",
    capacity: UInt32 = 4096,
    exclude: [String] = []
  ) {
    self.ipv4Range = ipv4Range
    self.ipv6Range = ipv6Range
    self.capacity = capacity
    self.exclude = exclude
  }

  public init(from decoder: Decoder) throws {
    let container = try decoder.container(keyedBy: CodingKeys.self)
    ipv4Range = try container.decodeIfPresent(String.self, forKey: .ipv4Range) ?? "198.18.0.0/16"
    ipv6Range = try container.decodeIfPresent(String.self, forKey: .ipv6Range) ?? "fd00:198:18::/64"
    capacity = try container.decodeIfPresent(UInt32.self, forKey: .capacity) ?? 4096
    exclude = try container.decodeIfPresent([String].self, forKey: .exclude) ?? []
  }

  enum CodingKeys: String, CodingKey {
    case ipv4Range = "ipv4-range"
    case ipv6Range = "ipv6-range"
    case capacity, exclude
  }
}

/// 监听和访问范围由 Rust 校验；默认仅允许本机，显式启动后才绑定端口。
public struct DNSServiceConfiguration: Codable, Equatable, Sendable {
  public var listen: String
  public var allowLAN: Bool
  public var allowedClients: [String]
  public init(
    listen: String = "127.0.0.1:0",
    allowLAN: Bool = false,
    allowedClients: [String] = []
  ) {
    self.listen = listen
    self.allowLAN = allowLAN
    self.allowedClients = allowedClients
  }

  public init(from decoder: Decoder) throws {
    let container = try decoder.container(keyedBy: CodingKeys.self)
    listen = try container.decodeIfPresent(String.self, forKey: .listen) ?? "127.0.0.1:0"
    allowLAN = try container.decodeIfPresent(Bool.self, forKey: .allowLAN) ?? false
    allowedClients = try container.decodeIfPresent([String].self, forKey: .allowedClients) ?? []
  }

  enum CodingKeys: String, CodingKey {
    case listen
    case allowLAN = "allow-lan"
    case allowedClients = "allowed-clients"
  }
}
