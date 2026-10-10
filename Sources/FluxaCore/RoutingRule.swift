import Foundation
import Network

/// 规则按顺序匹配；域名精确匹配与子域匹配必须显式区分。
public struct RoutingRule: Codable, Equatable, Sendable {
  public enum Match: String, Codable,
    Sendable { case domain, domainSuffix = "domain-suffix", ipCIDR = "ip-cidr" }
  public enum Policy: String, Codable, Sendable { case proxy, direct, reject }
  public let type: Match
  public let value: String
  public let policy: Policy

  public init(type: Match, value: String, policy: Policy) throws {
    guard !value.isEmpty,
          value.utf8.count <= 253 else { throw CoreError.invalidRequest("规则值为空或过长") }
    if type == .ipCIDR {
      let parts = value.split(separator: "/")
      guard parts.count == 2, let prefix = Int(parts[1]),
            (IPv4Address(String(parts[0])) != nil && (0 ... 32).contains(prefix))
            || (IPv6Address(String(parts[0])) != nil && (0 ... 128).contains(prefix))
      else {
        throw CoreError.invalidRequest("IP-CIDR 规则无效")
      }
    } else {
      guard value.split(separator: ".", omittingEmptySubsequences: false).allSatisfy({ label in
        !label.isEmpty && label.count <= 63 && label.first != "-" && label.last != "-"
          && label.allSatisfy { $0.isASCII && ($0.isLetter || $0.isNumber || $0 == "-") }
      }) else { throw CoreError.invalidRequest("域名规则无效") }
    }
    self.type = type
    self.value = value.lowercased()
    self.policy = policy
  }
}
