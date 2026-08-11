// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: 3-wayテキスト差分(base/left/right)の純粋ロジック。読み取り専用で、
// マージ・保存・適用は一切行わない(ファイルIOなし)。
// baseを共通祖先として left/right の変更を判定し、左だけの変更・右だけの変更・
// 左右が同一の変更(bothChangedSame)を自動的に分離し、左右で異なる変更を conflict とする。
// 表示は元の行(行末CR保持)を使い、ContentComparisonの正規化は判定のみに使う。
// 3側合計行数が上限(40,000)を超える場合は truncated=true で安全に中断する。

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
        // 表示用 = 元テキスト(行末CRを保持)。差分計算用 = 正規化済み行。
        let displayBase = LineDiff.splitLinesKeepingCR(base)
        let displayLeft = LineDiff.splitLinesKeepingCR(left)
        let displayRight = LineDiff.splitLinesKeepingCR(right)

        // 3側合計が上限を超える場合は安全に中断する(表示・計算とも省略)
        let totalLines = displayBase.count + displayLeft.count + displayRight.count
        guard totalLines <= maxTotalLines,
              displayBase.count <= maxTotalLines,
              displayLeft.count <= maxTotalLines,
              displayRight.count <= maxTotalLines else {
            return truncatedResult
        }

        let compareBase: [String]
        let compareLeft: [String]
        let compareRight: [String]
        if contentComparison.isStrict {
            compareBase = compareLines(base)
            compareLeft = compareLines(left)
            compareRight = compareLines(right)
        } else {
            compareBase = normalizedLines(base, contentComparison: contentComparison)
            compareLeft = normalizedLines(left, contentComparison: contentComparison)
            compareRight = normalizedLines(right, contentComparison: contentComparison)
        }

        // 改行コード正規化(\r→\n)で行数が変わると表示行と差分計算行の対応が取れないため
        // 中断する(LineDiff.rowsと同様の防御。孤立CRファイル + ignoreLineEndings等)。
        guard compareBase.count == displayBase.count,
              compareLeft.count == displayLeft.count,
              compareRight.count == displayRight.count else {
            return truncatedResult
        }

        // base→left / base→right の削除・挿入オフセット(CollectionDifference)
        let leftDiff = diffOffsets(base: compareBase, side: compareLeft)
        let rightDiff = diffOffsets(base: compareBase, side: compareRight)

        // 各側の編集スクリプト(baseとsideの整列単位)を作る。base行を共通祖先として
        // 2つの編集スクリプトを突き合わせるため、左右でハンク境界が異なる場合でも
        // 削除されたベース行と各側の置換行を正しく同じ行へ対応付ける。
        let leftUnits = buildUnits(
            base: compareBase, side: compareLeft,
            removed: leftDiff.removed, inserted: leftDiff.inserted
        )
        let rightUnits = buildUnits(
            base: compareBase, side: compareRight,
            removed: rightDiff.removed, inserted: rightDiff.inserted
        )

        let lines = mergeUnits(
            leftUnits: leftUnits,
            rightUnits: rightUnits,
            displayBase: displayBase,
            compareBase: compareBase,
            displayLeft: displayLeft,
            compareLeft: compareLeft,
            displayRight: displayRight,
            compareRight: compareRight
        )

        var unchanged = 0
        var leftChanged = 0
        var rightChanged = 0
        var bothSame = 0
        var conflict = 0
        for line in lines {
            switch line.status {
            case .unchanged: unchanged += 1
            case .leftChanged: leftChanged += 1
            case .rightChanged: rightChanged += 1
            case .bothChangedSame: bothSame += 1
            case .conflict: conflict += 1
            }
        }
        return ThreeWayDiff(
            lines: lines,
            truncated: false,
            unchangedCount: unchanged,
            leftChangedCount: leftChanged,
            rightChangedCount: rightChanged,
            bothChangedSameCount: bothSame,
            conflictCount: conflict
        )
    }

    // MARK: - 内部ヘルパ

    private static var truncatedResult: ThreeWayDiff {
        ThreeWayDiff(
            lines: [],
            truncated: true,
            unchangedCount: 0,
            leftChangedCount: 0,
            rightChangedCount: 0,
            bothChangedSameCount: 0,
            conflictCount: 0
        )
    }

    /// 差分計算用の行分割(空文字は空行1つ扱い。表示行と行数を一致させる)。
    /// CRLFはLFへ正規化する(LineDiff.splitLinesと同一挙動)。
    private static func compareLines(_ text: String) -> [String] {
        let normalized = text.contains("\r\n")
            ? text.replacingOccurrences(of: "\r\n", with: "\n")
            : text
        guard !normalized.isEmpty else { return [""] }
        return normalized
            .split(separator: "\n", omittingEmptySubsequences: false)
            .map(String.init)
    }

    /// 正規化後の行配列(空文字は空行1つ扱い)。
    private static func normalizedLines(
        _ text: String,
        contentComparison: ContentComparison
    ) -> [String] {
        let normalized = contentComparison.normalizedText(text)
        guard !normalized.isEmpty else { return [""] }
        return normalized
            .split(separator: "\n", omittingEmptySubsequences: false)
            .map(String.init)
    }

    /// CollectionDifferenceの削除(base側)/挿入(side側)オフセットを一度に取り出す。
    private static func diffOffsets(
        base: [String],
        side: [String]
    ) -> (removed: Set<Int>, inserted: Set<Int>) {
        var removed = Set<Int>()
        var inserted = Set<Int>()
        for change in side.difference(from: base) {
            switch change {
            case .remove(let offset, _, _): removed.insert(offset)
            case .insert(let offset, _, _): inserted.insert(offset)
            }
        }
        return (removed, inserted)
    }

    /// 行の状態を判定する。比較には正規化済みの行文字列を使う(表示には影響しない)。
    /// - baseがnil: 挿入行。左右の挿入が同一なら bothChangedSame、異なれば conflict。
    /// - baseあり: 各列をbaseと比較し、左右ともbaseのままなら unchanged、
    ///   片方だけ変更なら leftChanged/rightChanged、両方変更で左右が同一なら bothChangedSame、
    ///   異なる変更なら conflict。両側とも削除は「同一の変更」として bothChangedSame。
    private static func classify(
        base: String?,
        left: String?,
        right: String?
    ) -> ThreeWayDiffStatus {
        switch (base, left, right) {
        case (nil, let l?, let r?):
            return l == r ? .bothChangedSame : .conflict
        case (nil, _?, nil):
            return .leftChanged
        case (nil, nil, _?):
            return .rightChanged
        case (let b?, let l?, let r?):
            let leftSameAsBase = l == b
            let rightSameAsBase = r == b
            if leftSameAsBase && rightSameAsBase { return .unchanged }
            if leftSameAsBase { return .rightChanged }
            if rightSameAsBase { return .leftChanged }
            if l == r { return .bothChangedSame }
            return .conflict
        case (let b?, let l?, nil):
            // 右は削除。左がベースのままなら右だけの変更、そうでなければ別々の変更で競合
            return l == b ? .rightChanged : .conflict
        case (let b?, nil, let r?):
            // 左は削除。右がベースのままなら左だけの変更、そうでなければ競合
            return r == b ? .leftChanged : .conflict
        case (_?, nil, nil):
            // 両側とも同じ行を削除 = 同一の変更
            return .bothChangedSame
        case (nil, nil, nil):
            return .unchanged
        }
    }

    /// 片側の編集スクリプト(baseとsideの整列単位)。
    /// 対応するベース行とサイド行を1単位で表し、挿入・削除は片側だけを持つ。
    private enum SideUnit: Equatable {
        /// base行がside行に対応(matched: 同一行、または削除+置換)
        case aligned(base: Int, side: Int)
        /// base行がside側で削除された(置換行なし)
        case removed(base: Int)
        /// side行が新規挿入された(base対応なし)
        case inserted(side: Int)

        var baseIndex: Int? {
            switch self {
            case .aligned(let b, _), .removed(let b): return b
            case .inserted: return nil
            }
        }

        var sideIndex: Int? {
            switch self {
            case .aligned(_, let s), .inserted(let s): return s
            case .removed: return nil
            }
        }
    }

    /// baseとsideの編集スクリプトを整列単位列として構築する。
    /// 変更ブロック内の削除行と挿入行は位置的にペアリングして .aligned、
    /// 余った削除行は .removed、余った挿入行は .inserted とする。
    /// (2-wayのLineDiff.rowsと同じペアリング規則)
    private static func buildUnits(
        base: [String],
        side: [String],
        removed: Set<Int>,
        inserted: Set<Int>
    ) -> [SideUnit] {
        var units: [SideUnit] = []
        units.reserveCapacity(base.count + side.count)
        var i = 0
        var j = 0
        while i < base.count || j < side.count {
            if i < base.count, j < side.count, !removed.contains(i), !inserted.contains(j) {
                units.append(.aligned(base: i, side: j))
                i += 1
                j += 1
                continue
            }
            var removedRun: [Int] = []
            while i < base.count, removed.contains(i) {
                removedRun.append(i)
                i += 1
            }
            var insertedRun: [Int] = []
            while j < side.count, inserted.contains(j) {
                insertedRun.append(j)
                j += 1
            }
            if removedRun.isEmpty && insertedRun.isEmpty {
                // 安全弁(不変条件が崩れた場合の無限ループ防止)
                break
            }
            let pairedCount = min(removedRun.count, insertedRun.count)
            for k in 0..<pairedCount {
                units.append(.aligned(base: removedRun[k], side: insertedRun[k]))
            }
            for k in pairedCount..<removedRun.count {
                units.append(.removed(base: removedRun[k]))
            }
            for k in pairedCount..<insertedRun.count {
                units.append(.inserted(side: insertedRun[k]))
            }
        }
        return units
    }

    /// 左右の編集スクリプトをbase行を共通祖先として突き合わせ、3-wayの行モデルを生成する。
    /// 両側ともbase行に対応する単位は1行に結合し(base/left/rightを横並び表示)、
    /// 片側だけの純粋挿入は対応する位置の相手側挿入とペアリングする。
    /// 表示値は元テキスト(行末CR保持)、判定は正規化済み文字列を使う。
    private static func mergeUnits(
        leftUnits: [SideUnit],
        rightUnits: [SideUnit],
        displayBase: [String],
        compareBase: [String],
        displayLeft: [String],
        compareLeft: [String],
        displayRight: [String],
        compareRight: [String]
    ) -> [ThreeWayLine] {
        var lines: [ThreeWayLine] = []
        lines.reserveCapacity(max(compareBase.count, compareLeft.count, compareRight.count))
        var li = 0
        var ri = 0

        while li < leftUnits.count || ri < rightUnits.count {
            let leftUnit = li < leftUnits.count ? leftUnits[li] : nil
            let rightUnit = ri < rightUnits.count ? rightUnits[ri] : nil

            // 左右とも純粋挿入: 位置的にペアリングして1行に結合
            if case .inserted? = leftUnit, case .inserted? = rightUnit {
                let ls = leftUnit!.sideIndex!
                let rs = rightUnit!.sideIndex!
                let status = classify(base: nil, left: compareLeft[ls], right: compareRight[rs])
                appendRow(
                    &lines, id: lines.count, status: status,
                    base: nil, baseNumber: nil,
                    left: displayLeft[ls], leftNumber: ls + 1,
                    right: displayRight[rs], rightNumber: rs + 1
                )
                li += 1
                ri += 1
                continue
            }
            // 左だけ純粋挿入
            if case .inserted(let ls)? = leftUnit {
                appendRow(
                    &lines, id: lines.count, status: .leftChanged,
                    base: nil, baseNumber: nil,
                    left: displayLeft[ls], leftNumber: ls + 1,
                    right: nil, rightNumber: nil
                )
                li += 1
                continue
            }
            // 右だけ純粋挿入
            if case .inserted(let rs)? = rightUnit {
                appendRow(
                    &lines, id: lines.count, status: .rightChanged,
                    base: nil, baseNumber: nil,
                    left: nil, leftNumber: nil,
                    right: displayRight[rs], rightNumber: rs + 1
                )
                ri += 1
                continue
            }

            // 両側ともbase行対応。両方のスクリプトが同じ順序でbase行を持つため、
            // ここでは必ず同一のbase行を指す。
            guard let leftUnit, let rightUnit, leftUnit.baseIndex == rightUnit.baseIndex else {
                break // 安全弁
            }
            let baseIndex = leftUnit.baseIndex!
            let leftSide = leftUnit.sideIndex
            let rightSide = rightUnit.sideIndex
            let status = classify(
                base: compareBase[baseIndex],
                left: leftSide.map { compareLeft[$0] },
                right: rightSide.map { compareRight[$0] }
            )
            appendRow(
                &lines, id: lines.count, status: status,
                base: displayBase[baseIndex], baseNumber: baseIndex + 1,
                left: leftSide.map { displayLeft[$0] }, leftNumber: leftSide.map { $0 + 1 },
                right: rightSide.map { displayRight[$0] }, rightNumber: rightSide.map { $0 + 1 }
            )
            li += 1
            ri += 1
        }
        return lines
    }

    private static func appendRow(
        _ lines: inout [ThreeWayLine],
        id: Int,
        status: ThreeWayDiffStatus,
        base: String?, baseNumber: Int?,
        left: String?, leftNumber: Int?,
        right: String?, rightNumber: Int?
    ) {
        lines.append(ThreeWayLine(
            id: id,
            status: status,
            base: base,
            left: left,
            right: right,
            baseNumber: baseNumber,
            leftNumber: leftNumber,
            rightNumber: rightNumber
        ))
    }
}
