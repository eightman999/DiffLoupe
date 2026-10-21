// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: 比較結果(ComparisonResult)から「コピー計画」を生成する純粋ロジック。
// ファイルIO・コピー・削除・上書きは一切行わない(プレビュー専用)。
// 片側のみの項目はコピー候補、両側同一は対象外、差分ファイルはconflict、
// 反対側のみの項目は削除しないためpreservedDestination、比較エラーはblockedとする。
// 実装は Rust コア(diffloupe-ffi)への薄いファサード。

import Foundation

/// コピー計画の方向。sourceが「コピー元」、destinationが「コピー先」になる。
public enum SyncDirection: String, CaseIterable, Identifiable, Sendable, Hashable {
    case leftToRight
    case rightToLeft

    public var id: String { rawValue }

    public var displayName: String {
        switch self {
        case .leftToRight: return "左 → 右"
        case .rightToLeft: return "右 → 左"
        }
    }

    /// コピー元側(存在するURLを返す側)。nilは作業ツリー未設定時の対応(純粋関数のため判定のみ)。
    var sourceIsLeft: Bool {
        switch self {
        case .leftToRight: return true
        case .rightToLeft: return false
        }
    }
}

/// コピー計画の1項目のアクション種別。
public enum SyncAction: String, Sendable, Hashable {
    case copyFile
    case createDirectory
    case conflict
    case preservedDestination
    case blocked

    public var displayName: String {
        switch self {
        case .copyFile: return "コピー"
        case .createDirectory: return "ディレクトリ作成"
        case .conflict: return "差分(上書き候補)"
        case .preservedDestination: return "保持"
        case .blocked: return "エラー"
        }
    }
}

/// コピー計画の1項目。sourceはコピー元、destinationはコピー先。
/// preservedDestinationはsourceが nil(反対側の項目は削除しない)。
public struct SyncItem: Sendable, Equatable, Hashable, Identifiable {
    public let relativePath: String
    public let action: SyncAction
    public let source: URL?
    public let destination: URL?
    public let bytes: Int64
    public let reason: String

    public var id: String { "\(directionKey):\(relativePath)" }

    private let directionKey: String

    public init(
        direction: SyncDirection,
        relativePath: String,
        action: SyncAction,
        source: URL?,
        destination: URL?,
        bytes: Int64,
        reason: String
    ) {
        self.directionKey = direction.rawValue
        self.relativePath = relativePath
        self.action = action
        self.source = source
        self.destination = destination
        self.bytes = bytes
        self.reason = reason
    }
}

/// コピー計画。ComparisonResultから生成される読み取り専用のプレビュー。
/// 計画の適用(実際のコピー・削除・上書き)は本モデルでは提供しない。
public struct SyncPlan: Sendable, Equatable {
    public let direction: SyncDirection
    public let sourceRoot: URL
    public let destinationRoot: URL
    public let items: [SyncItem]
    public let summary: Summary

    public struct Summary: Sendable, Equatable {
        public var copyFiles = 0
        public var createDirectories = 0
        public var conflicts = 0
        public var preservedDestinations = 0
        public var blocked = 0
        public var totalBytes: Int64 = 0

        public var totalItems: Int {
            copyFiles + createDirectories + conflicts + preservedDestinations + blocked
        }
    }

    /// 比較結果からコピー計画を生成する純粋関数。ファイルIOは行わない。
    /// 入力のエントリは相対パス昇順(ComparisonResultの契約)に従い、順序を保持する。
    public static func make(from result: ComparisonResult, direction: SyncDirection) -> SyncPlan {
        let ffi = diffloupeSyncPlanMake(result: result.ffi, direction: direction.ffi)
        return SyncPlan(ffi)
    }
}

// MARK: - FFI 変換

extension SyncDirection {
    var ffi: FfiSyncDirection {
        switch self {
        case .leftToRight: return .leftToRight
        case .rightToLeft: return .rightToLeft
        }
    }
}

extension SyncAction {
    init(_ ffi: FfiSyncAction) {
        switch ffi {
        case .copyFile: self = .copyFile
        case .createDirectory: self = .createDirectory
        case .conflict: self = .conflict
        case .preservedDestination: self = .preservedDestination
        case .blocked: self = .blocked
        }
    }
}

extension SyncPlan {
    init(_ ffi: FfiSyncPlan) {
        self.init(
            direction: SyncDirection(rawValue: ffi.direction == .leftToRight ? "leftToRight" : "rightToLeft")!,
            sourceRoot: URL(fileURLWithPath: ffi.sourceRoot),
            destinationRoot: URL(fileURLWithPath: ffi.destinationRoot),
            items: ffi.items.map { item in
                SyncItem(
                    direction: item.directionKey == "leftToRight" ? .leftToRight : .rightToLeft,
                    relativePath: item.relativePath,
                    action: SyncAction(item.action),
                    source: item.source.map { URL(fileURLWithPath: $0) },
                    destination: item.destination.map { URL(fileURLWithPath: $0) },
                    bytes: item.bytes,
                    reason: item.reason
                )
            },
            summary: Summary(
                copyFiles: Int(ffi.summary.copyFiles),
                createDirectories: Int(ffi.summary.createDirectories),
                conflicts: Int(ffi.summary.conflicts),
                preservedDestinations: Int(ffi.summary.preservedDestinations),
                blocked: Int(ffi.summary.blocked),
                totalBytes: ffi.summary.totalBytes
            )
        )
    }
}
