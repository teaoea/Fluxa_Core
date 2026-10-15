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
  public var rules: [RoutingRule]?
  public var payloadYaml: String?
  public var policy: RoutingRule.Policy?
  public var outbound: String?
  enum CodingKeys: String, CodingKey {
    case id, layer, enabled, rules, policy, outbound
    case payloadYaml = "payload_yaml"
  }

  public init(id: String, layer: Layer, enabled: Bool = true, rules: [RoutingRule]) {
    self.id = id
    self.layer = layer
    self.enabled = enabled
    self.rules = rules
    payloadYaml = nil
    policy = nil
    outbound = nil
  }

  public init(
    id: String,
    enabled: Bool = true,
    payloadYaml: String,
    policy: RoutingRule.Policy = .proxy,
    outbound: String? = nil
  ) {
    self.id = id
    layer = .remote
    self.enabled = enabled
    rules = nil
    self.payloadYaml = payloadYaml
    self.policy = policy
    self.outbound = outbound
  }
}

/// 裁剪报告的信任程度；旧版本没有该字段时不能默认视为已验证。
public enum NativeReportVerification: String, Decodable, Sendable {
  case verifiedFromSources = "verified-from-sources"
  case structuralOnly = "structural-only"
}

public struct NativeRuntimeRulesSummary: Decodable, Sendable {
  public let reportVerification: NativeReportVerification?
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
    case reportVerification = "report_verification"
    case version, original, duplicates, conflicts, pruned, effective
    case directorySynced = "directory_synced"
  }
}

/// 后台生成状态，不传日志或规则正文；失败时活动规则版本不会改变。
public struct NativeRuntimeRulesJob: Decodable, Sendable {
  public enum Phase: String, Decodable, Sendable {
    case compiling, succeeded, failed, cancelled
  }

  public let id: UInt64
  public let phase: Phase
  public let version: UInt64?
  public let directorySynced: Bool?
  public let error: String?
  public let errorInfo: NativeKernelError?
  enum CodingKeys: String, CodingKey {
    case errorInfo = "error_info"
    case id, phase, version, error
    case directorySynced = "directory_synced"
  }
}

/// 生成文件和内核快照由 Rust 原子提交；客户端只传递原始输入与显示统计。
public final class NativeRuntimeRulesClient: Sendable {
  private let client: NativeCommandClient
  public init(directory: URL) throws {
    guard directory.isFileURL else { throw NativeClientError.boundary }
    client = try NativeCommandClient(requiredCapabilities: 196_608)
    _ = try client.command(["operation": "runtime_rules_open", "directory": directory.path])
  }

  /// 仅提交任务，去重、裁剪、编译和完整验证均由 Rust 的有界工作线程执行。
  public func beginUpdate(
    settings: RoutingSettings,
    sources: [NativeRuleSource]
  ) throws -> NativeRuntimeRulesJob {
    if sources.contains(where: { $0.rules?.contains(where: { $0.protocol != nil }) == true }) {
      try client.requireDomainTransport()
    }
    let reply = try client.command([
      "operation": "runtime_rules_begin",
      "settings": JSONSerialization.jsonObject(with: JSONEncoder().encode(settings)),
      "sources": JSONSerialization.jsonObject(with: JSONEncoder().encode(sources)),
    ])
    guard let job = reply.runtimeRulesJob else { throw NativeClientError.boundary }
    return job
  }

  public func beginUpdate(inputFile: URL) throws -> NativeRuntimeRulesJob {
    guard inputFile.isFileURL else { throw NativeClientError.boundary }
    let reply = try client.command([
      "operation": "runtime_rules_begin_file",
      "path": inputFile.path,
    ])
    guard let job = reply.runtimeRulesJob else { throw NativeClientError.boundary }
    return job
  }

  public func jobStatus(id: UInt64) throws -> NativeRuntimeRulesJob {
    guard
      let job = try client.command(["operation": "runtime_rules_job", "id": id]).runtimeRulesJob
    else { throw NativeClientError.boundary }
    return job
  }

  /// 取消只影响尚未提交的任务；已经完成的文件及快照不会被回滚。
  public func cancelJob(id: UInt64) throws {
    _ = try client.command(["operation": "runtime_rules_cancel", "id": id])
  }

  public func update(
    settings: RoutingSettings,
    sources: [NativeRuleSource]
  ) async throws -> NativeRuntimeRulesSummary {
    let job = try await Task.detached(priority: .utility) {
      try self.beginUpdate(settings: settings, sources: sources)
    }.value
    return try await waitForJob(id: job.id)
  }

  public func update(inputFile: URL) async throws -> NativeRuntimeRulesSummary {
    let job =
      try await Task
      .detached(priority: .utility) { try self.beginUpdate(inputFile: inputFile) }.value
    return try await waitForJob(id: job.id)
  }

  private func waitForJob(id: UInt64) async throws -> NativeRuntimeRulesSummary {
    try await withTaskCancellationHandler {
      while true {
        try Task.checkCancellation()
        let job = try jobStatus(id: id)
        switch job.phase {
        case .compiling:
          try await Task.sleep(nanoseconds: 50_000_000)
        case .succeeded:
          let summary = try status()
          guard summary.version == job.version
          else { throw NativeClientError.rejected("规则结果已被后续更新替换。") }
          return summary
        case .failed:
          throw NativeClientError.rejected(job.error ?? "规则编译失败。")
        case .cancelled:
          throw CancellationError()
        }
      }
    } onCancel: {
      try? self.cancelJob(id: id)
    }
  }

  public func status() throws -> NativeRuntimeRulesSummary {
    guard let summary = try client.command(["operation": "runtime_rules_status"]).runtimeRules
    else { throw NativeClientError.boundary }
    return summary
  }

  /// 纯决策检查；域名没有真实地址时不能把结果当作连接已经获准。
  public func evaluate(_ target: RoutingTarget) throws -> NativeRouteDecision {
    guard
      var command = try JSONSerialization.jsonObject(
        with: JSONEncoder().encode(target)
      ) as? [String: Any]
    else { throw NativeClientError.boundary }
    command["operation"] = "route_evaluate"
    guard let decision = try client.command(command).routeDecision
    else { throw NativeClientError.boundary }
    return decision
  }

  public func close() throws {
    _ = try client.command(["operation": "runtime_rules_close"])
  }
}

/// 持有 Rust 快照的生命周期锚点；仅传递句柄，不在 Swift 中复制索引或规则匹配。
/// 在途请求保留本对象，新版本提交不会使尚未配置网络客户端的旧句柄失效。
public final class NativeRuleSnapshot: Sendable {
  public let summary: NativeRuntimeRulesSummary
  public let routing: RoutingConfiguration
  private let keeper: NativeRoutingClient

  public init(summary: NativeRuntimeRulesSummary, settings: RoutingSettings) throws {
    self.summary = summary
    routing = RoutingConfiguration(settings: settings, runtimeHandle: summary.snapshotHandle)
    keeper = try NativeRoutingClient()
    try keeper.configure(routing)
  }
}
