// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: CollectionDifference(Myers法)による行単位差分と横並び表示用の行整列(SPEC.md §2.6)

import Foundation

public enum DiffRowKind: Sendable, Hashable {
    case context    // 一致行
    case added      // 右にのみ存在(追加)
    case removed    // 左にのみ存在(削除)
    case modified   // 変更(削除と追加のペア)
}

/// 横並び表示の1行。左右いずれかが nil のときは空セルを表す
public struct DiffRow: Identifiable, Sendable, Hashable {
    public let id: Int
    public let kind: DiffRowKind
    public let leftNumber: Int?
    public let leftText: String?
    public let rightNumber: Int?
    public let rightText: String?
}

public enum LineDiff {

    /// 左右合計がこの行数を超える場合は差分計算を行わない(SPEC.md §2.6)
    public static let maxTotalLines = 40_000

    /// LF基準で行分割する(CRLF対策)。
    /// Swiftでは"\r\n"が1つのCharacterになるため、先にLFへ正規化してから分割する
    public static func splitLines(_ text: String) -> [String] {
        let normalized = text.contains("\r\n")
            ? text.replacingOccurrences(of: "\r\n", with: "\n")
            : text
        return normalized
            .split(separator: "\n", omittingEmptySubsequences: false)
            .map(String.init)
    }

    /// 行末CRを保持したまま行分割する(表示用)。
    /// Swiftでは"\r\n"が1つのCharacterになるため、CRLF合字を「行末CR + 改行」に分解してから
    /// LFで分割し、各行の末尾にCRを残す。ignoreCase等の正規化設定でもユーザーが元の改行差を確認できるようにする。
    public static func splitLinesKeepingCR(_ text: String) -> [String] {
        var lines: [String] = []
        var current = ""
        for ch in text {
            if ch == "\n" {
                lines.append(current)
                current = ""
            } else if ch == "\r\n" {
                // CRLF合字: 行末CRを残して改行を確定する
                current.append("\r")
                lines.append(current)
                current = ""
            } else {
                current.append(ch)
            }
        }
        lines.append(current)
        return lines
    }

    /// 横並び表示用の整列済み行リストを返す。行数上限超過時は nil。
    /// 差分計算には contentComparison で正規化した行を使うが、
    /// DiffRow の leftText/rightText には正規化前の元テキストを入れる
    /// (大文字・空白・行末CRなど、ユーザーが実際のファイル内容を確認できるようにする)。
    /// contentComparison が厳密(既定)の場合は既存挙動と同一。
    public static func rows(
        leftText: String,
        rightText: String,
        contentComparison: ContentComparison = ContentComparison()
    ) -> [DiffRow]? {
        // 表示用 = 元テキスト(行末CRを保持)。差分計算用 = 正規化済み行。
        let displayLeft = splitLinesKeepingCR(leftText)
        let displayRight = splitLinesKeepingCR(rightText)
        guard displayLeft.count + displayRight.count <= maxTotalLines else { return nil }

        let compareLeft: [String]
        let compareRight: [String]
        if contentComparison.isStrict {
            compareLeft = splitLines(leftText)
            compareRight = splitLines(rightText)
        } else {
            compareLeft = contentComparison.normalizedText(leftText)
                .split(separator: "\n", omittingEmptySubsequences: false).map(String.init)
            compareRight = contentComparison.normalizedText(rightText)
                .split(separator: "\n", omittingEmptySubsequences: false).map(String.init)
        }

        // 改行コード正規化(\r→\n)で行数が変わると表示行と差分計算行の対応が取れないため省略する
        guard compareLeft.count <= maxTotalLines, compareRight.count <= maxTotalLines,
              compareLeft.count == displayLeft.count, compareRight.count == displayRight.count else {
            return nil
        }

        let difference = compareRight.difference(from: compareLeft)
        var removedOffsets = Set<Int>()
        var insertedOffsets = Set<Int>()
        for change in difference {
            switch change {
            case .remove(let offset, _, _): removedOffsets.insert(offset)
            case .insert(let offset, _, _): insertedOffsets.insert(offset)
            }
        }

        var rows: [DiffRow] = []
        rows.reserveCapacity(max(displayLeft.count, displayRight.count))
        var i = 0
        var j = 0

        func append(_ kind: DiffRowKind, leftIndex: Int?, rightIndex: Int?) {
            rows.append(DiffRow(
                id: rows.count,
                kind: kind,
                leftNumber: leftIndex.map { $0 + 1 },
                leftText: leftIndex.map { displayLeft[$0] },
                rightNumber: rightIndex.map { $0 + 1 },
                rightText: rightIndex.map { displayRight[$0] }
            ))
        }

        while i < displayLeft.count || j < displayRight.count {
            if i < displayLeft.count, j < displayRight.count,
               !removedOffsets.contains(i), !insertedOffsets.contains(j) {
                append(.context, leftIndex: i, rightIndex: j)
                i += 1
                j += 1
                continue
            }

            // 連続する削除ブロックと挿入ブロックを集め、index順にペアリングして「変更」とする
            var removedRun: [Int] = []
            while i < displayLeft.count, removedOffsets.contains(i) {
                removedRun.append(i)
                i += 1
            }
            var insertedRun: [Int] = []
            while j < displayRight.count, insertedOffsets.contains(j) {
                insertedRun.append(j)
                j += 1
            }

            if removedRun.isEmpty && insertedRun.isEmpty {
                // 不変条件が崩れた場合の安全弁(無限ループ防止)
                if i < displayLeft.count, j < displayRight.count {
                    append(.context, leftIndex: i, rightIndex: j)
                    i += 1
                    j += 1
                } else if i < displayLeft.count {
                    append(.removed, leftIndex: i, rightIndex: nil)
                    i += 1
                } else if j < displayRight.count {
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
