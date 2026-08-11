// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: フォルダ比較のフィルタ規則モデル。インクルード/エクスクルードglobの集合と、
// opt-inの組み込みプリセットを定義する。FolderComparator.OptionsとUI/CLI設定から共用できる
// Foundationのみのpublic型(SwiftPM対応)。照合規則は既存のGlobPatternと同一
// (区切りなし=ファイル名、あり=相対パス、`*`/`?`/`**`)。

import Foundation

/// フィルタ規則の種別
public enum FilterKind: Sendable, Hashable {
    /// インクルード(一致したファイルのみ対象)
    case include
    /// エクスクルード(一致したエントリを対象外・ディレクトリならサブツリーごと枝刈り)
    case exclude
}

/// 1件のフィルタ規則(globパターン + 種別)。
public struct FilterRule: Sendable, Hashable {
    public let kind: FilterKind
    public let pattern: String

    public init(kind: FilterKind, pattern: String) {
        self.kind = kind
        self.pattern = pattern
    }

    /// エントリがこの規則に一致するか。
    /// - Parameter name: エントリ名(最終パス成分)
    /// - Parameter relativePath: ルートからの相対パス(NFC正規化済み・"/"区切り)
    func matches(name: String, relativePath: String) -> Bool {
        GlobPattern(pattern).matches(name: name, relativePath: relativePath)
    }
}

/// インクルード/エクスクルードglobの集合。
/// 優先順位: エクスクルード > インクルード。
/// インクルードが空のときは全エントリ対象(従来挙動)。非空のときはファイル名/相対パスの
/// 少なくとも1つに一致するファイルのみが対象になる。ディレクトリはインクルードで刈らない
/// (一致しないディレクトリも降下して、ネストした一致ファイルへ到達できる)。
public struct FilterSet: Sendable, Hashable {
    /// インクルードglob。空なら全ファイル対象(従来挙動)。
    public var includePatterns: [String]
    /// エクスクルードglob。一致したエントリは対象外(ディレクトリはサブツリーごと枝刈り)。
    public var excludePatterns: [String]

    public init(includePatterns: [String] = [], excludePatterns: [String] = []) {
        self.includePatterns = Self.sanitized(includePatterns)
        self.excludePatterns = Self.sanitized(excludePatterns)
    }

    /// パターンを初期化時に正規化する: 前後空白のtrim・空/空白のみの除去・重複除去(順序維持)。
    /// 空/空白パターンをそのまま保持するとglobが何にも一致せず、インクルード非空と誤判定されて
    /// 「全ファイル除外」になる。除去後は空フィルタ(全ファイル対象)と同一の挙動になる。
    private static func sanitized(_ patterns: [String]) -> [String] {
        var seen = Set<String>()
        return patterns.reduce(into: []) { result, raw in
            let trimmed = raw.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !trimmed.isEmpty, seen.insert(trimmed).inserted else { return }
            result.append(trimmed)
        }
    }

    private var includeGlobs: [GlobPattern] { includePatterns.map(GlobPattern.init) }
    private var excludeGlobs: [GlobPattern] { excludePatterns.map(GlobPattern.init) }

    /// エントリ(ファイル)が比較対象に含まれるべきか。
    /// エクスクルード一致は常に false。インクルード非空なら少なくとも1つが一致すること。
    /// インクルード空ならエクスクルードのみで判定する(従来の除外挙動と同一)。
    public func allowsFile(name: String, relativePath: String) -> Bool {
        if excludeGlobs.contains(where: { $0.matches(name: name, relativePath: relativePath) }) {
            return false
        }
        if includePatterns.isEmpty { return true }
        return includeGlobs.contains(where: { $0.matches(name: name, relativePath: relativePath) })
    }

    /// エントリ(ファイル/ディレクトリ共通)が除外されるか。
    /// ディレクトリはこれが true のときサブツリーごと枝刈りされる。
    /// インクルードはディレクトリを刈らない(ネストしたファイルへ降下するため)。
    public func isExcluded(name: String, relativePath: String) -> Bool {
        excludeGlobs.contains(where: { $0.matches(name: name, relativePath: relativePath) })
    }

    /// ルール列へ平坦化する(レポート・表示用)。
    public var rules: [FilterRule] {
        includePatterns.map { FilterRule(kind: .include, pattern: $0) }
            + excludePatterns.map { FilterRule(kind: .exclude, pattern: $0) }
    }
}

/// 組み込みフィルタプリセット。安全な除外パターンの組を提供する。
/// 常にopt-inで、既定の比較には何も追加しない(明示指定されたときだけ展開される)。
public struct FilterPreset: Sendable, Hashable, Identifiable {
    /// 一意なID。CLI(--preset)・UI(永続化)で共用する。
    public let id: String
    /// 表示名(UI用)
    public let displayName: String
    /// このプリセットが展開する除外globパターン
    public let excludePatterns: [String]

    public init(id: String, displayName: String, excludePatterns: [String]) {
        self.id = id
        self.displayName = displayName
        self.excludePatterns = excludePatterns
    }

    /// ソース管理系のメタデータ(バージョン管理ディレクトリ)
    public static let sourceControl = FilterPreset(
        id: "source-control",
        displayName: "ソース管理 (Source Control)",
        excludePatterns: [".git", ".svn", ".hg", ".bzr", "_darcs", "CVS"]
    )

    /// ビルド成果物・生成物
    public static let buildArtifacts = FilterPreset(
        id: "build-artifacts",
        displayName: "ビルド成果物 (Build Artifacts)",
        excludePatterns: [
            "build", "dist", ".build", ".gradle", "DerivedData", "Pods",
            "*.o", "*.obj", "*.class", "*.pyc", "__pycache__",
            "*.dylib", "*.so", "*.dll", "*.exe",
        ]
    )

    /// Web / Node.js の依存・キャッシュ・圧縮物
    public static let webNodeJS = FilterPreset(
        id: "web-nodejs",
        displayName: "Web / Node.js",
        excludePatterns: [
            "node_modules", "bower_components", ".cache", ".parcel-cache",
            "*.min.js", "*.min.css", "coverage",
        ]
    )

    /// macOS のメタデータ・一時ファイル
    public static let macMetadata = FilterPreset(
        id: "macos-metadata",
        displayName: "macOS メタデータ",
        excludePatterns: [
            ".DS_Store", "._*",
            ".Spotlight-V100", ".Trashes", ".fseventsd", ".DocumentRevisions-V100", ".AppleDouble",
        ]
    )

    /// 全プリセット(表示順)
    public static let all: [FilterPreset] = [
        .sourceControl, .buildArtifacts, .webNodeJS, .macMetadata,
    ]

    /// IDからプリセットを引く。未知のIDは nil。
    public static func preset(id: String) -> FilterPreset? {
        all.first { $0.id == id }
    }
}
