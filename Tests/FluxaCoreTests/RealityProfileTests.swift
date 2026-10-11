@testable import FluxaCore
import Foundation
import XCTest

final class RealityProfileTests: XCTestCase {
  private let link = "vless://00000000-0000-4000-8000-000000000001@127.0.0.1:443?encryption=none&type=tcp&security=reality&flow=xtls-rprx-vision&sni=example.com&pbk=CQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA&sid=aabb&fp=chrome#测试节点"

  func testImportsExactStack() throws {
    let profile = try RealityProfile.parse(link)
    XCTAssertEqual(profile.name, "测试节点")
    XCTAssertEqual(profile.flow, "xtls-rprx-vision")
    XCTAssertEqual(profile.shortID, "aabb")
  }

  func testNativeFingerprintAndURIKeepParameters() throws {
    let profile = try RealityProfile.parse(link.replacingOccurrences(
      of: "fp=chrome",
      with: "fp=fluxa"
    ))
    let decoded = try RealityProfile.parse(profile.uri)
    XCTAssertEqual(decoded.userID, profile.userID)
    XCTAssertEqual(decoded.host, profile.host)
    XCTAssertEqual(decoded.fingerprint, "fluxa")
    XCTAssertEqual(decoded.publicKey, profile.publicKey)
    XCTAssertTrue(ProxyProtocol.vlessReality.isImplemented)
    XCTAssertFalse(ProxyProtocol.socks5.isImplemented)
  }

  func testAllPublishedFingerprintNamesAndUnknownRejection() throws {
    for name in [
      "chrome",
      "chrome120",
      "chrome131",
      "chrome133",
      "firefox",
      "firefox120",
      "safari",
      "safari16",
      "ios",
      "ios14",
      "android",
      "android-chrome120",
      "android-chrome133",
      "edge",
      "edge106",
      "random",
      "fluxa",
    ] {
      let profile = try RealityProfile.parse(link.replacingOccurrences(
        of: "fp=chrome",
        with: "fp=" + name
      ))
      XCTAssertEqual(profile.fingerprint, name)
      XCTAssertEqual(try RealityProfile.parse(profile.uri).fingerprint, name)
    }
    XCTAssertThrowsError(try RealityProfile.parse(link.replacingOccurrences(
      of: "fp=chrome",
      with: "fp=unknown"
    )))
    XCTAssertThrowsError(try RealityProfile.parse(link.replacingOccurrences(
      of: "fp=chrome",
      with: "fp=randomized"
    )))
  }

  func testRejectsTransportDowngradeAndUnknownParameters() {
    for value in [
      link.replacingOccurrences(of: "type=tcp", with: "type=ws"),
      link.replacingOccurrences(of: "flow=xtls-rprx-vision", with: "flow="),
      link.replacingOccurrences(of: "security=reality", with: "security=tls"),
      link.replacingOccurrences(of: "#测试节点", with: "&allowInsecure=1"),
    ] {
      XCTAssertThrowsError(try RealityProfile.parse(value))
    }
  }

  func testRejectsDuplicateQueryAndInvalidShortID() {
    XCTAssertThrowsError(try RealityProfile.parse(link.replacingOccurrences(
      of: "#测试节点",
      with: "&sid=aa"
    )))
    XCTAssertThrowsError(try RealityProfile.parse(link.replacingOccurrences(
      of: "sid=aabb",
      with: "sid=abc"
    )))
  }

  func testRejectsBadPublicKeyAndUUID() {
    XCTAssertThrowsError(try RealityProfile.parse(link.replacingOccurrences(
      of: "pbk=CQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
      with: "pbk=bad"
    )))
    XCTAssertThrowsError(try RealityProfile.parse(link.replacingOccurrences(
      of: "00000000-0000-4000-8000-000000000001",
      with: "invalid"
    )))
  }
}
