import FluxaEngine
import Foundation
import XCTest

/// Swift package 链接真实平台库后验证，不用模拟实现替代 FFI。
final class EngineFFITests: XCTestCase {
  func testNativePacketSessionRejectsUnsupportedTunnel() throws {
    #if canImport(libfluxa_core)
    let client = try NativeTunPacketClient()
    let packet =
      try XCTUnwrap(
        Data(base64Encoded: "RQAAKwAAQABABk6WwAACAcYzZAIwOQA1AAAAAQAAAABQAv//ztYAAGFiYw==")
      )
    let info = try client.inspect(packet)
    XCTAssertEqual(info.source, "192.0.2.1")
    XCTAssertEqual(info.destinationPort, 53)
    XCTAssertThrowsError(try client.startTunnel())
    try client.close()
    XCTAssertThrowsError(try client.inspect(packet))
    #else
    throw XCTSkip("当前平台未发布 Rust 二进制切片。")
    #endif
  }

  func testNativeCertificateIdentityRoundTrip() throws {
    #if canImport(libfluxa_core)
    let client = try NativeMitmClient()
    let certificate = try client.rootCertificate()
    let identity = try client.exportIdentityForSecureStorage()
    XCTAssertEqual(identity.certificatePEM, certificate)
    let restored = try NativeMitmClient()
    try restored.restoreIdentity(identity)
    XCTAssertEqual(try restored.rootCertificate(), certificate)
    try restored.stop()
    #else
    throw XCTSkip("尚未构建 Rust 平台库，请先运行 build-macos.sh。")
    #endif
  }

  func testNativeScriptIsolationAndTimeoutRecovery() throws {
    #if canImport(libfluxa_core)
    let client = try NativeScriptClient()
    let result = try client.execute(source: "input => ({value: input + 1})", input: Data("2".utf8))
    XCTAssertEqual(String(data: result, encoding: .utf8), "{\"value\":3}")
    XCTAssertThrowsError(try client.execute(
      source: "() => {while(true){}}",
      input: Data("null".utf8)
    ))
    let recovered = try client.execute(source: "input => input", input: Data("null".utf8))
    XCTAssertEqual(String(data: recovered, encoding: .utf8), "null")
    #else
    throw XCTSkip("尚未构建 Rust 平台库，请先运行 build-macos.sh。")
    #endif
  }
}
