// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: 行単位差分(LineDiff)とテキスト読み込み(TextFileLoader)の仕様検証

import XCTest

final class LineDiffTests: XCTestCase {

    func testIdenticalTexts() throws {
        let rows = try XCTUnwrap(LineDiff.rows(leftText: "a\nb\nc\n", rightText: "a\nb\nc\n"))
        XCTAssertTrue(rows.allSatisfy { $0.kind == .context })
        XCTAssertEqual(rows.count, 4)   // 末尾改行による空行を含む
    }

    func testAddition() throws {
        let rows = try XCTUnwrap(LineDiff.rows(leftText: "a\nb\n", rightText: "a\nx\nb\n"))
        let added = rows.filter { $0.kind == .added }
        XCTAssertEqual(added.count, 1)
        XCTAssertEqual(added.first?.rightText, "x")
        XCTAssertNil(added.first?.leftNumber)
        XCTAssertEqual(added.first?.rightNumber, 2)
    }

    func testRemoval() throws {
        let rows = try XCTUnwrap(LineDiff.rows(leftText: "a\nx\nb\n", rightText: "a\nb\n"))
        let removed = rows.filter { $0.kind == .removed }
        XCTAssertEqual(removed.count, 1)
        XCTAssertEqual(removed.first?.leftText, "x")
        XCTAssertNil(removed.first?.rightNumber)
    }

    func testModificationPairing() throws {
        let rows = try XCTUnwrap(LineDiff.rows(leftText: "a\nb\nc\n", rightText: "a\nX\nc\n"))
        let modified = rows.filter { $0.kind == .modified }
        XCTAssertEqual(modified.count, 1)
        XCTAssertEqual(modified.first?.leftText, "b")
        XCTAssertEqual(modified.first?.rightText, "X")
    }

    func testBlockPairingWithRemainder() throws {
        // 2行削除+3行挿入 → 2行は「変更」ペア、1行は「追加」
        let rows = try XCTUnwrap(LineDiff.rows(
            leftText: "1\n2\n3\n4\n",
            rightText: "1\nA\nB\nC\n4\n"))
        XCTAssertEqual(rows.filter { $0.kind == .modified }.count, 2)
        XCTAssertEqual(rows.filter { $0.kind == .added }.count, 1)
        XCTAssertEqual(rows.filter { $0.kind == .removed }.count, 0)
    }

    func testCRLFTreatedAsEqualLines() throws {
        let rows = try XCTUnwrap(LineDiff.rows(leftText: "a\r\nb\r\n", rightText: "a\nb\n"))
        XCTAssertTrue(rows.allSatisfy { $0.kind == .context })
    }

    func testLineNumbersAreSequential() throws {
        let rows = try XCTUnwrap(LineDiff.rows(leftText: "a\nb\nc\n", rightText: "a\nX\nY\nc\n"))
        let leftNumbers = rows.compactMap(\.leftNumber)
        let rightNumbers = rows.compactMap(\.rightNumber)
        XCTAssertEqual(leftNumbers, Array(1...4))
        XCTAssertEqual(rightNumbers, Array(1...5))
    }

    func testTooManyLinesReturnsNil() {
        let lines = String(repeating: "line\n", count: LineDiff.maxTotalLines / 2 + 1)
        XCTAssertNil(LineDiff.rows(leftText: lines, rightText: lines))
    }
}

final class TextFileLoaderTests: XCTestCase {

    private func writeTemp(_ data: Data) throws -> URL {
        let url = FileManager.default.temporaryDirectory
            .appendingPathComponent("TextFileLoaderTests-\(UUID().uuidString).bin")
        try data.write(to: url)
        addTeardownBlock { try? FileManager.default.removeItem(at: url) }
        return url
    }

    func testUTF8Text() throws {
        let url = try writeTemp(Data("こんにちは\nworld\n".utf8))
        guard case .text(let text) = try TextFileLoader.load(url: url) else {
            return XCTFail("テキストとして読めるべき")
        }
        XCTAssertTrue(text.contains("こんにちは"))
    }

    func testNullByteMeansBinary() throws {
        let url = try writeTemp(Data([0x41, 0x00, 0x42]))
        guard case .binary = try TextFileLoader.load(url: url) else {
            return XCTFail("バイナリ判定されるべき")
        }
    }

    func testInvalidUTF8MeansBinary() throws {
        let url = try writeTemp(Data([0xFF, 0xFE, 0x41, 0x42]))
        guard case .binary = try TextFileLoader.load(url: url) else {
            return XCTFail("バイナリ判定されるべき")
        }
    }

    func testEmptyFileIsText() throws {
        let url = try writeTemp(Data())
        guard case .text(let text) = try TextFileLoader.load(url: url) else {
            return XCTFail("空ファイルはテキスト扱い")
        }
        XCTAssertEqual(text, "")
    }
}
