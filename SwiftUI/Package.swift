// swift-tools-version: 5.9
// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: WinMerge互換のdiffloupe CLIをSwiftUIアプリの既存coreソース(Engine/Models)から
// 再構成するSwift Package。coreは既存ファイルをsources指定で参照し、UI/AppKit依存
// (DiffLoupeApp / UI / Pro / ImageDiffEngine)は含めない。比較アルゴリズムの二重実装はしない。
// DiffLoupeUIはSwiftUI/AppKitのUI層(UI / ImageDiffEngine)をライブラリ化したもので、
// 有償版などの別パッケージがAppModel・各ビュー・DiffLoupeEditionを再利用するために公開する
// (アプリ本体=Xcodeプロジェクトは従来どおりソースを直接コンパイルする)。
//
// 注意:
// - 同一ソースが複数ターゲットに重複するとSwiftPMエラーになるため、各coreファイルは
//   DiffLoupeCoreターゲットでのみ宣言している(CLI/テストは依存として参照)。
// - モジュール境界をまたいでcore型を利用するため、必要な型・メンバーにpublicを付与している
//   (挙動変更なし・アクセス修飾のみ)。

import PackageDescription

let package = Package(
    name: "DiffLoupe",
    defaultLocalization: "ja",
    platforms: [
        .macOS(.v13)
    ],
    products: [
        .executable(name: "diffloupe", targets: ["diffloupe"]),
        .library(name: "DiffLoupeCore", targets: ["DiffLoupeCore"]),
        .library(name: "DiffLoupeCLI", targets: ["DiffLoupeCLI"]),
        .library(name: "DiffLoupeUI", targets: ["DiffLoupeUI"]),
    ],
    targets: [
        // Rust コア(diffloupe-ffi)のユニバーサル static library。
        // 生成物のため git 管理外。`Scripts/build_rust_core.sh` を実行して生成する。
        .binaryTarget(
            name: "diffloupeFFI",
            path: "RustCore/diffloupeFFI.xcframework"
        ),
        // 既存のcore Swiftソース(Engine/Models)をsources指定で参照する。
        // AppKit依存(ImageDiffEngine)とUI/AppKit依存(DiffLoupeApp/UI/Pro)は含めない。
        // 実装は Rust コアへの薄いファサード。UniFFI 生成バインディング
        // (Engine/Generated/diffloupe.swift, 生成物)を同ターゲットでコンパイルする。
        .target(
            name: "DiffLoupeCore",
            dependencies: ["diffloupeFFI"],
            path: "DiffLoupe",
            exclude: [
                "UI",
                "Pro",
                "DiffLoupeApp.swift",
                "AppIcon.icns",
                "Info.plist",
                ".DS_Store",
                "Engine/ImageDiffEngine.swift",
            ],
            sources: [
                "Engine/Generated/diffloupe.swift",
                "Engine/ComparisonOptions.swift",
                "Engine/DirectoryWatcher.swift",
                "Engine/FolderComparator.swift",
                "Engine/FilterSet.swift",
                "Engine/GitSnapshot.swift",
                "Engine/HexDiff.swift",
                "Engine/InlineDiff.swift",
                "Engine/LineDiff.swift",
                "Engine/StructuredNormalizer.swift",
                "Engine/SyncPlan.swift",
                "Engine/TextFileLoader.swift",
                "Engine/ThreeWayDiff.swift",
                "Models/ComparisonReport.swift",
                "Models/DiffDisplay.swift",
                "Models/DiffModels.swift",
                "Models/RecentComparison.swift",
            ]
        ),
        // SwiftUI/AppKitのUI層。Coreの型を使うため依存に含める。Pro/(非公開)・DiffLoupeApp.swiftは対象外。
        .target(
            name: "DiffLoupeUI",
            dependencies: ["DiffLoupeCore"],
            path: "DiffLoupe",
            exclude: [
                "Pro",
                "Engine/Generated",
                "Engine/ComparisonOptions.swift",
                "Engine/DirectoryWatcher.swift",
                "Engine/FilterSet.swift",
                "Engine/FolderComparator.swift",
                "Engine/GitSnapshot.swift",
                "Engine/HexDiff.swift",
                "Engine/InlineDiff.swift",
                "Engine/LineDiff.swift",
                "Engine/StructuredNormalizer.swift",
                "Engine/SyncPlan.swift",
                "Engine/TextFileLoader.swift",
                "Engine/ThreeWayDiff.swift",
                "Models/ComparisonReport.swift",
                "Models/DiffDisplay.swift",
                "Models/DiffModels.swift",
                "Models/RecentComparison.swift",
                "DiffLoupeApp.swift",
                "AppIcon.icns",
                "Info.plist",
                ".DS_Store",
            ],
            sources: [
                "UI",
                "Engine/ImageDiffEngine.swift",
            ]
        ),
        // CLIの純粋ロジック(引数解析・終了コード判定・レポート出力・実行制御)。
        // ライブラリ化することでSwiftテストからimportできる(実行ファイルはimport不可のため)。
        .target(
            name: "DiffLoupeCLI",
            dependencies: ["DiffLoupeCore"],
            path: "CLILib"
        ),
        // WinMergeのCLI/CI相当のコマンド。main.swiftのみを持つ薄いエントリポイント。
        .executableTarget(
            name: "diffloupe",
            dependencies: ["DiffLoupeCLI", "DiffLoupeCore"],
            path: "CLI",
            sources: ["main.swift"]
        ),
        .testTarget(
            name: "DiffLoupeCLITests",
            dependencies: ["DiffLoupeCLI", "DiffLoupeCore"],
            path: "CLITests"
        ),
        // Coreエンジン(FolderComparator等)のフォーカステスト。XcodeのUIテストスイートの
        // 重複はせず、SwiftPMだけで検証すべき最小限のコア挙動(再帰オプション等)を扱う。
        .testTarget(
            name: "DiffLoupeCoreTests",
            dependencies: ["DiffLoupeCore"],
            path: "CoreTests"
        ),
    ]
)
