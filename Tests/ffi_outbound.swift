import Foundation

struct OutboundFixture: Decodable {
  let tcp: UInt16
  let udp: UInt16
  enum CodingKeys: String, CodingKey {
    case tcp = "outbound_tcp_port"
    case udp = "outbound_udp_port"
  }
}

@main
struct OutboundFFICheck {
  static func main() throws {
    let fixture = try JSONDecoder().decode(
      OutboundFixture.self,
      from: Data(contentsOf: URL(fileURLWithPath: CommandLine.arguments[1]))
    )
    let client = try NativeOutboundClient()
    let tcp = try client.open(address: "127.0.0.1:\(fixture.tcp)", transport: .tcp)
    try client.write(Data([0, 1, 2, 255]), id: tcp)
    var reply = Data()
    while reply.count < 4 {
      let read = try client.read(id: tcp, maximum: 2)
      precondition(!read.eof)
      reply.append(read.data)
    }
    precondition(reply == Data([0, 1, 2, 255]))
    try client.shutdownWrite(id: tcp)
    let eof = try client.read(id: tcp)
    precondition(eof.eof)
    try client.close(id: tcp)
    try rejection { _ = try client.read(id: tcp) }
    let udp = try client.open(address: "127.0.0.1:\(fixture.udp)", transport: .udp)
    for data in [Data("udp".utf8), Data()] {
      try client.write(data, id: udp)
      let read = try client.read(id: udp)
      precondition(read.data == data && !read.eof)
    }
    try client.write(Data("too-large".utf8), id: udp)
    try rejection { _ = try client.read(id: udp, maximum: 2) }
    try rejection { try client.shutdownWrite(id: udp) }
    try client.close(id: udp)
    for policy in [RoutingRule.Policy.proxy, .reject] {
      try rejection {
        _ = try client.open(address: "127.0.0.1:\(fixture.tcp)", transport: .tcp, policy: policy)
      }
    }
    try rejection { _ = try client.open(address: "example.com:443", transport: .tcp) }
    try checkCancellation(client: client, port: fixture.tcp)
    for _ in 0 ..< 100 {
      let id = try client.open(address: "127.0.0.1:\(fixture.udp)", transport: .udp)
      try client.close(id: id)
    }
    try client.closeAll()
    try client.closeAll()
    print("共用出站真实 Swift FFI：TCP/UDP、二进制数据、半关闭、EOF、报文边界、取消和 100 次生命周期通过；域名解析与路由执行尚未接入。")
  }

  static func checkCancellation(client: NativeOutboundClient, port: UInt16) throws {
    let id = try client.open(address: "127.0.0.1:\(port)", transport: .tcp)
    let finished = DispatchSemaphore(value: 0)
    DispatchQueue.global().async {
      do {
        _ = try client.read(id: id, timeoutMilliseconds: 60000)
        preconditionFailure("取消后不能返回成功")
      } catch { finished.signal() }
    }
    Thread.sleep(forTimeInterval: 0.05)
    try client.close(id: id)
    precondition(finished.wait(timeout: .now() + 1) == .success)
  }

  static func rejection(_ action: () throws -> Void) throws {
    var rejected = false
    do { try action() } catch { rejected = true }
    precondition(rejected)
  }
}
