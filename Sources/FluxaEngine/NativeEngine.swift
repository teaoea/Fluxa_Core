import FluxaCore

/// Swift 接入门面；Rust 平台库尚未接入时，所有网络启动明确拒绝。
/// 不在 Swift 中补写网络实现，也不将节点格式校验当作协议互通成功。
public actor NativeEngine {
  public enum Ingress: Sendable {
    case local(socksPort: UInt16, httpPort: UInt16)
    case tunnel(fileDescriptor: Int32, httpPort: UInt16?)
  }

  public static let shared = NativeEngine()
  public static let isAvailable = false
  private init() {}

  public func validate(profile _: RealityProfile?, mode _: RoutingMode = .rules) throws {
    throw CoreError.unsupported("自研 Rust 网络引擎尚未完成，当前不能验证协议连通性。")
  }

  public func start(
    profile _: RealityProfile?,
    ingress _: Ingress,
    rules _: [RoutingRule] = [],
    mode _: RoutingMode = .rules
  ) throws {
    throw CoreError.unsupported("自研 Rust 网络引擎尚未完成，当前不能启动网络连接。")
  }

  /// 未创建任何网络资源，停止操作保持幂等。
  public func stop() {}
}
