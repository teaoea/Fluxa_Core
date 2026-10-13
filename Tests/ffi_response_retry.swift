// 实际 Swift → Rust ABI：响应句柄耗尽后无损重试 TCP 读取，仅使用回环服务。
import Foundation
import libfluxa_core

@main
struct ResponseRetryFFICheck {
  static func main() throws {
    let fixture = try JSONSerialization.jsonObject(
      with: Data(contentsOf: URL(fileURLWithPath: CommandLine.arguments[1]))
    ) as! [String: Any]
    let engine = fluxa_engine_create()
    precondition(engine != 0)
    defer { precondition(fluxa_engine_destroy(engine) == 0) }
    func call(_ request: [String: Any]) throws -> UInt64 {
      let bytes = try JSONSerialization.data(withJSONObject: request)
      return bytes.withUnsafeBytes { input in
        fluxa_engine_command(engine, input.bindMemory(to: UInt8.self).baseAddress, bytes.count)
      }
    }
    func reply(_ handle: UInt64) throws -> [String: Any] {
      precondition(handle != 0)
      defer { precondition(fluxa_engine_destroy(handle) == 0) }
      var bytes = Data(count: fluxa_engine_buffer_length(handle))
      let count = bytes.count
      let copied = bytes.withUnsafeMutableBytes { output in
        fluxa_engine_buffer_copy(handle, output.bindMemory(to: UInt8.self).baseAddress, count)
      }
      precondition(copied == count)
      return try JSONSerialization.jsonObject(with: bytes) as! [String: Any]
    }
    let opened = try reply(call([
      "operation": "outbound_open", "address": "127.0.0.1:\(fixture["outbound_tcp_port"]!)",
      "transport": "tcp", "policy": "direct", "timeout_ms": 1000,
    ]))
    let id = opened["outbound_id"]!
    let payload = Data("FIRSTSECOND".utf8)
    let written = try reply(call([
      "operation": "outbound_write", "id": id,
      "body": payload.base64EncodedString(), "timeout_ms": 1000,
    ]))
    precondition(written["success"] as? Bool == true)
    var outputs: [UInt64] = []
    for _ in 0 ..< 63 {
      let handle = try call(["operation": "outbound_status"])
      precondition(handle != 0)
      outputs.append(handle)
    }
    defer { for handle in outputs { precondition(fluxa_engine_destroy(handle) == 0) } }
    let read: [String: Any] = [
      "operation": "outbound_read", "id": id, "maximum": 11, "timeout_ms": 1000,
    ]
    let refused = try call(read)
    precondition(refused == 0)
    precondition(fluxa_engine_destroy(outputs.removeLast()) == 0)
    var received = Data()
    while received.count < payload.count {
      let response = try reply(call(read))
      precondition(response["success"] as? Bool == true)
      received.append(Data(base64Encoded: response["body"] as! String)!)
    }
    precondition(received == payload)
    print("真实 Swift FFI：响应句柄耗尽后，释放配额并重试保留完整 TCP 数据。")
  }
}
