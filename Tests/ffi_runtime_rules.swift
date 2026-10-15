import Foundation

@main
struct RuntimeRulesFFICheck {
  static func main() async throws {
    let directory = FileManager.default.temporaryDirectory
      .appendingPathComponent("fluxa-runtime-ffi-\(UUID().uuidString)")
    defer { try? FileManager.default.removeItem(at: directory) }
    let client = try NativeRuntimeRulesClient(directory: directory)
    let local = try NativeRuleSource(
      id: "local", layer: .local,
      rules: [
        RoutingRule(type: .domainSuffix, value: "bbb.com", policy: .direct)
      ])
    let remote = try NativeRuleSource(
      id: "remote", layer: .remote,
      rules: [
        RoutingRule(type: .domain, value: "aaa.bbb.com", policy: .proxy, outbound: "test"),
        RoutingRule(type: .domain, value: "ads.bbb.com", policy: .reject),
        RoutingRule(type: .ipCIDR, value: "192.0.2.0/24", policy: .reject),
      ])
    let summary = try await client.update(
      settings: RoutingSettings(fallback: .direct),
      sources: [local, remote]
    )
    precondition(summary.reportVerification == .verifiedFromSources)
    precondition(
      summary.version == 1 && summary.original == 4 && summary.pruned == 0
        && summary
          .effective == 4
    )
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
      _ = try await client.update(
        settings: RoutingSettings(mode: mode, outbound: "test"),
        sources: [local, remote]
      )
      let decision = try client.evaluate(
        RoutingTarget(
          domain: "ads.bbb.com",
          address: "198.51.100.1"
        ))
      precondition(decision.policy == .reject)
    }
    try checkSharedSnapshot(client)
    let before = try Data(contentsOf: directory.appendingPathComponent("runtime-rules.bin"))
    let invalid = try JSONDecoder().decode(
      RoutingRule.self,
      from: Data(#"{"type":"IP-CIDR","value":"bad","policy":"direct"}"#.utf8)
    )
    do {
      _ = try await client.update(
        settings: RoutingSettings(),
        sources: [NativeRuleSource(id: "invalid", layer: .local, rules: [invalid])]
      )
      preconditionFailure("无效更新必须拒绝")
    } catch NativeClientError.kernel, NativeClientError.rejected {}
    let after = try Data(contentsOf: directory.appendingPathComponent("runtime-rules.bin"))
    precondition(before == after)
    let retained = try client.status()
    precondition(retained.version == 3)
    try client.close()
    let reopened = try NativeRuntimeRulesClient(directory: directory)
    let recovered = try reopened.status()
    precondition(recovered.reportVerification == .structuralOnly)
    precondition(recovered.version == 3)
    try await checkYAMLInput(reopened, directory: directory)
    try await checkBackground(reopened, directory: directory)
    try await checkClash(reopened, directory: directory)
    let scoped = try NativeRuleSource(
      id: "scoped", layer: .local,
      rules: [
        RoutingRule(type: .domain, value: "scope.example.net", policy: .reject, protocol: .udp),
        RoutingRule(type: .domain, value: "scope.example.net", policy: .direct),
      ])
    _ = try await reopened.update(settings: RoutingSettings(), sources: [scoped])
    try reopened.close()
    let scopedRecovery = try NativeRuntimeRulesClient(directory: directory)
    let tcp = try scopedRecovery.evaluate(
      RoutingTarget(domain: "scope.example.net", transport: .tcp))
    let udp = try scopedRecovery.evaluate(
      RoutingTarget(domain: "scope.example.net", transport: .udp))
    precondition(tcp.policy == .direct && udp.policy == .reject)
    try scopedRecovery.close()
    print("运行时规则真实 Swift FFI：来源、裁剪、IP/域名拦截、模式、失败保留、关闭及恢复通过。")
  }

  static func checkClash(_ client: NativeRuntimeRulesClient, directory: URL) async throws {
    let yaml = """
      payload:
        - DOMAIN,Ads.Example.com.
        - IP-CIDR,2001:db8::/32
      """
    let old = try NativeRuleSnapshot(
      summary: client.status(), settings: RoutingSettings(fallback: .direct))
    let subscription = try NativeSubscriptionClient()
    let count = try subscription.validateRules(yaml: yaml, policy: .reject)
    precondition(count == 2)
    _ = try await client.update(
      settings: RoutingSettings(),
      sources: [
        NativeRuleSource(id: "clash", payloadYaml: yaml, policy: .reject)
      ])
    let retained = try NativeRoutingClient()
    try retained.configure(old.routing)
    let oldDecision = try retained.evaluate(RoutingTarget(domain: "ads.example.com"))
    precondition(oldDecision.ruleVersion == old.summary.version)
    try retained.close()
    let decision = try client.evaluate(RoutingTarget(domain: "ads.example.com"))
    precondition(decision.policy == .reject)
    let bytes = try Data(contentsOf: directory.appendingPathComponent("runtime-rules.bin"))
    precondition(bytes.prefix(8) == Data("FLXARULE".utf8))
    do {
      _ = try subscription.validateRules(yaml: "payload: [domain,example.com]", policy: .direct)
      preconditionFailure("小写类型必须拒绝")
    } catch NativeClientError.kernel, NativeClientError.rejected {}
  }

  static func checkSharedSnapshot(_ client: NativeRuntimeRulesClient) throws {
    let shared = try NativeRoutingClient()
    let state = try client.status()
    try shared.configure(
      RoutingConfiguration(
        settings: RoutingSettings(mode: .global, outbound: "test"),
        runtimeHandle: state.snapshotHandle
      ))
    let sharedDecision = try shared.evaluate(
      RoutingTarget(
        domain: "ads.bbb.com",
        address: "198.51.100.1"
      ))
    precondition(sharedDecision.policy == .reject && sharedDecision.ruleVersion == state.version)
    try shared.close()
  }

  static func checkYAMLInput(_ client: NativeRuntimeRulesClient, directory: URL) async throws {
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
            - type: DOMAIN-KEYWORD
              value: example
              policy: direct
        - id: remote
          layer: remote
          rules:
            - type: IP-CIDR
              value: 192.0.2.0/24
              policy: reject
      """
    try Data(yaml.utf8).write(to: input)
    let summary = try await client.update(inputFile: input)
    precondition(summary.version == 4 && summary.effective == 2)
    let hit = try client.evaluate(RoutingTarget(domain: "myexample.net", address: "192.0.2.1"))
    precondition(hit.policy == .direct)
    let miss = try client.evaluate(RoutingTarget(domain: "different.test", address: "192.0.2.1"))
    precondition(miss.policy == .reject)
    try Data((yaml + "\nformat_version: 1\n").utf8).write(to: input)
    do {
      _ = try await client.update(inputFile: input)
      preconditionFailure("重复 YAML 键必须拒绝")
    } catch NativeClientError.kernel, NativeClientError.rejected {}
    let retained = try client.status()
    precondition(retained.version == 4)
  }

  static func checkBackground(_ client: NativeRuntimeRulesClient, directory: URL) async throws {
    var rules = try (0..<50000).map {
      try RoutingRule(type: .domain, value: "d\($0).unrelated.test", policy: .direct)
    }
    try rules.append(RoutingRule(type: .domainKeyword, value: "example", policy: .reject))
    let job = try client.beginUpdate(
      settings: RoutingSettings(),
      sources: [NativeRuleSource(id: "local", layer: .local, rules: rules)]
    )
    precondition(job.phase == .compiling)
    let deadline = Date().addingTimeInterval(20)
    while true {
      let state = try client.jobStatus(id: job.id)
      let decision = try client.evaluate(
        RoutingTarget(
          domain: "myexample.net",
          address: "192.0.2.1"
        ))
      precondition(
        (decision.ruleVersion == 4 && decision.policy == .direct)
          || (decision.ruleVersion == 5 && decision.policy == .reject))
      if state.phase == .succeeded {
        precondition(state.version == 5)
        break
      }
      precondition(state.phase == .compiling && Date() < deadline)
      try await Task.sleep(nanoseconds: 1_000_000)
    }
    let input = directory.appendingPathComponent("original-input.yaml")
    do {
      _ = try await client.update(inputFile: input)
      preconditionFailure("后台重复 YAML 键必须拒绝")
    } catch NativeClientError.kernel, NativeClientError.rejected {}
    let retained = try client.status()
    precondition(retained.version == 5)
    let updated = try await client.update(
      settings: RoutingSettings(fallback: .direct),
      sources: []
    )
    precondition(updated.version == 6 && updated.effective == 0)
  }
}
