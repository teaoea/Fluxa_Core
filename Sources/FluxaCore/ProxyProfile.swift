import Foundation

/// 协议标识保持稳定，新增内核或协议不会改变既有配置含义。
public enum ProxyProtocol: String, Codable, CaseIterable, Sendable {
  case vlessReality = "VLESS + REALITY"
  case shadowsocks = "Shadowsocks"
  case trojan = "Trojan"
  case vmess = "VMess"
  case hysteria2 = "Hysteria 2"
  case tuic = "TUIC"
  case wireGuard = "WireGuard"
  case socks5 = "SOCKS5"
  case http = "HTTP / HTTPS"

  public var isImplemented: Bool {
    false
  }
}

/// 已校验的 VLESS TCP REALITY 节点，凭据持久化由 App 负责。
public struct RealityProfile: Codable, Identifiable, Equatable, Sendable {
  public var id = UUID()
  public var name: String
  public let host: String
  public let port: UInt16
  public let userID: UUID
  public let serverName: String
  public let publicKey: String
  public let shortID: String
  public let fingerprint: String
  public let flow: String

  public static func parse(_ link: String) throws -> RealityProfile {
    guard let components = URLComponents(string: link
      .trimmingCharacters(in: .whitespacesAndNewlines)),
      components.scheme?.lowercased() == "vless",
      let host = components.host, !host.isEmpty,
      let user = components.user, let userID = UUID(uuidString: user),
      components.password == nil,
      let port = components.port, (1 ... 65535).contains(port),
      components.path.isEmpty || components.path == "/"
    else { throw CoreError.invalidRequest("VLESS 链接必须包含 UUID、服务器和端口。") }
    var parameters: [String: String] = [:]
    for item in components.queryItems ?? [] {
      guard parameters[item.name] == nil else {
        throw CoreError.invalidRequest("链接包含重复参数。")
      }
      parameters[item.name] = item.value ?? ""
    }
    let supported = Set(["security", "type", "encryption", "flow", "sni", "pbk", "sid", "fp"])
    guard Set(parameters.keys).isSubset(of: supported) else {
      throw CoreError.unsupported("链接包含本版本尚未支持的参数，请勿静默丢弃传输设置。")
    }
    guard parameters["security"] == "reality",
          ["tcp", "raw"].contains(parameters["type"] ?? "tcp"),
          parameters["encryption"] == nil || parameters["encryption"] == "none"
    else { throw CoreError.unsupported("第一版支持 VLESS REALITY TCP/RAW，encryption=none。") }
    let flow = parameters["flow"] ?? ""
    guard flow == "xtls-rprx-vision" else {
      throw CoreError.unsupported("首个协议组合要求 flow=xtls-rprx-vision。")
    }
    let key = parameters["pbk"] ?? ""
    let base64 = key.replacingOccurrences(of: "-", with: "+").replacingOccurrences(
      of: "_",
      with: "/"
    )
    let padded = base64 + String(repeating: "=", count: (4 - base64.count % 4) % 4)
    guard key.count == 43,
          key.allSatisfy({ $0.isASCII && ($0.isLetter || $0.isNumber || $0 == "-" || $0 == "_") }),
          Data(base64Encoded: padded)?.count == 32
    else { throw CoreError.invalidRequest("REALITY 公钥必须是 32 字节的 Base64URL 值。") }
    let shortID = parameters["sid"] ?? ""
    guard shortID.count <= 16, shortID.count.isMultiple(of: 2),
          shortID.allSatisfy({ $0.isASCII && $0.isHexDigit })
    else { throw CoreError.invalidRequest("Short ID 必须是最多 16 位、长度为偶数的十六进制字符串。") }
    let serverName = parameters["sni"] ?? ""
    guard !serverName.isEmpty, serverName.utf8.count <= 253,
          serverName.split(separator: ".", omittingEmptySubsequences: false).allSatisfy({ label in
            !label.isEmpty && label.count <= 63 && label.first != "-" && label.last != "-"
              && label.allSatisfy { $0.isASCII && ($0.isLetter || $0.isNumber || $0 == "-") }
          })
    else { throw CoreError.invalidRequest("请填写有效的 REALITY SNI 域名。") }
    let fingerprint = parameters["fp"] ?? "chrome"
    guard ["chrome", "firefox", "safari", "ios", "android", "edge", "random", "randomized"]
      .contains(fingerprint)
    else {
      throw CoreError.invalidRequest("不支持此 TLS fingerprint。")
    }
    return RealityProfile(
      name: components.fragment.flatMap { $0.isEmpty ? nil : $0 } ?? host,
      host: host.trimmingCharacters(in: CharacterSet(charactersIn: "[]")),
      port: UInt16(port),
      userID: userID,
      serverName: serverName,
      publicKey: key,
      shortID: shortID.lowercased(),
      fingerprint: fingerprint,
      flow: flow
    )
  }
}
