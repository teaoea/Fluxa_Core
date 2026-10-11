// swift-tools-version: 6.0
import PackageDescription

/// App 只下载固定发布的静态框架，不读取本机内核目录或触发 Rust 编译。
let package = Package(
  name: "FluxaCore",
  defaultLocalization: "en",
  platforms: [.macOS(.v14), .iOS(.v17), .tvOS(.v17), .watchOS(.v10), .visionOS(.v1)],
  products: [
    .library(name: "FluxaCore", targets: ["FluxaCore"]),
    .library(name: "FluxaEngine", targets: ["FluxaEngine"]),
  ],
  targets: [
    .target(name: "FluxaCore", resources: [.process("Resources")]),
    .target(
      name: "FluxaEngine",
      dependencies: [
        "FluxaCore",
        .target(name: "libfluxa_core", condition: .when(platforms: [.macOS])),
      ],
      linkerSettings: [
        .linkedFramework("Security", .when(platforms: [.macOS])),
        .linkedFramework("CoreFoundation", .when(platforms: [.macOS])),
      ]
    ),
    .binaryTarget(
      name: "libfluxa_core",
      url: "https://github.com/teaoea/Fluxa_Core/releases/download/v1.7.0/FluxaEngine.xcframework.zip",
      checksum: "a315f6bb2a8a30078756a5aed30643601846875fada439145d00c1948f2535f5"
    ),
    .testTarget(name: "FluxaCoreTests", dependencies: ["FluxaCore", "FluxaEngine"]),
  ],
  swiftLanguageModes: [.v5]
)
