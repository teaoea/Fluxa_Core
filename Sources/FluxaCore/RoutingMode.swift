/// 显式模式由用户选择；代理失败绝不隐式切换直连。
public enum RoutingMode: String, Codable, CaseIterable, Sendable {
  case direct, rules, global
}
