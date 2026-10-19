// swift-tools-version: 6.0
// Pixel Lab for Mac: the SwiftUI app. Build with `swift build --package-path app`,
// bundle with `scripts/build-app.sh`. No external packages.

import PackageDescription

let package = Package(
    name: "PixelLab",
    platforms: [.macOS(.v15)],
    products: [
        .executable(name: "PixelLab", targets: ["PixelLab"]),
    ],
    targets: [
        .executableTarget(
            name: "PixelLab",
            path: "Sources/PixelLab",
            resources: [.process("Resources")],
            swiftSettings: [.swiftLanguageMode(.v6)]
        ),
    ]
)
