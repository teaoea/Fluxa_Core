import Foundation

/// 旧头文件没有 DebugIPC 子模块时，SDK 必须可编译且明确报不可用。
@main
struct DebugUnavailableCheck {
  static func main() throws {
    do {
      _ = try NativeDebugClient(
        endpoint: "/not-used", credential: "/not-used", serverUID: 0,
        serverRequirement: "not-used", lifetimeSeconds: 60
      )
      preconditionFailure("旧 ABI 不应启用 Debug")
    } catch NativeClientError.unavailable {
      print("旧 ABI 门面编译及不可用返回通过；未读取凭证或发出请求")
    }
  }
}
