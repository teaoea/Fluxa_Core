@testable import FluxaCore
import XCTest

/// 验证真实计数的速度差分、重连与异常样本，避免首页显示负速度或跨连接累计。
final class TrafficStatisticsTests: XCTestCase {
  func testFirstSampleHasTotalsWithoutInventedSpeed() {
    var statistics = TrafficStatistics()
    statistics.update(snapshot(at: 10, uploaded: 100, downloaded: 200))
    XCTAssertEqual(statistics.snapshot?.downloadedBytes, 200)
    XCTAssertNil(statistics.downloadBytesPerSecond)
    XCTAssertNil(statistics.uploadBytesPerSecond)
  }

  func testRatesUseActualByteDeltaAndMonotonicTime() {
    var statistics = TrafficStatistics()
    statistics.update(snapshot(at: 10, uploaded: 100, downloaded: 200))
    statistics.update(snapshot(at: 12, uploaded: 300, downloaded: 800))
    XCTAssertEqual(statistics.uploadBytesPerSecond, 100)
    XCTAssertEqual(statistics.downloadBytesPerSecond, 300)
  }

  func testNewConnectionDoesNotReuseOldCounters() {
    var statistics = TrafficStatistics()
    statistics.update(snapshot(at: 10, uploaded: 100, downloaded: 200))
    statistics.update(.init(
      connectionID: UUID(),
      sampledAt: 12,
      uploadedBytes: 1,
      downloadedBytes: 2
    ))
    XCTAssertNil(statistics.downloadBytesPerSecond)
    XCTAssertEqual(statistics.snapshot?.downloadedBytes, 2)
  }

  func testCounterResetAndInvalidTimeHaveNoRates() {
    var statistics = TrafficStatistics()
    statistics.update(snapshot(at: 10, uploaded: 100, downloaded: 200))
    statistics.update(snapshot(at: 12, uploaded: 0, downloaded: 0))
    XCTAssertNil(statistics.uploadBytesPerSecond)
    statistics.update(snapshot(at: 12, uploaded: 10, downloaded: 20))
    XCTAssertNil(statistics.downloadBytesPerSecond)
  }

  private let connection = UUID()
  private func snapshot(
    at time: Double,
    uploaded: UInt64,
    downloaded: UInt64
  ) -> TunnelTrafficSnapshot {
    .init(
      connectionID: connection,
      sampledAt: time,
      uploadedBytes: uploaded,
      downloadedBytes: downloaded
    )
  }
}
