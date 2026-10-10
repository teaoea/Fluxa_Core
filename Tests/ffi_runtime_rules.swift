import Foundation

@main
struct RuntimeRulesFFICheck {
  static func main() throws {
    let directory = FileManager.default.temporaryDirectory
      .appendingPathComponent("fluxa-runtime-ffi-\(UUID().uuidString)")
    defer { try? FileManager.default.removeItem(at: directory) }
    let client = try NativeRuntimeRulesClient(directory: directory)
    let local = try NativeRuleSource(id: "local", layer: .local, rules: [
      RoutingRule(type: .domainSuffix, value: "bbb.com", policy: .direct),
    ])
    let remote = try NativeRuleSource(id: "remote", layer: .remote, rules: [
      RoutingRule(type: .domain, value: "aaa.bbb.com", policy: .proxy, outbound: "test"),
      RoutingRule(type: .domain, value: "ads.bbb.com", policy: .reject),
      RoutingRule(type: .ipCIDR, value: "192.0.2.0/24", policy: .reject),
    ])
    let summary = try client.update(
      settings: RoutingSettings(fallback: .direct),
      sources: [local, remote]
    )
    precondition(summary.version == 1 && summary.original == 4 && summary.pruned == 0 && summary
      .effective == 4)
    let allowed = try client.evaluate(RoutingTarget(domain: "aaa.bbb.com", address: "198.51.100.1"))
    precondition(allowed.policy == .proxy)
    for target in [
      RoutingTarget(domain: "ads.bbb.com", address: "198.51.100.1"),
      RoutingTarget(domain: "unmatched.test", address: "192.0.2.1"),
    ] {
      let decision = try client.evaluate(target)
      precondition(decision.policy == .reject)
    }
    for mode in [RoutingMode.direct, .global] {
      _ = try client.update(
        settings: RoutingSettings(mode: mode, outbound: "test"),
        sources: [local, remote]
      )
      let decision = try client.evaluate(RoutingTarget(
        domain: "ads.bbb.com",
        address: "198.51.100.1"
      ))
      precondition(decision.policy == .reject)
    }
    try checkSharedSnapshot(client)
    let before = try Data(contentsOf: directory.appendingPathComponent("runtime-rules.yaml"))
    let invalid = try JSONDecoder().decode(
      RoutingRule.self,
      from: Data(#"{"type":"ip-cidr","value":"bad","policy":"direct"}"#.utf8)
    )
    do {
      _ = try client.update(
        settings: RoutingSettings(),
        sources: [NativeRuleSource(id: "invalid", layer: .local, rules: [invalid])]
      )
      preconditionFailure("无效更新必须拒绝")
    } catch NativeClientError.rejected {}
    let after = try Data(contentsOf: directory.appendingPathComponent("runtime-rules.yaml"))
    precondition(before == after)
    let retained = try client.status()
    precondition(retained.version == 3)
    try client.close()
    let reopened = try NativeRuntimeRulesClient(directory: directory)
    let recovered = try reopened.status()
    precondition(recovered.version == 3)
    try checkYAMLInput(reopened, directory: directory)
    try reopened.close()
    print("运行时规则真实 Swift FFI：来源、裁剪、IP/域名拦截、模式、失败保留、关闭及恢复通过。")
  }

  static func checkSharedSnapshot(_ client: NativeRuntimeRulesClient) throws {
    let shared = try NativeRoutingClient()
    let state = try client.status()
    try shared.configure(RoutingConfiguration(
      settings: RoutingSettings(mode: .global, outbound: "test"),
      runtimeHandle: state.snapshotHandle
    ))
    let sharedDecision = try shared.evaluate(RoutingTarget(
      domain: "ads.bbb.com",
      address: "198.51.100.1"
    ))
    precondition(sharedDecision.policy == .reject && sharedDecision.ruleVersion == state.version)
    try shared.close()
  }

  static func checkYAMLInput(_ client: NativeRuntimeRulesClient, directory: URL) throws {
    let input = directory.appendingPathComponent("original-input.yaml")
    let yaml = """
    format_version: 1
    settings:
      mode: rules
      fallback: reject
    sources:
      - id: local
        layer: local
        rules:
          - type: key
            value: example
            policy: direct
      - id: remote
        layer: remote
        rules:
          - type: ip-cidr
            value: 192.0.2.0/24
            policy: reject
    """
    try Data(yaml.utf8).write(to: input)
    let summary = try client.update(inputFile: input)
    precondition(summary.version == 4 && summary.effective == 2)
    let hit = try client.evaluate(RoutingTarget(domain: "myexample.net", address: "192.0.2.1"))
    precondition(hit.policy == .direct)
    let miss = try client.evaluate(RoutingTarget(domain: "different.test", address: "192.0.2.1"))
    precondition(miss.policy == .reject)
    try Data((yaml + "\nformat_version: 1\n").utf8).write(to: input)
    do {
      _ = try client.update(inputFile: input)
      preconditionFailure("重复 YAML 键必须拒绝")
    } catch NativeClientError.rejected {}
    let retained = try client.status()
    precondition(retained.version == 4)
  }
}
