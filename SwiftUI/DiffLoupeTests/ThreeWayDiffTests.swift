// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: Xcodeアプリターゲットから見たThreeWayDiffの振る舞いを検証する。
// 実装はDiffLoupeCore(CoreTests/ThreeWayDiffTests.swift)と同一の純粋ロジックのため、
// ここでは主要カテゴリ(unchanged/片側変更/同一変更/conflict/挿入削除/正規化/40k上限)を確認する。

import XCTest
@testable import DiffLoupe

final class ThreeWayDiffTests: XCTestCase {

    private func makeText(_ lines: [String]) -> String {
        lines.joined(separator: "\n")
    }

    func testUnchangedAllSidesIdentical() {
        let text = makeText(["a", "b", "c"])
        let diff = ThreeWayDiff.make(base: text, left: text, right: text)
        XCTAssertFalse(diff.truncated)
        XCTAssertEqual(diff.lines.count, 3)
        XCTAssertTrue(diff.lines.allSatisfy { $0.status == .unchanged })
        XCTAssertEqual(diff.totalChangedCount, 0)
    }

    func testLeftOnlyChangeIsLeftChanged() {
        let base = makeText(["a", "b", "c"])
        let left = makeText(["a", "X", "c"])
        let right = makeText(["a", "b", "c"])
        let diff = ThreeWayDiff.make(base: base, left: left, right: right)
        XCTAssertEqual(diff.lines[1].status, .leftChanged)
        XCTAssertEqual(diff.lines[1].left, "X")
        XCTAssertEqual(diff.leftChangedCount, 1)
    }

    func testRightOnlyChangeIsRightChanged() {
        let base = makeText(["a", "b", "c"])
        let left = makeText(["a", "b", "c"])
        let right = makeText(["a", "Y", "c"])
        let diff = ThreeWayDiff.make(base: base, left: left, right: right)
        XCTAssertEqual(diff.lines[1].status, .rightChanged)
        XCTAssertEqual(diff.rightChangedCount, 1)
    }

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
    }

    func testInsertAndDelete() {
        let base = makeText(["a", "b", "c"])
        let left = makeText(["a", "x", "b"])   // c削除 + x挿入
        let right = makeText(["a", "b", "c"])
        let diff = ThreeWayDiff.make(base: base, left: left, right: right)
        XCTAssertEqual(diff.lines[1].status, .leftChanged)
        XCTAssertEqual(diff.lines[1].left, "x")
        XCTAssertEqual(diff.lines[2].status, .unchanged)
        XCTAssertEqual(diff.lines[2].base, "b")
        XCTAssertEqual(diff.lines[3].status, .leftChanged)
        XCTAssertEqual(diff.lines[3].base, "c")
        XCTAssertNil(diff.lines[3].left)
        XCTAssertEqual(diff.lines[3].right, "c")
    }

    func testOverlappingHunksDeleteBothSides() {
        let base = makeText(["A", "B", "C", "D"])
        let left = makeText(["A", "B", "D"])
        let right = makeText(["A", "B", "C"])
        let diff = ThreeWayDiff.make(base: base, left: left, right: right)
        XCTAssertEqual(diff.lines[2].status, .leftChanged)
        XCTAssertEqual(diff.lines[3].status, .rightChanged)
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
        XCTAssertEqual(diff.lines[1].status, .leftChanged)
        XCTAssertEqual(diff.lines[1].base, "B")
        XCTAssertEqual(diff.lines[1].left, "B'")
        XCTAssertEqual(diff.lines[2].status, .conflict)
        XCTAssertEqual(diff.lines[2].base, "C")
        XCTAssertNil(diff.lines[2].left)
        XCTAssertEqual(diff.lines[2].right, "C'")
        XCTAssertEqual(diff.lines[3].status, .unchanged)
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
        XCTAssertEqual(diff.lines[1].right, "B'")
        XCTAssertEqual(diff.lines[2].status, .conflict)
        XCTAssertEqual(diff.lines[2].base, "C")
        XCTAssertEqual(diff.lines[2].left, "C'")
        XCTAssertNil(diff.lines[2].right)
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

    func testIgnoreCaseUsesNormalizationForJudgingButKeepsOriginalDisplay() {
        let base = makeText(["foo", "bar"])
        let left = makeText(["FOO", "bar"])
        let right = makeText(["FOO", "bar"])
        let options = ContentComparison(ignoreCase: true)
        let diff = ThreeWayDiff.make(base: base, left: left, right: right, contentComparison: options)
        XCTAssertTrue(diff.lines.allSatisfy { $0.status == .unchanged })
        XCTAssertEqual(diff.totalChangedCount, 0)
        XCTAssertEqual(diff.lines[0].left, "FOO")
    }

    func testIgnoreCaseMarksSameNormalizedChangeAsBothChangedSame() {
        let base = makeText(["foo", "bar"])
        let left = makeText(["FOO x", "bar"])
        let right = makeText(["FOO x", "bar"])
        let options = ContentComparison(ignoreCase: true)
        let diff = ThreeWayDiff.make(base: base, left: left, right: right, contentComparison: options)
        XCTAssertEqual(diff.lines[0].status, .bothChangedSame)
    }

    func testTruncatedOverTotalLineCap() {
        let lineCount = 14_000
        let base = makeText(Array(repeating: "base", count: lineCount))
        let left = makeText(Array(repeating: "left", count: lineCount))
        let right = makeText(Array(repeating: "right", count: lineCount))
        let diff = ThreeWayDiff.make(base: base, left: left, right: right)
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
        XCTAssertFalse(diff.truncated)
        XCTAssertEqual(diff.lines[lineCount - 1].status, .conflict)
    }
}
