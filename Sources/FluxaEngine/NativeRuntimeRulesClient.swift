import Foundation
#if canImport(FluxaCore)
import FluxaCore
#endif

/// 原始规则来源值；Swift 不执行合并、裁剪或索引匹配。
public struct NativeRuleSource: Encodable, Sendable {
  public enum Layer: String, Encodable, Sendable { case local, remote }
  public var id: String
  public var layer: Layer
  public var enabled: Bool
  public var rules: [RoutingRule]
  public init(id: String, layer: Layer, enabled: Bool = true, rules: [RoutingRule]) {
    self.id = id
    self.layer = layer
    self.enabled = enabled
    self.rules = rules
  }
}

public struct NativeRuntimeRulesSummary: Decodable, Sendable {
  public let snapshotHandle: UInt64
  public let version: UInt64
  public let original: Int
  public let duplicates: Int
  public let conflicts: Int
  public let pruned: Int
  public let effective: Int
  public let directorySynced: Bool?
  enum CodingKeys: String, CodingKey {
    case snapshotHandle = "snapshot_handle"
    case version, original, duplicates, conflicts, pruned, effective
    case directorySynced = "directory_synced"
  }
}

/// 生成文件和内核快照由 Rust 原子提交；客户端只传递原始输入与显示统计。
public final class NativeRuntimeRulesClient: Sendable {
  private let client: NativeCommandClient
  public init(directory: URL) throws {
    guard directory.isFileURL else { throw NativeClientError.boundary }
    client = try NativeCommandClient(requiredCapabilities: 65536)
    _ = try client.command(["operation": "runtime_rules_open", "directory": directory.path])
  }

  public func update(
    settings: RoutingSettings, sources: [NativeRuleSource]
  ) throws -> NativeRuntimeRulesSummary {
    let reply = try client.command([
      "operation": "runtime_rules_update",
      "settings": JSONSerialization.jsonObject(with: JSONEncoder().encode(settings)),
      "sources": JSONSerialization.jsonObject(with: JSONEncoder().encode(sources)),
    ])
    guard let summary = reply.runtimeRules else { throw NativeClientError.boundary }
    return summary
  }

  /// 大规则集采用 YAML 文件输入，保持 FFI 的既有缓冲区上限。
  public func update(inputFile: URL) throws -> NativeRuntimeRulesSummary {
    guard inputFile.isFileURL else { throw NativeClientError.boundary }
    let reply = try client.command([
      "operation": "runtime_rules_update_file",
      "path": inputFile.path,
    ])
    guard let summary = reply.runtimeRules else { throw NativeClientError.boundary }
    return summary
  }

  public func status() throws -> NativeRuntimeRulesSummary {
    guard let summary = try client.command(["operation": "runtime_rules_status"]).runtimeRules
    else { throw NativeClientError.boundary }
    return summary
  }

  /// 纯决策检查；域名没有真实地址时不能把结果当作连接已经获准。
  public func evaluate(_ target: RoutingTarget) throws -> NativeRouteDecision {
    guard var command = try JSONSerialization.jsonObject(
      with: JSONEncoder().encode(target)
    ) as? [String: Any] else { throw NativeClientError.boundary }
    command["operation"] = "route_evaluate"
    guard let decision = try client.command(command).routeDecision
    else { throw NativeClientError.boundary }
    return decision
  }

  public func close() throws {
    _ = try client.command(["operation": "runtime_rules_close"])
  }
}
