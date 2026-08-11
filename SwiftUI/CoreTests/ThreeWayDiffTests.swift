// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: ThreeWayDiff.make(base:left:right:contentComparison:)の純粋ロジックを検証する。
// unchanged / 片側変更 / 両者同一変更 / conflict / 挿入・削除 / 正規化 / 40k上限を確認する。

import XCTest
@testable import DiffLoupeCore

final class ThreeWayDiffTests: XCTestCase {

    private func makeText(_ lines: [String]) -> String {
        lines.joined(separator: "\n")
    }

    // MARK: - unchanged

    func testUnchangedAllSidesIdentical() {
        let text = makeText(["a", "b", "c"])
        let diff = ThreeWayDiff.make(base: text, left: text, right: text)

        XCTAssertFalse(diff.truncated)
        XCTAssertEqual(diff.lines.count, 3)
        XCTAssertTrue(diff.lines.allSatisfy { $0.status == .unchanged })
        XCTAssertEqual(diff.unchangedCount, 3)
        XCTAssertEqual(diff.conflictCount, 0)
        XCTAssertEqual(diff.totalChangedCount, 0)

        // 原文表示と行番号
        XCTAssertEqual(diff.lines[0].base, "a")
        XCTAssertEqual(diff.lines[0].left, "a")
        XCTAssertEqual(diff.lines[0].right, "a")
        XCTAssertEqual(diff.lines[1].baseNumber, 2)
        XCTAssertEqual(diff.lines[1].leftNumber, 2)
        XCTAssertEqual(diff.lines[1].rightNumber, 2)
    }

    func testEmptyFilesAreUnchanged() {
        let diff = ThreeWayDiff.make(base: "", left: "", right: "")
        XCTAssertFalse(diff.truncated)
        XCTAssertEqual(diff.lines.count, 1)
        XCTAssertEqual(diff.lines[0].status, .unchanged)
    }

    // MARK: - 片側変更

    func testLeftOnlyChangeIsLeftChanged() {
        let base = makeText(["a", "b", "c"])
        let left = makeText(["a", "X", "c"])
        let right = makeText(["a", "b", "c"])
        let diff = ThreeWayDiff.make(base: base, left: left, right: right)

        XCTAssertEqual(diff.lines.count, 3)
        XCTAssertEqual(diff.lines[1].status, .leftChanged)
        XCTAssertEqual(diff.lines[1].base, "b")
        XCTAssertEqual(diff.lines[1].left, "X")
        XCTAssertEqual(diff.lines[1].right, "b")
        XCTAssertEqual(diff.leftChangedCount, 1)
        XCTAssertEqual(diff.rightChangedCount, 0)
    }

    func testRightOnlyChangeIsRightChanged() {
        let base = makeText(["a", "b", "c"])
        let left = makeText(["a", "b", "c"])
        let right = makeText(["a", "Y", "c"])
        let diff = ThreeWayDiff.make(base: base, left: left, right: right)

        XCTAssertEqual(diff.lines[1].status, .rightChanged)
        XCTAssertEqual(diff.lines[1].right, "Y")
        XCTAssertEqual(diff.rightChangedCount, 1)
    }

    // MARK: - 両者同一変更 / conflict

    func testBothChangedSame() {
        let base = makeText(["a", "b", "c"])
        let left = makeText(["a", "X", "c"])
        let right = makeText(["a", "X", "c"])
        let diff = ThreeWayDiff.make(base: base, left: left, right: right)

        XCTAssertEqual(diff.lines[1].status, .bothChangedSame)
        XCTAssertEqual(diff.bothChangedSameCount, 1)
        XCTAssertEqual(diff.conflictCount, 0)
    }

    func testConflict() {
        let base = makeText(["a", "b", "c"])
        let left = makeText(["a", "X", "c"])
        let right = makeText(["a", "Y", "c"])
        let diff = ThreeWayDiff.make(base: base, left: left, right: right)

        XCTAssertEqual(diff.lines[1].status, .conflict)
        XCTAssertEqual(diff.conflictCount, 1)
        XCTAssertEqual(diff.bothChangedSameCount, 0)
    }

    // MARK: - 挿入・削除

    func testInsertOnLeftOnly() {
        let base = makeText(["a", "b"])
        let left = makeText(["a", "x", "b"])
        let right = makeText(["a", "b"])
        let diff = ThreeWayDiff.make(base: base, left: left, right: right)

        XCTAssertEqual(diff.lines.count, 3)
        XCTAssertEqual(diff.lines[1].status, .leftChanged)
        XCTAssertNil(diff.lines[1].base)
        XCTAssertEqual(diff.lines[1].left, "x")
        XCTAssertNil(diff.lines[1].right)
    }

    func testDeleteOnLeftOnly() {
        let base = makeText(["a", "b", "c"])
        let left = makeText(["a", "c"])
        let right = makeText(["a", "b", "c"])
        let diff = ThreeWayDiff.make(base: base, left: left, right: right)

        XCTAssertEqual(diff.lines[1].status, .leftChanged)
        XCTAssertEqual(diff.lines[1].base, "b")
        XCTAssertNil(diff.lines[1].left)
        XCTAssertEqual(diff.lines[1].right, "b")
    }

    func testBothInsertSameLine() {
        let base = makeText(["a", "b"])
        let left = makeText(["a", "x", "b"])
        let right = makeText(["a", "x", "b"])
        let diff = ThreeWayDiff.make(base: base, left: left, right: right)

        XCTAssertEqual(diff.lines[1].status, .bothChangedSame)
        XCTAssertEqual(diff.bothChangedSameCount, 1)
    }

    func testInsertConflict() {
        let base = makeText(["a", "b"])
        let left = makeText(["a", "x", "b"])
        let right = makeText(["a", "y", "b"])
        let diff = ThreeWayDiff.make(base: base, left: left, right: right)

        XCTAssertEqual(diff.lines[1].status, .conflict)
        XCTAssertEqual(diff.conflictCount, 1)
    }

    func testDeleteOnBothSidesIsBothChangedSame() {
        let base = makeText(["a", "b", "c"])
        let left = makeText(["a", "c"])
        let right = makeText(["a", "c"])
        let diff = ThreeWayDiff.make(base: base, left: left, right: right)

        XCTAssertEqual(diff.lines[1].status, .bothChangedSame)
        XCTAssertEqual(diff.lines[1].base, "b")
        XCTAssertNil(diff.lines[1].left)
        XCTAssertNil(diff.lines[1].right)
        XCTAssertEqual(diff.bothChangedSameCount, 1)
    }

    func testOverlappingHunksDeleteBothSides() {
        // base: A B C D / left: A B D (C削除) / right: A B C (D削除)
        let base = makeText(["A", "B", "C", "D"])
        let left = makeText(["A", "B", "D"])
        let right = makeText(["A", "B", "C"])
        let diff = ThreeWayDiff.make(base: base, left: left, right: right)

        XCTAssertEqual(diff.lines.count, 4)
        XCTAssertEqual(diff.lines[2].status, .leftChanged)
        XCTAssertEqual(diff.lines[3].status, .rightChanged)
    }

    func testDeleteVersusChangeIsConflict() {
        // leftはBを削除、rightはB→Xへ変更 → 競合
        let base = makeText(["A", "B", "C"])
        let left = makeText(["A", "C"])
        let right = makeText(["A", "X", "C"])
        let diff = ThreeWayDiff.make(base: base, left: left, right: right)

        XCTAssertEqual(diff.lines[1].status, .conflict)
        XCTAssertEqual(diff.conflictCount, 1)
    }

    // MARK: - ハンク整列の回帰(監査指摘の実反例)

    func testReplacementMergesToConflictCE1() {
        // 実反例: base=[A,B,C,D], left=[A,B',D] (B→B'置換+C削除),
        // right=[A,B,C',D] (C→C'置換)。C行は base C / left nil / right C' の1行に
        // 結合して conflict になるべき(孤立したrightChanged行を作らない)。
        let base = makeText(["A", "B", "C", "D"])
        let left = makeText(["A", "B'", "D"])
        let right = makeText(["A", "B", "C'", "D"])
        let diff = ThreeWayDiff.make(base: base, left: left, right: right)

        XCTAssertFalse(diff.truncated)
        XCTAssertEqual(diff.lines.count, 4)

        XCTAssertEqual(diff.lines[0].status, .unchanged)
        XCTAssertEqual(diff.lines[1].status, .leftChanged)
        XCTAssertEqual(diff.lines[1].base, "B")
        XCTAssertEqual(diff.lines[1].left, "B'")
        XCTAssertEqual(diff.lines[1].right, "B")

        XCTAssertEqual(diff.lines[2].status, .conflict)
        XCTAssertEqual(diff.lines[2].base, "C")
        XCTAssertNil(diff.lines[2].left)
        XCTAssertEqual(diff.lines[2].right, "C'")

        XCTAssertEqual(diff.lines[3].status, .unchanged)
        XCTAssertEqual(diff.lines[3].base, "D")
        XCTAssertEqual(diff.conflictCount, 1)
        XCTAssertEqual(diff.bothChangedSameCount, 0)
        XCTAssertEqual(diff.rightChangedCount, 0)
    }

    func testReplacementMergesToConflictSymmetricCE2() {
        // 対称ケース: base=[A,B,C,D], left=[A,B,C',D], right=[A,B',D]。
        // C行は base C / left C' / right nil の1行に結合して conflict。
        let base = makeText(["A", "B", "C", "D"])
        let left = makeText(["A", "B", "C'", "D"])
        let right = makeText(["A", "B'", "D"])
        let diff = ThreeWayDiff.make(base: base, left: left, right: right)

        XCTAssertFalse(diff.truncated)
        XCTAssertEqual(diff.lines.count, 4)

        XCTAssertEqual(diff.lines[1].status, .rightChanged)
        XCTAssertEqual(diff.lines[1].base, "B")
        XCTAssertEqual(diff.lines[1].left, "B")
        XCTAssertEqual(diff.lines[1].right, "B'")

        XCTAssertEqual(diff.lines[2].status, .conflict)
        XCTAssertEqual(diff.lines[2].base, "C")
        XCTAssertEqual(diff.lines[2].left, "C'")
        XCTAssertNil(diff.lines[2].right)

        XCTAssertEqual(diff.lines[3].status, .unchanged)
        XCTAssertEqual(diff.conflictCount, 1)
        XCTAssertEqual(diff.bothChangedSameCount, 0)
        XCTAssertEqual(diff.leftChangedCount, 0)
    }

    func testLeftDeleteRightInsertNotIsolated() {
        // 左がBを削除し、右がCをXに置換する別ハンク。右の挿入Xは孤立行に
        // ならず、base Cの行に結合されて表示される(未収集挿入の孤立防止)。
        let base = makeText(["A", "B", "C", "D"])
        let left = makeText(["A", "C", "D"])
        let right = makeText(["A", "B", "X", "D"])
        let diff = ThreeWayDiff.make(base: base, left: left, right: right)

        XCTAssertFalse(diff.truncated)
        XCTAssertEqual(diff.lines.count, 4)

        XCTAssertEqual(diff.lines[1].status, .leftChanged)
        XCTAssertEqual(diff.lines[1].base, "B")
        XCTAssertNil(diff.lines[1].left)
        XCTAssertEqual(diff.lines[1].right, "B")

        XCTAssertEqual(diff.lines[2].status, .rightChanged)
        XCTAssertEqual(diff.lines[2].base, "C")
        XCTAssertEqual(diff.lines[2].left, "C")
        XCTAssertEqual(diff.lines[2].right, "X")

        XCTAssertEqual(diff.lines[3].status, .unchanged)
        XCTAssertEqual(diff.rightChangedCount, 1)
    }

    func testInsertDeleteChangeCombinationPreserved() {
        // 挿入・削除・置換が混在しても従来の分類(insert/delete/change)を維持する。
        let base = makeText(["a", "b", "c", "d"])
        let left = makeText(["a", "X", "c"])        // b→X置換 + d削除
        let right = makeText(["a", "b", "Y", "d"])  // c→Y置換
        let diff = ThreeWayDiff.make(base: base, left: left, right: right)

        XCTAssertFalse(diff.truncated)
        XCTAssertEqual(diff.lines.count, 4)
        XCTAssertEqual(diff.lines[0].status, .unchanged)
        XCTAssertEqual(diff.lines[1].status, .leftChanged)
        XCTAssertEqual(diff.lines[1].base, "b")
        XCTAssertEqual(diff.lines[1].left, "X")
        XCTAssertEqual(diff.lines[1].right, "b")
        XCTAssertEqual(diff.lines[2].status, .rightChanged)
        XCTAssertEqual(diff.lines[2].base, "c")
        XCTAssertEqual(diff.lines[2].left, "c")
        XCTAssertEqual(diff.lines[2].right, "Y")
        XCTAssertEqual(diff.lines[3].status, .leftChanged)
        XCTAssertEqual(diff.lines[3].base, "d")
        XCTAssertNil(diff.lines[3].left)
        XCTAssertEqual(diff.lines[3].right, "d")
        XCTAssertEqual(diff.leftChangedCount, 2)
        XCTAssertEqual(diff.rightChangedCount, 1)
        XCTAssertEqual(diff.conflictCount, 0)
    }

    // MARK: - 正規化(判定のみに使用・表示は原文)

    func testIgnoreCaseUsesNormalizationForJudgingButKeepsOriginalDisplay() {
        let base = makeText(["foo", "bar"])
        let left = makeText(["FOO", "bar"])
        let right = makeText(["FOO", "bar"])
        let options = ContentComparison(ignoreCase: true)
        let diff = ThreeWayDiff.make(base: base, left: left, right: right, contentComparison: options)

        // 大文字差は正規化で同一 → 変更として扱われない
        XCTAssertTrue(diff.lines.allSatisfy { $0.status == .unchanged })
        XCTAssertEqual(diff.totalChangedCount, 0)
        // 表示は原文のまま
        XCTAssertEqual(diff.lines[0].base, "foo")
        XCTAssertEqual(diff.lines[0].left, "FOO")
        XCTAssertEqual(diff.lines[0].right, "FOO")
    }

    func testIgnoreCaseMarksSameNormalizedChangeAsBothChangedSame() {
        let base = makeText(["foo", "bar"])
        let left = makeText(["FOO x", "bar"])
        let right = makeText(["FOO x", "bar"])
        let options = ContentComparison(ignoreCase: true)
        let diff = ThreeWayDiff.make(base: base, left: left, right: right, contentComparison: options)

        XCTAssertEqual(diff.lines[0].status, .bothChangedSame)
        XCTAssertEqual(diff.lines[0].left, "FOO x")
    }

    func testIgnoreTrailingWhitespaceTreatsWhitespaceOnlyDiffAsUnchanged() {
        let base = makeText(["x ", "y"])
        let left = makeText(["x", "y"])
        let right = makeText(["x", "y"])
        let options = ContentComparison(ignoreTrailingWhitespace: true)
        let diff = ThreeWayDiff.make(base: base, left: left, right: right, contentComparison: options)

        XCTAssertEqual(diff.totalChangedCount, 0)
        XCTAssertEqual(diff.lines[0].left, "x")
    }

    func testStrictComparisonTreatsCaseDifferenceAsChange() {
        let base = makeText(["foo"])
        let left = makeText(["FOO"])
        let right = makeText(["foo"])
        let diff = ThreeWayDiff.make(base: base, left: left, right: right)

        XCTAssertEqual(diff.lines[0].status, .leftChanged)
    }

    // MARK: - 40k上限

    func testTruncatedOverTotalLineCap() {
        let lineCount = 14_000
        let base = makeText(Array(repeating: "base", count: lineCount))
        let left = makeText(Array(repeating: "left", count: lineCount))
        let right = makeText(Array(repeating: "right", count: lineCount))

        let diff = ThreeWayDiff.make(base: base, left: left, right: right)
        // 14k+14k+14k = 42k > 40k → 中断
        XCTAssertTrue(diff.truncated)
        XCTAssertEqual(diff.lines.count, 0)
    }

    func testNotTruncatedAtCapBoundary() {
        let lineCount = 13_000
        var base = Array(repeating: "common", count: lineCount)
        var left = base
        var right = base
        base[lineCount - 1] = "base end"
        left[lineCount - 1] = "left end"
        right[lineCount - 1] = "right end"

        let diff = ThreeWayDiff.make(
            base: makeText(base),
            left: makeText(left),
            right: makeText(right)
        )
        // 39k ≤ 40k → 計算する。最終行は左右が異なる変更で競合
        XCTAssertFalse(diff.truncated)
        XCTAssertEqual(diff.lines.count, lineCount)
        XCTAssertEqual(diff.lines[lineCount - 1].status, .conflict)
        XCTAssertEqual(diff.conflictCount, 1)
    }

    // MARK: - Sendable / Equatable / Hashable

    func testSendableEquatableHashable() {
        let text = makeText(["a", "b"])
        let diff1 = ThreeWayDiff.make(base: text, left: text, right: text)
        let diff2 = ThreeWayDiff.make(base: text, left: text, right: text)

        XCTAssertEqual(diff1, diff2)
        XCTAssertEqual(diff1.lines[0], diff2.lines[0])
        // Hashable(Set/辞書で使用可能)
        XCTAssertEqual(Set(diff1.lines).count, diff1.lines.count)
        XCTAssertEqual(Set([diff1, diff2]).count, 1)
    }
}
