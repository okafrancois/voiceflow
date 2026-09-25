// swift-tools-version: 6.0
import PackageDescription

// Static library linked into the Tauri binary by build.rs. It exposes a small
// C ABI over the Swift-only SpeechAnalyzer and Foundation Models APIs.
let package = Package(
    name: "AppleBridge",
    platforms: [.macOS(.v12)],
    products: [
        .library(name: "AppleBridge", type: .static, targets: ["AppleBridge"]),
    ],
    targets: [
        .target(name: "AppleBridge"),
    ],
    swiftLanguageModes: [.v5]
)
