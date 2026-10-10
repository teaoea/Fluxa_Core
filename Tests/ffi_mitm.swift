// 实际 Swift 客户端 FFI 验证；所有 TLS 和网络 I/O 均在 Rust 内执行。
import Foundation

private struct Fixture: Decodable {
  let dnsPort: UInt16
  let port: UInt16
  let certificatePEM: String
  enum CodingKeys: String, CodingKey {
    case dnsPort = "dns_port"
    case port
    case certificatePEM = "certificate_pem"
  }

  var dns: DNSConfiguration {
    DNSConfiguration(servers: [DNSServer(transport: .udp, address: "127.0.0.1:\(dnsPort)")])
  }

  var routing: RoutingConfiguration {
    RoutingConfiguration(settings: RoutingSettings(mode: .direct))
  }

  var url: String {
    "https://localhost:\(port)"
  }
}

@main
struct MitmFFITests {
  static func main() throws {
    let data = try Data(contentsOf: URL(fileURLWithPath: CommandLine.arguments[1]))
    let fixture = try JSONDecoder().decode(Fixture.self, from: data)
    try trafficScript(fixture)
    try interception(fixture)
    try passthrough(fixture)
    try untrustedUpstream(fixture)
    try captureDisabled(fixture)
    print("Swift 客户端 FFI MITM：启动、双 TLS、日志、透传、CA 恢复和停止全部通过")
  }

  private static func trafficScript(_ fixture: Fixture) throws {
    let proxy = try NativeMitmClient()
    _ = try proxy.configureNetwork(dns: fixture.dns, routing: fixture.routing)
    try proxy.addTrustedUpstreamRoot(fixture.certificatePEM)
    let root = try proxy.rootCertificate()
    let port = try proxy.start(hosts: ["localhost"], captureBodies: true, rewrites: [
      NativeRewriteRule(
        host: "localhost",
        pathPrefix: "/json",
        action: "response-script",
        script: "remove-json-keys",
        jsonKeys: ["ad"]
      ),
    ])
    defer { try? proxy.stop() }
    let client = try NativeHttpsClient()
    _ = try client.configureNetwork(dns: fixture.dns, routing: fixture.routing)
    try client.addTrustedRoot(root)
    let response = try client.execute(method: "GET", url: fixture.url + "/json", proxyPort: port)
    guard let body = try JSONSerialization.jsonObject(with: response.body) as? [String: Any],
          let text = String(data: response.body, encoding: .utf8)
    else { throw NativeClientError.boundary }
    precondition(body["ad"] == nil && body["title"] as? String == "keep")
    precondition(text.contains("18446744073709551615"))
    precondition(!response.headers.contains { $0.name.lowercased() == "etag" })
    let record = try waitForCapture(proxy)
    precondition(record.intercepted && record.responseBody == response.body && record.error == nil)
  }

  private static func interception(_ fixture: Fixture) throws {
    let proxy = try NativeMitmClient()
    _ = try proxy.configureNetwork(dns: fixture.dns, routing: fixture.routing)
    let root = try proxy.rootCertificate()
    let identity = try proxy.exportIdentityForSecureStorage()
    precondition(identity.certificatePEM == root)
    let restored = try NativeMitmClient()
    try restored.restoreIdentity(identity)
    let restoredRoot = try restored.rootCertificate()
    precondition(restoredRoot == root)
    let other = try NativeMitmClient().exportIdentityForSecureStorage()
    expectRejected { try restored.restoreIdentity(NativeMitmIdentity(
      certificatePEM: root, privateKeyPEM: other.privateKeyPEM
    )) }
    try proxy.addTrustedUpstreamRoot(fixture.certificatePEM)
    let port = try proxy.start(hosts: ["localhost"], captureBodies: true)
    defer { try? proxy.stop() }
    expectRejected { _ = try proxy.start(hosts: ["localhost"]) }
    expectRejected { try proxy.restoreIdentity(identity) }
    let client = try NativeHttpsClient()
    _ = try client.configureNetwork(dns: fixture.dns, routing: fixture.routing)
    try client.addTrustedRoot(root)
    let response = try client.execute(
      method: "POST", url: fixture.url + "/echo", body: Data("FFI MITM payload".utf8),
      proxyPort: port
    )
    precondition(response.status == 200 && response.body == Data("FFI MITM payload".utf8))
    let record = try waitForCapture(proxy)
    precondition(record.intercepted && record.status == 200 && record.method == "POST")
    precondition(record.requestBody == response.body && record.responseBody == response.body)
    precondition(record.error == nil)
    expectRejected { try proxy.acknowledgeCapture(record.id + 1) }
    let retained = try proxy.peekCapture()
    precondition(retained.capture?.id == record.id && retained.dropped == 0)
    try proxy.acknowledgeCapture(record.id)
    let empty = try proxy.peekCapture()
    precondition(empty.capture == nil)
    let untrustedClient = try NativeHttpsClient()
    _ = try untrustedClient.configureNetwork(dns: fixture.dns, routing: fixture.routing)
    try untrustedClient.addTrustedRoot(fixture.certificatePEM)
    expectRejected { _ = try untrustedClient.execute(
      method: "GET",
      url: fixture.url,
      proxyPort: port
    ) }
    try proxy.stop()
    try proxy.stop()
    expectRejected { _ = try client.execute(method: "GET", url: fixture.url, proxyPort: port) }
  }

  private static func passthrough(_ fixture: Fixture) throws {
    let proxy = try NativeMitmClient()
    _ = try proxy.configureNetwork(dns: fixture.dns, routing: fixture.routing)
    let port = try proxy.start(hosts: ["sub.localhost"], captureBodies: true)
    defer { try? proxy.stop() }
    let client = try NativeHttpsClient()
    _ = try client.configureNetwork(dns: fixture.dns, routing: fixture.routing)
    try client.addTrustedRoot(fixture.certificatePEM)
    let response = try client.execute(method: "GET", url: fixture.url, proxyPort: port)
    precondition(response.status == 200)
    let record = try waitForCapture(proxy)
    precondition(!record.intercepted && record.requestBody.isEmpty && record.responseBody.isEmpty)
    try proxy.acknowledgeCapture(record.id)
  }

  private static func untrustedUpstream(_ fixture: Fixture) throws {
    let proxy = try NativeMitmClient()
    _ = try proxy.configureNetwork(dns: fixture.dns, routing: fixture.routing)
    expectRejected { _ = try proxy.start(hosts: ["*.localhost"]) }
    let root = try proxy.rootCertificate()
    let port = try proxy.start(hosts: ["localhost"], captureBodies: true)
    defer { try? proxy.stop() }
    let client = try NativeHttpsClient()
    _ = try client.configureNetwork(dns: fixture.dns, routing: fixture.routing)
    try client.addTrustedRoot(root)
    expectRejected { _ = try client.execute(method: "GET", url: fixture.url, proxyPort: port) }
    let record = try waitForCapture(proxy)
    precondition(record.intercepted && record.error != nil && record.status == 502)
    precondition(record.responseBodyLength > 0 && !record.responseBody.isEmpty)
  }

  private static func captureDisabled(_ fixture: Fixture) throws {
    let proxy = try NativeMitmClient()
    _ = try proxy.configureNetwork(dns: fixture.dns, routing: fixture.routing)
    try proxy.addTrustedUpstreamRoot(fixture.certificatePEM)
    let root = try proxy.rootCertificate()
    let port = try proxy.start(hosts: ["localhost"])
    defer { try? proxy.stop() }
    let client = try NativeHttpsClient()
    _ = try client.configureNetwork(dns: fixture.dns, routing: fixture.routing)
    try client.addTrustedRoot(root)
    _ = try client.execute(
      method: "POST",
      url: fixture.url,
      body: Data("secret".utf8),
      proxyPort: port
    )
    let record = try waitForCapture(proxy)
    precondition(record.requestBody.isEmpty && record.responseBody.isEmpty)
    precondition(record.requestBodyLength == 6 && record.responseBodyLength == 6)
  }

  private static func waitForCapture(_ client: NativeMitmClient) throws -> NativeCapture {
    for _ in 0 ..< 200 {
      if let record = try client.peekCapture().capture {
        return record
      }
      Thread.sleep(forTimeInterval: 0.005)
    }
    throw NativeClientError.boundary
  }

  private static func expectRejected(_ action: () throws -> Void) {
    do { try action()
      preconditionFailure("应该拒绝的操作却成功了")
    } catch {}
  }
}
