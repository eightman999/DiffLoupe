// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: 変更行(modified)の行内差分を計算する純粋関数群。左右の元テキストを保持したまま、
// 文字レベルの変更範囲(文字インデックス)を返し、UI側で強調表示する(SPEC §2.6 拡張)。
// 実装は Rust コア(diffloupe-ffi)への薄いファサード。

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
        diffloupeInlineHighlight(left: left, right: right, caseInsensitive: caseInsensitive)
            .map { InlineHighlight($0) }
    }

    /// 行単位の差分結果から、変更行(modified)だけの行内ハイライトをまとめて計算する。
    /// 変更が無い行・ハイライト対象外の行は結果に含まれない。
    public static func highlightsForRows(_ rows: [DiffRow], caseInsensitive: Bool) -> [Int: InlineHighlight] {
        var result: [Int: InlineHighlight] = [:]
        for pair in diffloupeInlineHighlightsForRows(
            rows: rows.map { $0.ffi }, caseInsensitive: caseInsensitive) {
            result[Int(pair.rowId)] = InlineHighlight(pair.highlight)
        }
        return result
    }
}

// MARK: - FFI 変換

extension InlineHighlight {
    init(_ ffi: FfiInlineHighlight) {
        self.init(
            leftRanges: ffi.leftRanges.map { Int($0.start)..<Int($0.end) },
            rightRanges: ffi.rightRanges.map { Int($0.start)..<Int($0.end) }
        )
    }
}
