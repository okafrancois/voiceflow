// swift-tools-version: 6.0
import PackageDescription

let package = Package(
    name: "VoiceFlow",
    platforms: [
        .macOS("26.0")
    ],
    dependencies: [
        .package(url: "https://github.com/argmaxinc/WhisperKit.git", from: "0.9.0"),
        .package(url: "https://github.com/k2-fsa/sherpa-onnx", from: "1.13.8")
    ],
    targets: [
        .executableTarget(
            name: "VoiceFlow",
            dependencies: [
                .product(name: "WhisperKit", package: "WhisperKit"),
                .product(name: "sherpa-onnx", package: "sherpa-onnx")
            ],
            path: "Sources/VoiceFlow",
            swiftSettings: [
                // Strict concurrency: shared access between the audio thread,
                // the hotkey thread and the UI is checked at
                // compile time.
                .swiftLanguageMode(.v6)
            ]
        ),
        .testTarget(
            name: "VoiceFlowTests",
            dependencies: ["VoiceFlow"],
            path: "Tests/VoiceFlowTests",
            swiftSettings: [.swiftLanguageMode(.v6)]
        )
    ]
)
