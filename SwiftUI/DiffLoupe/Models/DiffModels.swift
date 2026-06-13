// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: フォルダ比較の結果を表すモデル群。分類ステータス・エントリ・ツリーノードを定義する

import Foundation

/// ファイル/ディレクトリの分類ステータス(SPEC.md §2.4)
enum DiffStatus: String, Sendable, Hashable {
    case identical   // 内容一致
    case different   // 内容相違・種別不一致・(ディレクトリは)差分を含む
    case leftOnly    // 左フォルダのみに存在
    case rightOnly   // 右フォルダのみに存在
    case error       // 読み込み不可
}

/// 走査で得た1エントリのメタデータ
struct FileEntryMeta: Sendable {
    var url: URL
    var isDirectory: Bool
    var size: Int64      // ディレクトリ・取得不能時は -1
    var isSymlink: Bool
}

/// 相対パスで突き合わせた比較結果1件
struct ComparedEntry: Sendable, Identifiable, Hashable {
    let relativePath: String   // NFC正規化済み・"/"区切り
    let name: String
    let isDirectory: Bool
    let status: DiffStatus
    let leftURL: URL?
    let rightURL: URL?
    let leftSize: Int64
    let rightSize: Int64
    let errorMessage: String?

    var id: String { relativePath }
}

/// ツリー表示用ノード(ディレクトリは子孫の集約ステータスを持つ)
struct DiffNode: Identifiable, Hashable, Sendable {
    let id: String          // 相対パス(ルートは "")
    let name: String
    let isDirectory: Bool
    let status: DiffStatus
    var children: [DiffNode]?   // ファイルは nil
}

/// 比較全体の結果
struct ComparisonResult: Sendable {
    struct Summary: Sendable, Equatable {
        var identicalFiles = 0
        var differentFiles = 0
        var leftOnlyFiles = 0
        var rightOnlyFiles = 0
        var errorFiles = 0
        var totalFiles: Int {
            identicalFiles + differentFiles + leftOnlyFiles + rightOnlyFiles + errorFiles
        }
    }

    let leftRoot: URL
    let rightRoot: URL
    let entries: [ComparedEntry]              // 相対パス昇順
    let entriesByPath: [String: ComparedEntry]
    let rootNode: DiffNode
    let summary: Summary
}

/// 比較処理の進捗(SPEC.md §2.5)
enum ComparisonProgress: Sendable {
    case scanning(found: Int)
    case comparing(done: Int, total: Int)
}
