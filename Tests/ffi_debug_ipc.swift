import Foundation
import libfluxa_core
import libfluxa_core.DebugIPC

/// 专用 ABI 与普通业务命令隔离，独立固定预期，不从内核生成断言。
@main
struct DebugIPCFFICheck {
  static func main() throws {
    let arguments = CommandLine.arguments
    if arguments.count == 5 {
      let client = try NativeDebugClient(
        endpoint: arguments[1], credential: arguments[2], serverUID: UInt32(arguments[3])!,
        serverRequirement: arguments[4], lifetimeSeconds: 60
      )
      let observedStatus = try client.status()
      precondition(!observedStatus.active)
      print("签名 IPC 连接及状态查询完成")
      let rule = try RoutingRule(type: .ipCIDR, value: "127.0.0.1/32", policy: .direct)
      let status = try client.replace([NativeDebugRule(rule: rule, lifetimeSeconds: 30)])
      precondition(status.active && status.count == 1)
      print("规则替换完成")
      try client.clear()
      print("清空完成")
      let clearedStatus = try client.status()
      precondition(!clearedStatus.active)
      let audit = try client.audit()
      precondition(!audit.isEmpty)
      try client.close()
      print("真实 Swift → 专用 C ABI → 签名 IPC：查询、替换、清空、审计和关闭通过")
      return
    }
    let engine = fluxa_engine_create()
    precondition(engine != 0)
    defer { _ = fluxa_engine_destroy(engine) }
    for input in [#"{"operation":"debug_replace","rules":[]}"#,
                  #"{"operation":"debug_status","authorized":true,"uid":0}"#] {
      let data = Data(input.utf8)
      let response = data.withUnsafeBytes {
        fluxa_engine_command(engine, $0.bindMemory(to: UInt8.self).baseAddress, $0.count)
      }
      precondition(response != 0 && fluxa_engine_last_error_code() == 1001)
      precondition(fluxa_engine_destroy(response) == 0)
    }
    // 未登记字段不能进入专用连接流程，普通引擎句柄不能当 Debug 客户端。
    let input = Data(#"{"authorized":true,"uid":0}"#.utf8)
    let handle = input.withUnsafeBytes {
      fluxa_engine_debug_ipc_connect($0.bindMemory(to: UInt8.self).baseAddress, $0.count)
    }
    precondition(handle == 0 && fluxa_engine_last_error_code() == 1001)
    print("真实 Swift FFI：普通 API 拒绝 Debug，专用 API 拒绝伪造身份字段")
  }
}
