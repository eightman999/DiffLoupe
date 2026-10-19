// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: 変更行(modified)の行内差分を計算する純粋関数群。左右の元テキストを保持したまま、
// 文字レベルの変更範囲(文字インデックス)を返し、UI側で強調表示する(SPEC §2.6 拡張)。
// 差分計算・ハイライトのロジックをUIから分離し、テスト可能にする。

import Foundation

/// 行内差分の結果。変更行の左右それぞれの「変更された文字範囲」を文字インデックスで表す。
/// leftRanges は左テキスト内の削除/変更領域、rightRanges は右テキスト内の挿入/変更領域。
public struct InlineHighlight: Equatable, Sendable {
    public let leftRanges: [Range<Int>]
    public let rightRanges: [Range<Int>]
}

public enum InlineDiff {

    /// 行内差分を計算する対象の左右合計文字数の上限。これを超える行は nil を返す
    /// (LCS動的計画法の計算量・メモリ保護。ハイライトが無くても表示自体は壊れない)。
    public static let maxCharacters = 1200

    /// 変更行の左右テキストの差分範囲を計算する。
    /// - Parameters:
    ///   - left: 左側の元テキスト(nil でも安全)
    ///   - right: 右側の元テキスト(nil でも安全)
    ///   - caseInsensitive: 大文字小文字を無視して比較するか(ContentComparison.ignoreCase に連動)
    /// - Returns: 変更範囲。両側が同一・片側 nil・空同士・文字数上限超過の場合は nil
    public static func highlight(left: String?, right: String?, caseInsensitive: Bool) -> InlineHighlight? {
        guard let left, let right else { return nil }
        guard !left.isEmpty || !right.isEmpty else { return nil }

        let leftChars = Array(left)
        let rightChars = Array(right)
        guard leftChars.count + rightChars.count <= maxCharacters else { return nil }

        if leftChars == rightChars { return nil }

        let (leftRanges, rightRanges) = changedRanges(
            leftChars, rightChars, caseInsensitive: caseInsensitive)
        if leftRanges.isEmpty && rightRanges.isEmpty { return nil }
        return InlineHighlight(leftRanges: leftRanges, rightRanges: rightRanges)
    }

    /// 行単位の差分結果から、変更行(modified)だけの行内ハイライトをまとめて計算する。
    /// 変更が無い行・ハイライト対象外の行は結果に含まれない。
    public static func highlightsForRows(_ rows: [DiffRow], caseInsensitive: Bool) -> [Int: InlineHighlight] {
        var result: [Int: InlineHighlight] = [:]
        for row in rows where row.kind == .modified {
            if let highlight = highlight(left: row.leftText, right: row.rightText, caseInsensitive: caseInsensitive) {
                result[row.id] = highlight
            }
        }
        return result
    }

    // MARK: - LCS(後ろ向き動的計画法)による変更範囲の算出

    private static func changedRanges(
        _ left: [Character], _ right: [Character],
        caseInsensitive: Bool
    ) -> ([Range<Int>], [Range<Int>]) {
        let n = left.count
        let m = right.count
        guard n > 0, m > 0 else {
            if n == 0, m == 0 { return ([], []) }
            if n == 0 { return ([], [0..<m]) }
            return ([0..<n], [])
        }

        // LCS長テーブル(後ろ向き)。left[i...] と right[j...] の最長共通部分列長
        var dp = Array(repeating: Array(repeating: 0, count: m + 1), count: n + 1)
        for i in stride(from: n - 1, through: 0, by: -1) {
            for j in stride(from: m - 1, through: 0, by: -1) {
                if charsEqual(left[i], right[j], caseInsensitive: caseInsensitive) {
                    dp[i][j] = dp[i + 1][j + 1] + 1
                } else {
                    dp[i][j] = max(dp[i + 1][j], dp[i][j + 1])
                }
            }
        }

        var leftRanges: [Range<Int>] = []
        var rightRanges: [Range<Int>] = []
        var leftStart: Int?
        var rightStart: Int?

        func closeLeft(_ end: Int) {
            if let start = leftStart {
                if start < end { leftRanges.append(start..<end) }
                leftStart = nil
            }
        }
        func closeRight(_ end: Int) {
            if let start = rightStart {
                if start < end { rightRanges.append(start..<end) }
                rightStart = nil
            }
        }

        // バックトラック: 一致する文字のペアはスキップし、不一致の連続領域を変更範囲として確定する
        var i = 0
        var j = 0
        while i < n || j < m {
            if i < n, j < m, charsEqual(left[i], right[j], caseInsensitive: caseInsensitive) {
                closeLeft(i)
                closeRight(j)
                i += 1
                j += 1
            } else if j < m, (i >= n || dp[i][j + 1] >= dp[i + 1][j]) {
                // 右側に挿入された文字
                if rightStart == nil { rightStart = j }
                j += 1
            } else {
                // 左側から削除された文字
                if leftStart == nil { leftStart = i }
                i += 1
            }
        }
        closeLeft(n)
        closeRight(m)
        return (leftRanges, rightRanges)
    }

    private static func charsEqual(_ a: Character, _ b: Character, caseInsensitive: Bool) -> Bool {
        guard caseInsensitive else { return a == b }
        return String(a).lowercased() == String(b).lowercased()
    }
}
