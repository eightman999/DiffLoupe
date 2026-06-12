// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: CollectionDifference(Myers法)による行単位差分と横並び表示用の行整列(SPEC.md §2.6)

import Foundation

enum DiffRowKind: Sendable, Hashable {
    case context    // 一致行
    case added      // 右にのみ存在(追加)
    case removed    // 左にのみ存在(削除)
    case modified   // 変更(削除と追加のペア)
}

/// 横並び表示の1行。左右いずれかが nil のときは空セルを表す
struct DiffRow: Identifiable, Sendable, Hashable {
    let id: Int
    let kind: DiffRowKind
    let leftNumber: Int?
    let leftText: String?
    let rightNumber: Int?
    let rightText: String?
}

enum LineDiff {

    /// 左右合計がこの行数を超える場合は差分計算を行わない(SPEC.md §2.6)
    static let maxTotalLines = 40_000

    /// LF基準で行分割する(CRLF対策)。
    /// Swiftでは"\r\n"が1つのCharacterになるため、先にLFへ正規化してから分割する
    static func splitLines(_ text: String) -> [String] {
        let normalized = text.contains("\r\n")
            ? text.replacingOccurrences(of: "\r\n", with: "\n")
            : text
        return normalized
            .split(separator: "\n", omittingEmptySubsequences: false)
            .map(String.init)
    }

    /// 横並び表示用の整列済み行リストを返す。行数上限超過時は nil。
    static func rows(leftText: String, rightText: String) -> [DiffRow]? {
        let left = splitLines(leftText)
        let right = splitLines(rightText)
        guard left.count + right.count <= maxTotalLines else { return nil }

        let difference = right.difference(from: left)
        var removedOffsets = Set<Int>()
        var insertedOffsets = Set<Int>()
        for change in difference {
            switch change {
            case .remove(let offset, _, _): removedOffsets.insert(offset)
            case .insert(let offset, _, _): insertedOffsets.insert(offset)
            }
        }

        var rows: [DiffRow] = []
        rows.reserveCapacity(max(left.count, right.count))
        var i = 0
        var j = 0

        func append(_ kind: DiffRowKind, leftIndex: Int?, rightIndex: Int?) {
            rows.append(DiffRow(
                id: rows.count,
                kind: kind,
                leftNumber: leftIndex.map { $0 + 1 },
                leftText: leftIndex.map { left[$0] },
                rightNumber: rightIndex.map { $0 + 1 },
                rightText: rightIndex.map { right[$0] }
            ))
        }

        while i < left.count || j < right.count {
            if i < left.count, j < right.count,
               !removedOffsets.contains(i), !insertedOffsets.contains(j) {
                append(.context, leftIndex: i, rightIndex: j)
                i += 1
                j += 1
                continue
            }

            // 連続する削除ブロックと挿入ブロックを集め、index順にペアリングして「変更」とする
            var removedRun: [Int] = []
            while i < left.count, removedOffsets.contains(i) {
                removedRun.append(i)
                i += 1
            }
            var insertedRun: [Int] = []
            while j < right.count, insertedOffsets.contains(j) {
                insertedRun.append(j)
                j += 1
            }

            if removedRun.isEmpty && insertedRun.isEmpty {
                // 不変条件が崩れた場合の安全弁(無限ループ防止)
                if i < left.count, j < right.count {
                    append(.context, leftIndex: i, rightIndex: j)
                    i += 1
                    j += 1
                } else if i < left.count {
                    append(.removed, leftIndex: i, rightIndex: nil)
                    i += 1
                } else if j < right.count {
                    append(.added, leftIndex: nil, rightIndex: j)
                    j += 1
                }
                continue
            }

            let pairedCount = min(removedRun.count, insertedRun.count)
            for k in 0..<pairedCount {
                append(.modified, leftIndex: removedRun[k], rightIndex: insertedRun[k])
            }
            for k in pairedCount..<removedRun.count {
                append(.removed, leftIndex: removedRun[k], rightIndex: nil)
            }
            for k in pairedCount..<insertedRun.count {
                append(.added, leftIndex: nil, rightIndex: insertedRun[k])
            }
        }

        return rows
    }
}
