import Foundation

@main
struct GeoIPFFICheck {
  static func main() throws {
    // 测试数据库是自有生成数据；实际 Swift 调用 Rust 的文件、查询和释放入口。
    let database = URL(fileURLWithPath: CommandLine.arguments[1])
    let client = try NativeGeoIPClient()
    try client.open(database: database)
    let ipv4 = try client.lookup(address: "8.8.8.8", language: "zh-CN")
    precondition(ipv4?.countryCode == "US" && ipv4?.city == "Los Angeles")
    let ipv6 = try client.lookup(address: "2001:4860:4860::8888")
    precondition(ipv6?.countryCode == "TW")
    let missing = try client.lookup(address: "192.0.2.1")
    precondition(missing == nil)
    do { _ = try client.lookup(address: "invalid")
      fatalError("非法地址不能成功")
    } catch {}
    let invalid = database.appendingPathExtension("invalid")
    try Data("invalid".utf8).write(to: invalid)
    defer { try? FileManager.default.removeItem(at: invalid) }
    do { try client.open(database: invalid)
      fatalError("损坏库不能成功")
    } catch {}
    let retained = try client.lookup(address: "8.8.8.8")
    precondition(retained?.countryCode == "US")
    try client.close()
    do { _ = try client.lookup(address: "8.8.8.8")
      fatalError("关闭后不能查询")
    } catch {}
    try client.open(database: database)
    let reopened = try client.lookup(address: "8.8.8.8")
    precondition(reopened?.countryCode == "US")
    try client.close()
    print("GeoIP 实际 Swift FFI 通过：双栈、缺失、非法输入、失败保留旧库、关闭与重开")
  }
}
