// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "DiffLoupeSwiftUI",
    platforms: [
        .macOS(.v12)
    ],
    products: [
        .executable(name: "DiffLoupeSwiftUI", targets: ["DiffLoupeSwiftUI"])
    ],
    targets: [
        .executableTarget(
            name: "DiffLoupeSwiftUI",
            path: "Sources"
        )
    ]
)
