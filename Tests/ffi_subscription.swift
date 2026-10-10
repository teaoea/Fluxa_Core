// 验证真实 Swift → Rust UTC 计划与 HTTPS 下载，不在 Swift 中实现网络处理。
import Foundation

@main
struct SubscriptionFFITests {
  static func expect(_ expected: Bool, _ result: NativeSubscriptionScheduleDecision) {
    precondition(result.due == expected)
  }

  static func main() throws {
    let client = try NativeSubscriptionClient()
    let noon: UInt64 = 86400 + 43200
    expect(
      false,
      try client.evaluate(updateTime: "12:00", now: noon - 1, lastSuccess: noon - 86400)
    )
    expect(true, try client.evaluate(updateTime: "12:00", now: noon))
    expect(false, try client.evaluate(updateTime: "12:00", now: noon + 1, lastSuccess: noon))
    expect(false, try client.evaluate(updateTime: "12:00", now: noon + 1, lastAttempt: noon))
    expect(true, try client.evaluate(updateTime: "12:00", now: noon + 86400, lastSuccess: noon))
    expect(false, try client.evaluate(updateTime: nil, now: noon))
    expect(true, try client.evaluate(updateTime: nil, now: noon, manual: true))
    expect(true, try client.evaluate(updateTime: nil, now: noon, enabled: false, manual: true))
    expect(false, try client.evaluate(updateTime: "12:00", now: noon, enabled: false))
    let resumed = noon + 10 * 86400 - 3600
    expect(true, try client.evaluate(updateTime: "12:00", now: resumed, lastSuccess: noon))
    expect(false, try client.evaluate(updateTime: "12:00", now: resumed, lastAttempt: resumed))
    expect(true, try client.evaluate(updateTime: "12:00", now: resumed + 60, lastAttempt: resumed))
    expect(
      false,
      try client.evaluate(updateTime: "12:00", now: resumed + 61, lastSuccess: resumed + 60)
    )
    for invalid in ["24:00", "12:60", "1:00", "12：00", "", " 12:00"] {
      do {
        _ = try client.evaluate(updateTime: invalid, now: noon)
        preconditionFailure("Rust 必须拒绝非法时间")
      } catch {}
    }
    guard let metadata = try JSONSerialization.jsonObject(
      with: Data(contentsOf: URL(fileURLWithPath: CommandLine.arguments[1]))
    ) as? [String: Any], let port = metadata["port"] as? Int,
                             let dnsPort = metadata["dns_port"] as? Int,
                             let root = metadata["certificate_pem"] as? String
    else { throw NativeClientError.boundary }
    try client.configureNetwork(dns: DNSConfiguration(servers: [
      DNSServer(transport: .udp, address: "127.0.0.1:\(dnsPort)"),
    ]))
    try client.addTrustedRoot(root)
    let url = "https://localhost:\(port)"
    for path in ["/subscription", "/subscription-redirect"] {
      let content = try client.fetch(url: url + path)
      precondition(String(data: content, encoding: .utf8)?.contains("kind: rules") == true)
    }
    for path in [
      "/subscription-downgrade",
      "/subscription-loop",
      "/subscription-limit",
      "/subscription-invalid",
    ] {
      do {
        _ = try client.fetch(url: url + path)
        preconditionFailure("Rust 必须拒绝不安全或超限下载")
      } catch {}
    }
    print("订阅真实 Swift FFI：UTC、跨日、恢复补跑去重、手动、停用、TLS 下载及重定向/正文边界通过")
  }
}
