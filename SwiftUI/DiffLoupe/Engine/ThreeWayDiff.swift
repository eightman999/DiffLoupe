// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: 3-wayテキスト差分(base/left/right)の純粋ロジック。読み取り専用で、
// マージ・保存・適用は一切行わない(ファイルIOなし)。
// baseを共通祖先として left/right の変更を判定し、左だけの変更・右だけの変更・
// 左右が同一の変更(bothChangedSame)を自動的に分離し、左右で異なる変更を conflict とする。
// 表示は元の行(行末CR保持)を使い、ContentComparisonの正規化は判定のみに使う。
// 3側合計行数が上限(40,000)を超える場合は truncated=true で安全に中断する。
// 実装は Rust コア(diffloupe-ffi)への薄いファサード。

import Foundation

/// 3-way差分の1行の状態。
public enum ThreeWayDiffStatus: String, Sendable, Hashable {
    /// 3側すべて同一(判定値ベース)
    case unchanged
    /// 左だけ変更(挿入・削除・置換)
    case leftChanged
    /// 右だけ変更(挿入・削除・置換)
    case rightChanged
    /// 左右が同一の変更(正規化後で同一と判定できる)
    case bothChangedSame
    /// 左右が異なる変更(競合)
    case conflict
}

/// 3-way差分の1行。各列は原文の表示値(行末CR保持)で、対応する行が無い列は nil。
/// 行番号は元ファイルの1始まりの行番号(表示行)。
public struct ThreeWayLine: Identifiable, Sendable, Hashable {
    public let id: Int
    public let status: ThreeWayDiffStatus
    public let base: String?
    public let left: String?
    public let right: String?
    public let baseNumber: Int?
    public let leftNumber: Int?
    public let rightNumber: Int?
}

/// 3-way差分の結果。純粋ロジックでファイルIOは行わない(Sendable/Equatable/Hashable)。
public struct ThreeWayDiff: Sendable, Equatable, Hashable {
    /// 3側合計の最大行数。超えた場合は truncated=true で中断する
    public static let maxTotalLines = 40_000

    public let lines: [ThreeWayLine]
    /// 3側合計行数が上限を超えた、または正規化で行数が変わり表示と対応が取れないため中断した
    public let truncated: Bool
    public let unchangedCount: Int
    public let leftChangedCount: Int
    public let rightChangedCount: Int
    public let bothChangedSameCount: Int
    public let conflictCount: Int

    /// 変更のある行の合計(conflict込み)
    public var totalChangedCount: Int {
        leftChangedCount + rightChangedCount + bothChangedSameCount + conflictCount
    }

    /// base/left/rightの3テキストを比較して行モデルを生成する。
    /// - Parameters:
    ///   - base: 共通祖先のテキスト
    ///   - left: 左側のテキスト
    ///   - right: 右側のテキスト
    ///   - contentComparison: 判定用の正規化(表示には影響しない)
    public static func make(
        base: String,
        left: String,
        right: String,
        contentComparison: ContentComparison = ContentComparison()
    ) -> ThreeWayDiff {
        ThreeWayDiff(diffloupeThreeWayMake(
            base: base, left: left, right: right,
            contentComparison: contentComparison.ffi))
    }
}

// MARK: - FFI 変換

extension ThreeWayDiffStatus {
    init(_ ffi: FfiThreeWayDiffStatus) {
        switch ffi {
        case .unchanged: self = .unchanged
        case .leftChanged: self = .leftChanged
        case .rightChanged: self = .rightChanged
        case .bothChangedSame: self = .bothChangedSame
        case .conflict: self = .conflict
        }
    }
}

extension ThreeWayDiff {
    init(_ ffi: FfiThreeWayDiff) {
        self.init(
            lines: ffi.lines.map { l in
                ThreeWayLine(
                    id: Int(l.id),
                    status: ThreeWayDiffStatus(l.status),
                    base: l.base,
                    left: l.left,
                    right: l.right,
                    baseNumber: l.baseNumber.map { Int($0) },
                    leftNumber: l.leftNumber.map { Int($0) },
                    rightNumber: l.rightNumber.map { Int($0) }
                )
            },
            truncated: ffi.truncated,
            unchangedCount: Int(ffi.unchangedCount),
            leftChangedCount: Int(ffi.leftChangedCount),
            rightChangedCount: Int(ffi.rightChangedCount),
            bothChangedSameCount: Int(ffi.bothChangedSameCount),
            conflictCount: Int(ffi.conflictCount)
        )
    }
}
