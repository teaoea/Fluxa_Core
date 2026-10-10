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
      url: "https://github.com/teaoea/Fluxa_Core/releases/download/v1.0.0/FluxaEngine.xcframework.zip",
      checksum: "5a797598ef2a51babbf287f6e614bba2e0432a46ffa32a6020918fdceee861ef"
    ),
    .testTarget(name: "FluxaCoreTests", dependencies: ["FluxaCore", "FluxaEngine"]),
  ],
  swiftLanguageModes: [.v5]
)
