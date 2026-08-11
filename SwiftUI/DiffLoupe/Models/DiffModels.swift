// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: フォルダ比較の結果を表すモデル群。分類ステータス・エントリ・ツリーノードを定義する

import Foundation

/// ファイル/ディレクトリの分類ステータス(SPEC.md §2.4)
public enum DiffStatus: String, Sendable, Hashable {
    case identical   // 内容一致
    case different   // 内容相違・種別不一致・(ディレクトリは)差分を含む
    case leftOnly    // 左フォルダのみに存在
    case rightOnly   // 右フォルダのみに存在
    case error       // 読み込み不可
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

/// 走査で得た1エントリのメタデータ
struct FileEntryMeta: Sendable {
    var url: URL
    var isDirectory: Bool
    var size: Int64      // ディレクトリ・取得不能時は -1
    var isSymlink: Bool
    var modificationDate: Date? = nil
    var permissions: Int? = nil      // POSIXパーミッションビット
    var ownerID: Int? = nil
    var groupID: Int? = nil
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
