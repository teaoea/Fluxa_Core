import Foundation

public enum CoreError: LocalizedError, Equatable {
  case invalidRequest(String)
  case unsupported(String)

  public var errorDescription: String? {
    switch self {
    case let .invalidRequest(detail): String(
        format: localized("配置无效：%@"),
        locale: Locale.current,
        localized(detail)
      )
    case let .unsupported(detail): localized(detail)
    }
  }

  private func localized(_ key: String) -> String {
    #if SWIFT_PACKAGE
    NSLocalizedString(key, bundle: .module, comment: "")
    #else
    key
    #endif
  }
}
