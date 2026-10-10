import Foundation

struct DNSFFIFixture: Decodable {
  let dnsPort: Int
  let dotPort: Int
  let dohPort: Int
  let certificatePEM: String
  enum CodingKeys: String, CodingKey {
    case dnsPort = "dns_port"
    case dotPort = "dot_port"
    case dohPort = "doh_port"
    case certificatePEM = "dns_certificate_pem"
  }
}

@main
struct DNSFFICheck {
  static func main() throws {
    let fixture = try JSONDecoder().decode(
      DNSFFIFixture.self,
      from: Data(contentsOf: URL(fileURLWithPath: CommandLine.arguments[1]))
    )
    let port = fixture.dnsPort
    let dotPort = fixture.dotPort
    let dohPort = fixture.dohPort
    let certificate = fixture.certificatePEM
    let servers = [
      DNSServer(transport: .udp, address: "127.0.0.1:\(port)"),
      DNSServer(transport: .tcp, address: "127.0.0.1:\(port)"),
      DNSServer(transport: .dot, address: "127.0.0.1:\(dotPort)", serverName: "localhost"),
      DNSServer(
        transport: .doh,
        url: "https://localhost:\(dohPort)/dns-query",
        bootstrap: "127.0.0.1:\(dohPort)"
      ),
    ]
    for server in servers {
      let client = try NativeDNSClient()
      try client.addTrustedRoot(certificatePEM: certificate)
      let config = DNSConfiguration(servers: [server], timeoutMilliseconds: 1000)
      try client.validate(config)
      try client.configure(config)
      let first = try client.query(name: "alias.example.test")
      precondition(first.addresses == ["192.0.2.44"] && first.records
        .contains(where: { $0.recordType == "CNAME" }))
      let cached = try client.query(name: "alias.example.test")
      precondition(cached.fromCache)
      let ipv6 = try client.query(name: "example.test", type: "AAAA")
      precondition(ipv6.addresses == ["2001:db8::44"])
      let negative = try client.query(name: "nxdomain.example.test")
      precondition(negative.rcode == 3)
      let cachedNegative = try client.query(name: "nxdomain.example.test")
      precondition(cachedNegative.fromCache)
      try client.clearCache()
      let fresh = try client.query(name: "alias.example.test")
      precondition(!fresh.fromCache)
      try client.close()
      do {
        _ = try client.query(name: "example.test")
        preconditionFailure("关闭后不能查询")
      } catch NativeClientError.rejected {}
    }
    let validator = try NativeDNSClient()
    try validator.configure(DNSConfiguration(
      servers: [servers[0]],
      timeoutMilliseconds: 1000,
      dnssec: true
    ))
    do {
      let forged = try validator.query(name: "ad-only.example.test")
      precondition(forged.validation != "secure", "不能相信未验证的 AD 标记")
    } catch NativeClientError.rejected {}
    try validator.close()
    for _ in 0 ..< 100 {
      let client = try NativeDNSClient()
      try client.configure(DNSConfiguration(servers: [servers[0]]))
      try client.close()
    }
    print("真实 Swift DNS FFI 通过：UDP/TCP/DoT/HTTP2 DoH、CNAME、A/AAAA、正负缓存、关闭及 100 次释放。")
  }
}
