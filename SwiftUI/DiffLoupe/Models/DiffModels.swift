// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: フォルダ比較の結果を表すモデル群。分類ステータス・エントリ・ツリーノードを定義する。
// 実装は Rust コア(diffloupe-ffi)への薄いファサード。UniFFI 生成バインディング
// (RustCore/bindings/diffloupe.swift)の Ffi* 型と相互変換する。

import Foundation

/// ファイル/ディレクトリの分類ステータス(SPEC.md §2.4)
public enum DiffStatus: String, Sendable, Hashable {
    case identical   // 内容一致
    case different   // 内容相違・種別不一致・(ディレクトリは)差分を含む
    case leftOnly    // 左フォルダのみに存在
    case rightOnly   // 右フォルダのみに存在
    case error       // 読み込み不可

    init(_ ffi: FfiDiffStatus) {
        switch ffi {
        case .identical: self = .identical
        case .different: self = .different
        case .leftOnly: self = .leftOnly
        case .rightOnly: self = .rightOnly
        case .error: self = .error
        }
    }

    var ffi: FfiDiffStatus {
        switch self {
        case .identical: return .identical
        case .different: return .different
        case .leftOnly: return .leftOnly
        case .rightOnly: return .rightOnly
        case .error: return .error
        }
    }
}

/// POSIX基本メタデータ(st_modeのパーミッションビットとUID/GID)。取得不能時は各値が nil
public struct POSIXBasicMetadata: Sendable, Hashable {
    public let permissions: Int?      // パーミッションビット(0o7777 相当)
    public let ownerID: Int?
    public let groupID: Int?

    public init(permissions: Int?, ownerID: Int?, groupID: Int?) {
        self.permissions = permissions
        self.ownerID = ownerID
        self.groupID = groupID
    }
}

/// 片側のみのファイルが他方の「内容同一」ファイルへのリネーム/移動候補であることを表す。
/// DiffStatus(左のみ/右のみ)は変更せず、UI表示用の付加情報として扱う(SPEC §2.3 との整合)。
public struct RenameCandidate: Sendable, Hashable {
    /// 対向側で同内容のファイルの相対パス(重複時はソート順の先頭)
    public let counterpartPath: String
    /// 1対1の対応(内容が同一起のものが左右各1件ずつ)か。
    /// false は「同内容の候補が複数存在し、この対応は確定ではない」ことを示す
    public let isUniqueMatch: Bool
    /// 対向側で同内容を持つファイルの件数(1対1なら常に1)
    public let totalMatches: Int

    public init(counterpartPath: String, isUniqueMatch: Bool, totalMatches: Int) {
        self.counterpartPath = counterpartPath
        self.isUniqueMatch = isUniqueMatch
        self.totalMatches = totalMatches
    }
}

/// 相対パスで突き合わせた比較結果1件
public struct ComparedEntry: Sendable, Identifiable, Hashable {
    public let relativePath: String   // NFC正規化済み・"/"区切り
    public let name: String
    public let isDirectory: Bool
    public let status: DiffStatus
    public let leftURL: URL?
    public let rightURL: URL?
    public let leftSize: Int64
    public let rightSize: Int64
    public let errorMessage: String?
    /// 変更日時(contentModificationDate)。取得不能・ディレクトリは nil もありうる
    public let leftModificationDate: Date?
    public let rightModificationDate: Date?
    /// POSIX基本メタデータ(パーミッション・UID/GID)。可能なら収集する
    public let leftPOSIX: POSIXBasicMetadata?
    public let rightPOSIX: POSIXBasicMetadata?
    /// リネーム/移動候補(detectRenamesオプション有効時のみ、片側のみファイルに設定される)
    public let renameCandidate: RenameCandidate?

    public var id: String { relativePath }
}

/// ツリー表示用ノード(ディレクトリは子孫の集約ステータスを持つ)
public struct DiffNode: Identifiable, Hashable, Sendable {
    public let id: String          // 相対パス(ルートは "")
    public let name: String
    public let isDirectory: Bool
    public let status: DiffStatus
    public var children: [DiffNode]?   // ファイルは nil
}

/// 比較全体の結果
public struct ComparisonResult: Sendable {
    public struct Summary: Sendable, Equatable {
        public var identicalFiles = 0
        public var differentFiles = 0
        public var leftOnlyFiles = 0
        public var rightOnlyFiles = 0
        public var errorFiles = 0
        public var totalFiles: Int {
            identicalFiles + differentFiles + leftOnlyFiles + rightOnlyFiles + errorFiles
        }
    }

    public let leftRoot: URL
    public let rightRoot: URL
    public let entries: [ComparedEntry]              // 相対パス昇順
    public let entriesByPath: [String: ComparedEntry]
    public let rootNode: DiffNode
    public let summary: Summary
}

/// 比較処理の進捗(SPEC.md §2.5)
public enum ComparisonProgress: Sendable {
    case scanning(found: Int)
    case comparing(done: Int, total: Int)
    /// リネーム/移動候補検出(片側のみファイルの内容ハッシュ突き合わせ)。有効時のみ通知される
    case renames(done: Int, total: Int)
}

// MARK: - FFI 変換(Rust コア ↔ Swift モデル)

extension POSIXBasicMetadata {
    init(_ ffi: FfiPosixBasicMetadata) {
        self.init(
            permissions: ffi.permissions.map { Int($0) },
            ownerID: ffi.ownerId.map { Int($0) },
            groupID: ffi.groupId.map { Int($0) }
        )
    }

    var ffi: FfiPosixBasicMetadata {
        FfiPosixBasicMetadata(
            permissions: permissions.map { UInt32($0) },
            ownerId: ownerID.map { UInt32($0) },
            groupId: groupID.map { UInt32($0) }
        )
    }
}

extension RenameCandidate {
    init(_ ffi: FfiRenameCandidate) {
        self.init(
            counterpartPath: ffi.counterpartPath,
            isUniqueMatch: ffi.isUniqueMatch,
            totalMatches: Int(ffi.totalMatches)
        )
    }

    var ffi: FfiRenameCandidate {
        FfiRenameCandidate(
            counterpartPath: counterpartPath,
            isUniqueMatch: isUniqueMatch,
            totalMatches: UInt64(totalMatches)
        )
    }
}

extension ComparedEntry {
    init(_ ffi: FfiComparedEntry) {
        self.init(
            relativePath: ffi.relativePath,
            name: ffi.name,
            isDirectory: ffi.isDirectory,
            status: DiffStatus(ffi.status),
            leftURL: ffi.leftPath.map { URL(fileURLWithPath: $0, isDirectory: ffi.isDirectory) },
            rightURL: ffi.rightPath.map { URL(fileURLWithPath: $0, isDirectory: ffi.isDirectory) },
            leftSize: ffi.leftSize,
            rightSize: ffi.rightSize,
            errorMessage: ffi.errorMessage,
            leftModificationDate: ffi.leftModificationDate.map { Date(timeIntervalSince1970: $0) },
            rightModificationDate: ffi.rightModificationDate.map { Date(timeIntervalSince1970: $0) },
            leftPOSIX: ffi.leftPosix.map { POSIXBasicMetadata($0) },
            rightPOSIX: ffi.rightPosix.map { POSIXBasicMetadata($0) },
            renameCandidate: ffi.renameCandidate.map { RenameCandidate($0) }
        )
    }

    var ffi: FfiComparedEntry {
        FfiComparedEntry(
            relativePath: relativePath,
            name: name,
            isDirectory: isDirectory,
            status: status.ffi,
            leftPath: leftURL?.path,
            rightPath: rightURL?.path,
            leftSize: leftSize,
            rightSize: rightSize,
            errorMessage: errorMessage,
            leftModificationDate: leftModificationDate?.timeIntervalSince1970,
            rightModificationDate: rightModificationDate?.timeIntervalSince1970,
            leftPosix: leftPOSIX?.ffi,
            rightPosix: rightPOSIX?.ffi,
            renameCandidate: renameCandidate?.ffi
        )
    }
}

extension DiffNode {
    init(_ ffi: FfiDiffNode) {
        self.init(
            id: ffi.id,
            name: ffi.name,
            isDirectory: ffi.isDirectory,
            status: DiffStatus(ffi.status),
            children: ffi.children?.map { DiffNode($0) }
        )
    }

    var ffi: FfiDiffNode {
        FfiDiffNode(
            id: id,
            name: name,
            isDirectory: isDirectory,
            status: status.ffi,
            children: children?.map { $0.ffi }
        )
    }
}

extension DiffNode {
    /// 圧縮ツリー(id/name は対応エントリから復元)から構築する
    init(_ ffi: FfiTreeNode, entries: [FfiComparedEntry]) {
        let id: String
        let name: String
        if ffi.entryIndex == UInt32.max {
            id = ffi.id
            name = ffi.name
        } else {
            let e = entries[Int(ffi.entryIndex)]
            id = e.relativePath
            name = ffi.name.isEmpty ? e.name : ffi.name
        }
        self.init(
            id: id,
            name: name,
            isDirectory: ffi.isDirectory,
            status: DiffStatus(ffi.status),
            children: ffi.children?.map { DiffNode($0, entries: entries) }
        )
    }
}

extension ComparisonResult {
    /// Rust 側の結果を受けて Swift モデルへ変換する。
    /// ルートは呼び出し時の URL を使う(FFI 側は canonical 化したパスを返すため)。
    init(_ ffi: FfiComparisonResult, leftRoot: URL, rightRoot: URL) {
        let entries = ffi.entries.map { ComparedEntry($0) }
        self.init(
            leftRoot: leftRoot,
            rightRoot: rightRoot,
            entries: entries,
            entriesByPath: Dictionary(uniqueKeysWithValues: entries.map { ($0.relativePath, $0) }),
            rootNode: DiffNode(ffi.rootNode, entries: ffi.entries),
            summary: Summary(ffi.summary)
        )
    }

    /// 手組み結果(テスト等)を FFI レコードへ戻す。レポート/コピー計画で使用する。
    var ffi: FfiComparisonResult {
        FfiComparisonResult(
            leftRoot: leftRoot.path,
            rightRoot: rightRoot.path,
            entries: entries.map { $0.ffi },
            // Swift → Rust 方向ではツリーは使われない(Rust 側が entries から再構築する)
            rootNode: FfiTreeNode(
                entryIndex: UInt32.max, id: rootNode.id, name: rootNode.name,
                isDirectory: rootNode.isDirectory, status: rootNode.status.ffi, children: nil),
            summary: summary.ffi
        )
    }
}

extension ComparisonResult.Summary {
    init(_ ffi: FfiComparisonSummary) {
        self.init(
            identicalFiles: Int(ffi.identicalFiles),
            differentFiles: Int(ffi.differentFiles),
            leftOnlyFiles: Int(ffi.leftOnlyFiles),
            rightOnlyFiles: Int(ffi.rightOnlyFiles),
            errorFiles: Int(ffi.errorFiles)
        )
    }

    var ffi: FfiComparisonSummary {
        FfiComparisonSummary(
            identicalFiles: UInt64(identicalFiles),
            differentFiles: UInt64(differentFiles),
            leftOnlyFiles: UInt64(leftOnlyFiles),
            rightOnlyFiles: UInt64(rightOnlyFiles),
            errorFiles: UInt64(errorFiles)
        )
    }
}

extension ComparisonProgress {
    init(_ ffi: FfiComparisonProgress) {
        switch ffi {
        case .scanning(let found): self = .scanning(found: Int(found))
        case .comparing(let done, let total): self = .comparing(done: Int(done), total: Int(total))
        case .renames(let done, let total): self = .renames(done: Int(done), total: Int(total))
        }
    }
}
