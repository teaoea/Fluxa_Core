import Foundation

@main
struct GeoIPFFICheck {
  static func expect(_ code: UInt16, _ action: () throws -> Void) throws {
    do { try action(); fatalError("必须返回指定内核错误") }
    catch NativeClientError.kernel(let error) { precondition(error.code == code, "实际 \(error.code)，预期 \(code)") }
  }
  static func main() throws {
    let metadata = try JSONSerialization.jsonObject(with: Data(contentsOf: URL(fileURLWithPath: CommandLine.arguments[2]))) as! [String: Any]
    let port = metadata["port"] as! Int
    let dnsPort = metadata["dns_port"] as! Int
    let raw = try NativeCommandClient(requiredCapabilities: 1_048_576)
    let client = NativeGeoIPClient(client: raw)
    let warning = try client.configureSource(url: nil)
    precondition(warning?.level == "warn" && warning?.code == 2410)
    let disabled = try client.lookupResult(address: "unconfigured.example")
    precondition(disabled.status.state == "disabled" && disabled.status.code == 2410 && disabled.location == nil)
    try client.refresh() // 未配置不初始化 HTTPS，不应因缺少 DNS 失败。
    _ = try raw.configureNetwork(dns: DNSConfiguration(servers: [DNSServer(transport: .udp, address: "127.0.0.1:\(dnsPort)")]), routing: RoutingConfiguration(settings: RoutingSettings(mode: .direct)), limits: OutboundLimits())
    _ = try raw.command(["operation": "add_trusted_root", "certificate_pem": metadata["certificate_pem"] as! String])
    _ = try client.configureSource(url: "https://localhost:\(port)/geoip")
    try expect(2408) { _ = try client.lookup(address: "8.8.8.8") }
    try expect(2401) { _ = try client.configureSource(url: "") }
    try expect(2402) { _ = try client.configureSource(url: "http://localhost/geoip") }
    // 无效更新不得覆盖原来有效 URL；下一次更新仍从 /geoip 成功。
    try client.refresh()
    let ipv4 = try client.lookupResult(address: "8.8.8.8", language: "zh-CN")
    precondition(ipv4.location?.countryCode == "US" && ipv4.status.code == 2411)
    let ipv6 = try client.lookup(address: "2001:4860:4860::8888")
    precondition(ipv6?.countryCode == "TW")
    let missing = try client.lookupResult(address: "192.0.2.1")
    precondition(missing.location == nil && missing.status.state == "no-match" && missing.status.code == 2412)
    for (path, code): (String, UInt16) in [("/geoip-corrupt", 2405), ("/geoip-truncated", 2406), ("/geoip-limit", 2407), ("/subscription-error", 2403)] {
      _ = try client.configureSource(url: "https://localhost:\(port)\(path)")
      try expect(code) { try client.refresh() }
      try expect(2408) { _ = try client.lookup(address: "8.8.8.8") }
    }
    _ = try client.configureSource(url: "https://localhost:\(port)/geoip")
    try client.open()
    try expect(1001) { _ = try raw.command(["operation": "geoip_open", "path": "/tmp/untrusted.mmdb"]) }
    try expect(1001) { _ = try client.lookup(address: "bad..address") }
    try client.close()
    try expect(2408) { _ = try client.lookup(address: "8.8.8.8") }
    try client.open()
    _ = try client.configureSource(url: nil)
    let finalDisabled = try client.lookupResult(address: "8.8.8.8")
    precondition(finalDisabled.status.state == "disabled")
    print("GeoIP 实际 Swift FFI 通过：配置 URL、真实 HTTPS 下载、Disabled/Ready/no-match、2401–2408 与失败保留")
  }
}
