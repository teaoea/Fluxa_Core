import Foundation

/// 统计 IP 数据包字节；不包含节点外层加密开销，按一次隧道连接独立计数。
public struct TunnelTrafficSnapshot: Codable, Sendable {
  public let connectionID: UUID
  public let sampledAt: TimeInterval
  public let uploadedBytes: UInt64
  public let downloadedBytes: UInt64

  public init(
    connectionID: UUID, sampledAt: TimeInterval,
    uploadedBytes: UInt64, downloadedBytes: UInt64
  ) {
    self.connectionID = connectionID
    self.sampledAt = sampledAt
    self.uploadedBytes = uploadedBytes
    self.downloadedBytes = downloadedBytes
  }
}

/// 使用单调时间与计数差计算实际速度；首样本、重连或计数回退时不生成速度。
public struct TrafficStatistics: Sendable {
  public private(set) var snapshot: TunnelTrafficSnapshot?
  public private(set) var downloadBytesPerSecond: Double?
  public private(set) var uploadBytesPerSecond: Double?

  public init() {}

  public mutating func update(_ current: TunnelTrafficSnapshot) {
    downloadBytesPerSecond = nil
    uploadBytesPerSecond = nil
    if let previous = snapshot, previous.connectionID == current.connectionID {
      let elapsed = current.sampledAt - previous.sampledAt
      if elapsed.isFinite, elapsed > 0,
         current.downloadedBytes >= previous.downloadedBytes,
         current.uploadedBytes >= previous.uploadedBytes {
        downloadBytesPerSecond = Double(current.downloadedBytes - previous.downloadedBytes) /
          elapsed
        uploadBytesPerSecond = Double(current.uploadedBytes - previous.uploadedBytes) / elapsed
      }
    }
    snapshot = current
  }
}
