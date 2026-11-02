// swift-tools-version: 6.0
import PackageDescription
import Foundation
let rustLibrary = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().appendingPathComponent("target/release").path
let package = Package(
    name: "Fountage",
    platforms: [.macOS(.v14)],
    targets: [
        .systemLibrary(name: "FountageCore", path: "Sources/FountageCore"),
        .executableTarget(name: "Fountage", dependencies: ["FountageCore"],
            linkerSettings: [.unsafeFlags(["-L", rustLibrary, "-lfountage_core", "-liconv"])]),
        .testTarget(name: "FountageTests", dependencies: ["Fountage"])
    ],
    swiftLanguageModes: [.v5]
)
