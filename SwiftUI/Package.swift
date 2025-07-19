// Copyright (c) eightman 2005-2025
// Furin-lab All rights reserved.
// 動作設計: Swift Package Manager用のビルド設定ファイル

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
