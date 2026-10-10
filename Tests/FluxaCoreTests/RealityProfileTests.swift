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
