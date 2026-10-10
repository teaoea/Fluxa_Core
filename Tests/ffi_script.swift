import Foundation

/// 编译实际 Swift 客户端并链接 Rust 静态库，验证失败恢复与隔离。
@main
struct ScriptFFICheck {
  static func main() throws {
    let client = try NativeScriptClient()
    let input = Data(#"{"city":"武汉市","body":"hello"}"#.utf8)
    let output = try client.execute(
      source: "input => ({city: input.city, body: input.body.toUpperCase()})", input: input
    )
    let object = try JSONSerialization.jsonObject(with: output) as? [String: String]
    precondition(object == ["city": "武汉市", "body": "HELLO"])
    for source in [
      "() => {while(true){}}", "() => undefined", "() => Promise.resolve(1)",
      "() => ({get secret(){throw new Error('secret')}})", "invalid syntax",
      "() => {let v=[];for(;;)v.push('x'.repeat(100000))}",
      "() => fetch('https://example.com')",
    ] {
      do {
        _ = try client.execute(source: source, input: Data("null".utf8))
        preconditionFailure("应拒绝无效脚本")
      } catch NativeClientError.rejected {}
      let next = try client.execute(source: "input => input + 1", input: Data("1".utf8))
      precondition(String(data: next, encoding: .utf8) == "2")
    }
    _ = try client.execute(source: "() => {globalThis.secret = 1;return null}", input: input)
    let isolated = try client.execute(source: "() => typeof secret", input: input)
    precondition(String(data: isolated, encoding: .utf8) == #""undefined""#)
    let null = try client.execute(source: "input => input", input: Data("null".utf8))
    precondition(String(data: null, encoding: .utf8) == "null")
    // 多次创建、执行、销毁，验证 FFI 结果与运行时不会占住资源句柄。
    for _ in 0 ..< 100 {
      let fresh = try NativeScriptClient()
      let result = try fresh.execute(source: "input=>input", input: Data("[1,true,null]".utf8))
      precondition(String(data: result, encoding: .utf8) == "[1,true,null]")
    }
    print("Swift JS 客户端真实 FFI 测试通过：转换、超时、内存失败恢复、隔离及资源回收。")
  }
}
