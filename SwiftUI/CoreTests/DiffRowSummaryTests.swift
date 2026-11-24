// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: DiffDisplay.summary(of:)(テキスト貼り付け比較の要約)のフォーカステスト。
// 文字列2つからLineDiff.rowsで行を作り、追加・削除・変更・一致の行数集計を検証する。

import XCTest
@testable import DiffLoupeCore

final class DiffRowSummaryTests: XCTestCase {

    private func summary(_ left: String, _ right: String, _ cc: ContentComparison = ContentComparison()) throws -> DiffDisplay.RowSummary {
        let rows = try XCTUnwrap(LineDiff.rows(leftText: left, rightText: right, contentComparison: cc))
        return DiffDisplay.summary(of: rows)
    }

    func testEmptyRowsIsIdentical() {
        let s = DiffDisplay.summary(of: [])
        XCTAssertEqual(s, DiffDisplay.RowSummary())
        XCTAssertTrue(s.isIdentical)
    }

    func testIdenticalText() throws {
        let s = try summary("a\nb\nc", "a\nb\nc")
        XCTAssertTrue(s.isIdentical)
        XCTAssertEqual(s.contextCount, 3)
        XCTAssertEqual(s.differenceCount, 0)
    }

    func testAddedRemovedModified() throws {
        let added = try summary("a\nb", "a\nb\nc")
        XCTAssertEqual(added.addedCount, 1)
        XCTAssertEqual(added.removedCount, 0)
        XCTAssertEqual(added.contextCount, 2)

        let removed = try summary("a\nb\nc", "a\nc")
        XCTAssertEqual(removed.removedCount, 1)
        XCTAssertEqual(removed.addedCount, 0)

        let modified = try summary("a\nb\nc", "a\nB\nc")
        XCTAssertEqual(modified.modifiedCount, 1)
        XCTAssertEqual(modified.contextCount, 2)
        XCTAssertFalse(modified.isIdentical)
        XCTAssertEqual(modified.differenceCount, 1)
    }

    func testIgnoreCaseMakesIdentical() throws {
        var cc = ContentComparison()
        cc.ignoreCase = true
        XCTAssertTrue(try summary("Hello", "hello", cc).isIdentical)
        XCTAssertFalse(try summary("Hello", "hello").isIdentical)
    }

    func testTooManyLinesReturnsNil() {
        let big = Array(repeating: "x", count: LineDiff.maxTotalLines / 2 + 1).joined(separator: "\n")
        XCTAssertNil(LineDiff.rows(leftText: big, rightText: big))
    }
}
