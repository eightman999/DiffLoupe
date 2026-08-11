// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: 行内差分(InlineDiff)の仕様検証。変更範囲の計算・Unicode・大文字小文字・上限保護

import XCTest
@testable import DiffLoupe

final class InlineDiffTests: XCTestCase {

    // MARK: - 基本

    func testIdenticalReturnsNil() {
        XCTAssertNil(InlineDiff.highlight(left: "abc", right: "abc", caseInsensitive: false))
    }

    func testEmptyStringsReturnNil() {
        XCTAssertNil(InlineDiff.highlight(left: "", right: "", caseInsensitive: false))
    }

    func testNilSideReturnsNil() {
        XCTAssertNil(InlineDiff.highlight(left: nil, right: "abc", caseInsensitive: false))
        XCTAssertNil(InlineDiff.highlight(left: "abc", right: nil, caseInsensitive: false))
    }

    func testSingleCharacterChange() throws {
        let h = try XCTUnwrap(InlineDiff.highlight(left: "abc", right: "axc", caseInsensitive: false))
        XCTAssertEqual(h.leftRanges, [1..<2])
        XCTAssertEqual(h.rightRanges, [1..<2])
    }

    func testInsertion() throws {
        let h = try XCTUnwrap(InlineDiff.highlight(left: "ab", right: "aXb", caseInsensitive: false))
        XCTAssertEqual(h.leftRanges, [])
        XCTAssertEqual(h.rightRanges, [1..<2])
    }

    func testDeletion() throws {
        let h = try XCTUnwrap(InlineDiff.highlight(left: "aXb", right: "ab", caseInsensitive: false))
        XCTAssertEqual(h.leftRanges, [1..<2])
        XCTAssertEqual(h.rightRanges, [])
    }

    func testEmptyLeftInsertion() throws {
        let h = try XCTUnwrap(InlineDiff.highlight(left: "", right: "abc", caseInsensitive: false))
        XCTAssertEqual(h.leftRanges, [])
        XCTAssertEqual(h.rightRanges, [0..<3])
    }

    func testEmptyRightDeletion() throws {
        let h = try XCTUnwrap(InlineDiff.highlight(left: "abc", right: "", caseInsensitive: false))
        XCTAssertEqual(h.leftRanges, [0..<3])
        XCTAssertEqual(h.rightRanges, [])
    }

    func testInsertionAtBothEnds() throws {
        let h = try XCTUnwrap(InlineDiff.highlight(left: "abc", right: "XXabcYY", caseInsensitive: false))
        XCTAssertEqual(h.leftRanges, [])
        XCTAssertEqual(h.rightRanges, [0..<2, 5..<7])
    }

    func testDeletionAtBothEnds() throws {
        let h = try XCTUnwrap(InlineDiff.highlight(left: "XXabcYY", right: "abc", caseInsensitive: false))
        XCTAssertEqual(h.leftRanges, [0..<2, 5..<7])
        XCTAssertEqual(h.rightRanges, [])
    }

    func testInsertionInMiddle() throws {
        let h = try XCTUnwrap(InlineDiff.highlight(left: "abcdef", right: "abxycdef", caseInsensitive: false))
        XCTAssertEqual(h.leftRanges, [])
        XCTAssertEqual(h.rightRanges, [2..<4])
    }

    func testDeletionInMiddle() throws {
        let h = try XCTUnwrap(InlineDiff.highlight(left: "abxycdef", right: "abcdef", caseInsensitive: false))
        XCTAssertEqual(h.leftRanges, [2..<4])
        XCTAssertEqual(h.rightRanges, [])
    }

    // MARK: - 大文字小文字

    func testCaseInsensitiveIgnoresCaseOnlyDifference() {
        XCTAssertNil(InlineDiff.highlight(left: "Hello World", right: "hello world", caseInsensitive: true))
    }

    func testCaseSensitiveDetectsCaseDifference() throws {
        let h = try XCTUnwrap(InlineDiff.highlight(left: "Hello World", right: "hello world", caseInsensitive: false))
        XCTAssertEqual(h.leftRanges, [0..<1, 6..<7])
        XCTAssertEqual(h.rightRanges, [0..<1, 6..<7])
    }

    func testCaseInsensitiveStillFindsRealDifference() throws {
        // 大文字小文字のみの差は無視されるため、全文字が一致 → nil
        XCTAssertNil(InlineDiff.highlight(left: "Hello", right: "hEllo", caseInsensitive: true))
        // 実質的な文字列違いは検出される(先頭5文字は一致、末尾" world"が挿入)
        let h = try XCTUnwrap(InlineDiff.highlight(left: "Hello", right: "hello world", caseInsensitive: true))
        XCTAssertEqual(h.leftRanges, [])
        XCTAssertEqual(h.rightRanges, [5..<11])
    }

    // MARK: - Unicode

    func testUnicodeCharacters() throws {
        // 日本語の変更(にち → ばん)
        let h = try XCTUnwrap(InlineDiff.highlight(left: "こんにちは", right: "こんばんは", caseInsensitive: false))
        XCTAssertEqual(h.leftRanges, [2..<4])
        XCTAssertEqual(h.rightRanges, [2..<4])
    }

    func testUnicodeWithEmoji() throws {
        // 絵文字1文字は1つのCharacterとして扱われ、Unicodeが壊れない
        let h = try XCTUnwrap(InlineDiff.highlight(left: "a🎉b", right: "a😀b", caseInsensitive: false))
        XCTAssertEqual(h.leftRanges, [1..<2])
        XCTAssertEqual(h.rightRanges, [1..<2])
    }

    // MARK: - 上限保護

    func testTooLongLineReturnsNil() {
        let overHalf = String(repeating: "x", count: InlineDiff.maxCharacters / 2 + 1)
        XCTAssertNil(InlineDiff.highlight(left: overHalf, right: overHalf, caseInsensitive: false))
        let overRight = String(repeating: "y", count: InlineDiff.maxCharacters + 1)
        XCTAssertNil(InlineDiff.highlight(left: "a", right: overRight, caseInsensitive: false))
    }

    func testAtLimitIsCalculated() {
        let half = String(repeating: "x", count: InlineDiff.maxCharacters / 2 - 1)
        XCTAssertNotNil(InlineDiff.highlight(left: half, right: half + "y", caseInsensitive: false))
    }

    // MARK: - highlightsForRows

    func testHighlightsForRowsOnlyModified() {
        let rows = [
            DiffRow(id: 0, kind: .context, leftNumber: 1, leftText: "same", rightNumber: 1, rightText: "same"),
            DiffRow(id: 1, kind: .modified, leftNumber: 2, leftText: "abc", rightNumber: 2, rightText: "axc"),
            DiffRow(id: 2, kind: .added, leftNumber: nil, leftText: nil, rightNumber: 3, rightText: "new"),
            DiffRow(id: 3, kind: .removed, leftNumber: 4, leftText: "old", rightNumber: nil, rightText: nil),
            DiffRow(id: 4, kind: .modified, leftNumber: 5, leftText: "Hello", rightNumber: 5, rightText: "Hello World"),
        ]
        let highlights = InlineDiff.highlightsForRows(rows, caseInsensitive: false)
        XCTAssertEqual(highlights.count, 2)
        XCTAssertNotNil(highlights[1])
        XCTAssertNil(highlights[0])
        XCTAssertNil(highlights[2])
        XCTAssertNil(highlights[3])
        XCTAssertNotNil(highlights[4])
    }

    func testHighlightsForRowsRespectsCaseInsensitive() {
        let rows = [
            DiffRow(id: 0, kind: .modified, leftNumber: 1, leftText: "Hello World", rightNumber: 1, rightText: "hello world"),
            DiffRow(id: 1, kind: .modified, leftNumber: 2, leftText: "abc", rightNumber: 2, rightText: "axc"),
        ]
        // 大文字小文字無視: 大文字小文字のみの差の行はハイライトなし
        let insensitive = InlineDiff.highlightsForRows(rows, caseInsensitive: true)
        XCTAssertNil(insensitive[0])
        XCTAssertNotNil(insensitive[1])
        // 大文字小文字区別: 両方ハイライト
        let sensitive = InlineDiff.highlightsForRows(rows, caseInsensitive: false)
        XCTAssertNotNil(sensitive[0])
        XCTAssertNotNil(sensitive[1])
    }
}
