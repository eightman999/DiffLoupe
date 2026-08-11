// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: 比較結果(ComparisonResult)から「コピー計画」を生成する純粋ロジック。
// ファイルIO・コピー・削除・上書きは一切行わない(プレビュー専用)。
// 片側のみの項目はコピー候補、両側同一は対象外、差分ファイルはconflict、
// 反対側のみの項目は削除しないためpreservedDestination、比較エラーはblockedとする。

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

    var displayName: String {
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
        let sourceIsLeft = direction.sourceIsLeft
        let sourceRoot = sourceIsLeft ? result.leftRoot : result.rightRoot
        let destinationRoot = sourceIsLeft ? result.rightRoot : result.leftRoot

        var items: [SyncItem] = []
        var copyFiles = 0
        var createDirectories = 0
        var conflicts = 0
        var preservedDestinations = 0
        var blocked = 0
        var totalBytes: Int64 = 0

        for entry in result.entries {
            switch entry.status {
            case .identical:
                continue
            case .leftOnly, .rightOnly:
                let existsOnSource =
                    (entry.status == .leftOnly && sourceIsLeft)
                    || (entry.status == .rightOnly && !sourceIsLeft)
                if existsOnSource {
                    let sourceURL = sourceIsLeft ? entry.leftURL : entry.rightURL
                    if entry.isDirectory {
                        items.append(SyncItem(
                            direction: direction,
                            relativePath: entry.relativePath,
                            action: .createDirectory,
                            source: sourceURL,
                            destination: destinationRoot
                                .appendingPathComponent(entry.relativePath),
                            bytes: 0,
                            reason: "ディレクトリの作成が必要"
                        ))
                        createDirectories += 1
                    } else {
                        let byteCount = max(entry.leftSize, entry.rightSize)
                        items.append(SyncItem(
                            direction: direction,
                            relativePath: entry.relativePath,
                            action: .copyFile,
                            source: sourceURL,
                            destination: destinationRoot
                                .appendingPathComponent(entry.relativePath),
                            bytes: byteCount,
                            reason: "コピーが必要"
                        ))
                        copyFiles += 1
                        totalBytes += byteCount
                    }
                } else {
                    // 反対側のみの項目は削除しない → 保持対象として記録する
                    let destinationURL = sourceIsLeft ? entry.rightURL : entry.leftURL
                    items.append(SyncItem(
                        direction: direction,
                        relativePath: entry.relativePath,
                        action: .preservedDestination,
                        source: nil,
                        destination: destinationURL,
                        bytes: 0,
                        reason: "反対側のみに存在するため削除せず保持"
                    ))
                    preservedDestinations += 1
                }
            case .different:
                let sourceURL = sourceIsLeft ? entry.leftURL : entry.rightURL
                let destinationURL = sourceIsLeft ? entry.rightURL : entry.leftURL
                items.append(SyncItem(
                    direction: direction,
                    relativePath: entry.relativePath,
                    action: .conflict,
                    source: sourceURL,
                    destination: destinationURL,
                    bytes: 0,
                    reason: "両側に存在し内容が異なるため上書き候補"
                ))
                conflicts += 1
            case .error:
                items.append(SyncItem(
                    direction: direction,
                    relativePath: entry.relativePath,
                    action: .blocked,
                    source: entry.leftURL,
                    destination: entry.rightURL,
                    bytes: 0,
                    reason: entry.errorMessage ?? "比較エラーのため実行不可"
                ))
                blocked += 1
            }
        }

        return SyncPlan(
            direction: direction,
            sourceRoot: sourceRoot,
            destinationRoot: destinationRoot,
            items: items,
            summary: Summary(
                copyFiles: copyFiles,
                createDirectories: createDirectories,
                conflicts: conflicts,
                preservedDestinations: preservedDestinations,
                blocked: blocked,
                totalBytes: totalBytes
            )
        )
    }
}
