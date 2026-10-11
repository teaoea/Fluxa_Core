import Foundation

/// 手动真实节点验收；文件应由调用者保存在私有临时目录，不输出或保存凭据。
@main
struct ProtocolRequestFFICheck {
  static func main() throws {
    let arguments = CommandLine.arguments
    guard arguments.count == 3 else { throw NativeClientError.boundary }
    let file = URL(fileURLWithPath: arguments[1])
    let size = try file.resourceValues(forKeys: [.fileSizeKey]).fileSize ?? 0
    guard size > 0, size <= 8192 else { throw NativeClientError.boundary }
    let profile = try JSONDecoder().decode(NativeProxyOutbound.self, from: Data(contentsOf: file))
    let client = try NativeHttpsClient()
    let server = DNSServer(
      transport: .doh, url: "https://cloudflare-dns.com/dns-query", bootstrap: "1.1.1.1:443"
    )
    let dns = DNSConfiguration(servers: [server])
    let routing = RoutingConfiguration(settings: RoutingSettings(
      mode: .global,
      outbound: profile.id
    ))
    try client.configureNetwork(dns: dns, routing: routing, outbounds: [profile])
    let result = try client.execute(method: "GET", url: arguments[2])
    guard (200 ..< 400).contains(result.status) else { throw NativeClientError.boundary }
    try client.cancelConnections()
    print("真实节点 Swift FFI：HTTPS 状态 \(result.status)，正文 \(result.body.count) 字节，连接释放通过。")
  }
}
