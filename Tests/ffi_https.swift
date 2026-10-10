import Foundation

// 使用实际客户端门面与真实 Rust 库；不使用 Swift 网络 API 或模拟 FFI。
import libfluxa_core

@main
struct HttpsFFITests {
  static func main() throws {
    let file = CommandLine.arguments[1]
    guard let metadata = try JSONSerialization.jsonObject(
      with: Data(contentsOf: URL(fileURLWithPath: file))
    ) as? [String: Any], let port = metadata["port"] as? Int,
                             let root = metadata["certificate_pem"] as? String else {
      throw NativeClientError.boundary
    }
    guard let httpPort = metadata["http_port"] as? Int else { throw NativeClientError.boundary }
    guard let dnsPort = metadata["dns_port"] as? Int else { throw NativeClientError.boundary }
    let server = DNSServer(transport: .udp, address: "127.0.0.1:\(dnsPort)")
    let dns = DNSConfiguration(servers: [server])
    let routing = RoutingConfiguration(settings: RoutingSettings(mode: .direct))
    let url = "https://localhost:\(port)"
    let client = try NativeHttpsClient()
    expectRejected { _ = try client.execute(method: "GET", url: url) }
    _ = try client.configureNetwork(dns: dns, routing: routing)
    expectRejected { _ = try client.execute(method: "GET", url: url) }
    try client.addTrustedRoot(root)
    let response = try client.execute(
      method: "POST",
      url: url + "/echo",
      body: Data("Swift → Rust decrypted HTTPS".utf8)
    )
    precondition(response.status == 200)
    precondition(String(data: response.body, encoding: .utf8) == "Swift → Rust decrypted HTTPS")
    precondition(response.headers.contains { $0.name == "X-Fixture" })
    let plain = try client.executeRequest(
      method: "POST",
      url: "http://localhost:\(httpPort)/echo",
      body: Data("HTTP FFI".utf8)
    )
    precondition(plain.body == Data("HTTP FFI".utf8))
    expectRejected { _ = try client.execute(method: "GET", url: "http://localhost:\(httpPort)/") }
    let chunked = try client.execute(method: "GET", url: url + "/chunked")
    precondition(chunked.body == Data("hello world".utf8))
    let head = try client.execute(method: "HEAD", url: url + "/head")
    precondition(head.body.isEmpty)
    let redirect = try client.execute(method: "GET", url: url + "/redirect")
    precondition(redirect.status == 302)
    expectRejected { _ = try client.execute(method: "GET", url: "https://127.0.0.1:\(port)/") }
    expectRejected { _ = try client.execute(method: "GET", url: url + "/ambiguous") }
    expectRejected { _ = try client.execute(method: "GET", url: url + "/oversized") }
    expectRejected { _ = try client.execute(method: "GET", url: url + "/truncated") }
    expectRejected { _ = try client.execute(
      method: "GET",
      url: url,
      headers: [NativeHeader("X-Test", "ok\r\nHost: wrong")]
    ) }
    // 再次新建客户端不会继承其他句柄的自定义根证书。
    let other = try NativeHttpsClient()
    _ = try other.configureNetwork(dns: dns, routing: routing)
    expectRejected { _ = try other.execute(method: "GET", url: url) }
    _ = try client.configureNetwork(dns: dns, routing: RoutingConfiguration())
    expectRejected { _ = try client.execute(method: "GET", url: url) }
    _ = try client.configureNetwork(dns: dns, routing: routing)
    try checkCancellation(client, targets: [url + "/wait", "http://localhost:\(httpPort)/wait"])
    checkBoundary()
    print("Swift 客户端 FFI HTTPS：解密、信任隔离、错误域名、分帧与缓冲区边界全部通过")
  }

  static func checkCancellation(_ client: NativeHttpsClient, targets: [String]) throws {
    for target in targets {
      let finished = DispatchSemaphore(value: 0)
      DispatchQueue.global().async {
        do {
          _ = try client.executeRequest(method: "GET", url: target)
          preconditionFailure("取消后不能返回请求成功")
        } catch { finished.signal() }
      }
      var established = false
      for _ in 0 ..< 400 {
        if try client.outboundStatus().sessions.first?.uploaded ?? 0 > 0 {
          established = true
          break
        }
        Thread.sleep(forTimeInterval: 0.005)
      }
      precondition(established)
      try client.cancelConnections()
      precondition(finished.wait(timeout: .now() + 2) == .success)
      let status = try client.outboundStatus()
      precondition(status.sessions.isEmpty)
    }
  }

  static func checkBoundary() {
    let handle = fluxa_engine_create()
    defer { _ = fluxa_engine_destroy(handle) }
    precondition(fluxa_engine_command(handle, nil, 0) == 0)
    precondition(fluxa_engine_command(handle, nil, 12 * 1024 * 1024 + 1) == 0)
    let input = Data("{\"operation\":\"unknown\"}".utf8)
    let reply = input.withUnsafeBytes { fluxa_engine_command(
      handle,
      $0.bindMemory(to: UInt8.self).baseAddress,
      $0.count
    ) }
    defer { _ = fluxa_engine_destroy(reply) }
    precondition(reply != 0)
    let length = fluxa_engine_buffer_length(reply)
    var sentinel: UInt8 = 0xAA
    let needed = fluxa_engine_buffer_copy(reply, &sentinel, 1)
    precondition(needed == length && sentinel == 0xAA)
    precondition(fluxa_engine_buffer_length(UInt64.max) == 0)
  }

  static func expectRejected(_ action: () throws -> Void) {
    do { try action()
      preconditionFailure("应该拒绝的请求却成功了")
    } catch {}
  }
}
