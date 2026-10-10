import Foundation
import Network

/// 来源和类型优先级由 Rust 执行；域名精确匹配与子域匹配必须显式区分。
public struct RoutingRule: Codable, Equatable, Sendable {
  public enum Match: String, Codable,
    Sendable { case domain, domainSuffix = "domain-suffix", ipCIDR = "ip-cidr"
    case destinationPort = "destination-port", transport, domainKeyword = "domain-keyword"
  }

  public enum Policy: String, Codable, Sendable { case proxy, direct, reject }
  public let type: Match
  public let value: String
  public let policy: Policy
  public let outbound: String?

  public init(type: Match, value: String, policy: Policy, outbound: String? = nil) throws {
    guard !value.isEmpty,
          value.utf8.count <= 253 else { throw CoreError.invalidRequest("规则值为空或过长") }
    if let outbound {
      guard policy == .proxy, !outbound.isEmpty, outbound.utf8.count <= 64,
            outbound.utf8.allSatisfy({ byte in
              (48 ... 57).contains(byte) || (65 ... 90).contains(byte)
                || (97 ... 122).contains(byte) || [45, 46, 95].contains(byte)
            }) else { throw CoreError.invalidRequest("出站标识无效") }
    }
    if type == .destinationPort {
      let parts = value.split(separator: "-", omittingEmptySubsequences: false)
      guard (1 ... 2).contains(parts.count),
            let first = UInt16(parts[0]), first > 0,
            let last = UInt16(parts.last ?? ""), last >= first
      else { throw CoreError.invalidRequest("目标端口规则无效") }
    } else if type == .transport {
      guard ["tcp", "udp"].contains(value) else { throw CoreError.invalidRequest("传输类型规则无效") }
    } else if type == .domainKeyword {
      guard value.utf8.allSatisfy({ byte in
        (48 ... 57).contains(byte) || (65 ... 90).contains(byte)
          || (97 ... 122).contains(byte) || [45, 46].contains(byte)
      }) else { throw CoreError.invalidRequest("域名关键词无效") }
    } else if type == .ipCIDR {
      let parts = value.split(separator: "/")
      guard parts.count == 2, let prefix = Int(parts[1]),
            (IPv4Address(String(parts[0])) != nil && (0 ... 32).contains(prefix))
            || (IPv6Address(String(parts[0])) != nil && (0 ... 128).contains(prefix))
      else {
        throw CoreError.invalidRequest("IP-CIDR 规则无效")
      }
    } else {
      guard (value.hasSuffix(".") ? String(value.dropLast()) : value).split(
        separator: ".",
        omittingEmptySubsequences: false
      ).allSatisfy({ label in
        !label.isEmpty && label.count <= 63 && label.first != "-" && label.last != "-"
          && label.allSatisfy { $0.isASCII && ($0.isLetter || $0.isNumber || $0 == "-") }
      }) else { throw CoreError.invalidRequest("域名规则无效") }
    }
    self.type = type
    self.value = value.lowercased()
    self.policy = policy
    self.outbound = outbound
  }
}
