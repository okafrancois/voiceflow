// swift-tools-version: 6.0
import PackageDescription

// Static library linked into the Tauri binary by build.rs. It exposes a small
// C ABI over Apple's SpeechAnalyzer, which is only reachable from Swift.
let package = Package(
    name: "AppleSpeech",
    platforms: [.macOS(.v12)],
    products: [
        .library(name: "AppleSpeech", type: .static, targets: ["AppleSpeech"]),
    ],
    targets: [
        .target(name: "AppleSpeech"),
    ],
    swiftLanguageModes: [.v5]
)
