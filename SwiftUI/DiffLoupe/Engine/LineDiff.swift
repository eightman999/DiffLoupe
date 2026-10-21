// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: Myers法による行単位差分と横並び表示用の行整列(SPEC.md §2.6)。
// 実装は Rust コア(diffloupe-ffi)への薄いファサード。

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
        diffloupeSplitLines(text: text)
    }

    /// 行末CRを保持したまま行分割する(表示用)。
    /// ignoreCase等の正規化設定でもユーザーが元の改行差を確認できるようにする。
    public static func splitLinesKeepingCR(_ text: String) -> [String] {
        diffloupeSplitLinesKeepingCr(text: text)
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
        diffloupeLineDiffRows(
            leftText: leftText,
            rightText: rightText,
            contentComparison: contentComparison.ffi
        )?.map { DiffRow($0) }
    }
}

// MARK: - FFI 変換

extension DiffRowKind {
    init(_ ffi: FfiDiffRowKind) {
        switch ffi {
        case .context: self = .context
        case .added: self = .added
        case .removed: self = .removed
        case .modified: self = .modified
        }
    }

    var ffi: FfiDiffRowKind {
        switch self {
        case .context: return .context
        case .added: return .added
        case .removed: return .removed
        case .modified: return .modified
        }
    }
}

extension DiffRow {
    init(_ ffi: FfiDiffRow) {
        self.init(
            id: Int(ffi.id),
            kind: DiffRowKind(ffi.kind),
            leftNumber: ffi.leftNumber.map { Int($0) },
            leftText: ffi.leftText,
            rightNumber: ffi.rightNumber.map { Int($0) },
            rightText: ffi.rightText
        )
    }

    var ffi: FfiDiffRow {
        FfiDiffRow(
            id: UInt64(id),
            kind: kind.ffi,
            leftNumber: leftNumber.map { UInt64($0) },
            leftText: leftText,
            rightNumber: rightNumber.map { UInt64($0) },
            rightText: rightText
        )
    }
}
