import Foundation

/// 使用 Rust 脚本引擎处理 JSON；脚本来源必须由应用控制，不提供下载执行入口。
/// 每次执行相互隔离；同步调用应放在后台，输入和输出都是拥有独立内存的 Data。
public final class NativeScriptClient: Sendable {
  private let transport: NativeCommandClient

  public init() throws {
    transport = try NativeCommandClient(requiredCapabilities: 4)
  }

  /// source 为同步函数表达式，input 为 JSON 文本；不支持 Promise 或原生系统调用。
  public func execute(source: String, input: Data) throws -> Data {
    guard !source.isEmpty, source.utf8.count <= 256 * 1024,
          input.count <= 1024 * 1024 else { throw NativeClientError.boundary }
    let value = try JSONSerialization.jsonObject(with: input, options: .fragmentsAllowed)
    let reply = try transport.command(["operation": "script", "source": source, "input": value])
    guard let result = reply.scriptResultJSON,
          result.utf8.count <= 1024 * 1024 else { throw NativeClientError.boundary }
    return Data(result.utf8)
  }
}
