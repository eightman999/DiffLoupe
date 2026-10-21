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
        guard case .text(let text, let encodingName) = try TextFileLoader.load(url: url) else {
            return XCTFail("テキストとして読めるべき")
        }
        XCTAssertTrue(text.contains("こんにちは"))
        XCTAssertEqual(encodingName, "UTF-8")
    }

    func testNullByteMeansBinary() throws {
        let url = try writeTemp(Data([0x41, 0x00, 0x42]))
        guard case .binary = try TextFileLoader.load(url: url) else {
            return XCTFail("バイナリ判定されるべき")
        }
    }

    func testInvalidUTF8MeansBinary() throws {
        // PNGヘッダ相当のバイト列(真のバイナリ)。8bit値を含みISO-2022-JPのESC検出にも
        // 引っかからず、Shift_JIS/EUC-JPとしても妥当な文字列にならない
        let url = try writeTemp(Data([0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D]))
        guard case .binary = try TextFileLoader.load(url: url) else {
            return XCTFail("バイナリ判定されるべき")
        }
    }

    func testEmptyFileIsText() throws {
        let url = try writeTemp(Data())
        guard case .text(let text, let encodingName) = try TextFileLoader.load(url: url) else {
            return XCTFail("空ファイルはテキスト扱い")
        }
        XCTAssertEqual(text, "")
        XCTAssertEqual(encodingName, "UTF-8")
    }

    func testShiftJISText() throws {
        let original = "こんにちは、世界\nシフトJISのテスト"
        let data = try XCTUnwrap(original.data(using: .shiftJIS))
        let url = try writeTemp(data)
        guard case .text(let text, let encodingName) = try TextFileLoader.load(url: url) else {
            return XCTFail("Shift_JISとして読めるべき")
        }
        XCTAssertEqual(text, original)
        XCTAssertEqual(encodingName, "Shift_JIS")
    }

    func testEUCJPText() throws {
        let original = "これはEUC-JPのテストです\n日本語文字列"
        let data = try XCTUnwrap(original.data(using: .japaneseEUC))
        let url = try writeTemp(data)
        guard case .text(let text, let encodingName) = try TextFileLoader.load(url: url) else {
            return XCTFail("EUC-JPとして読めるべき")
        }
        XCTAssertEqual(text, original)
        XCTAssertEqual(encodingName, "EUC-JP")
    }

    func testISO2022JPText() throws {
        let original = "日本語のISO-2022-JPテスト"
        let data = try XCTUnwrap(original.data(using: .iso2022JP))
        let url = try writeTemp(data)
        guard case .text(let text, let encodingName) = try TextFileLoader.load(url: url) else {
            return XCTFail("ISO-2022-JPとして読めるべき")
        }
        XCTAssertEqual(text, original)
        XCTAssertEqual(encodingName, "ISO-2022-JP")
    }

    func testUTF16LEWithBOMText() throws {
        let original = "UTF-16LEのBOM付きテスト\n日本語"
        var data = Data([0xFF, 0xFE])
        data.append(try XCTUnwrap(original.data(using: .utf16LittleEndian)))
        let url = try writeTemp(data)
        guard case .text(let text, let encodingName) = try TextFileLoader.load(url: url) else {
            return XCTFail("UTF-16 LEとして読めるべき")
        }
        XCTAssertEqual(text, original)
        XCTAssertEqual(encodingName, "UTF-16 LE")
    }

    func testUTF8WithBOMText() throws {
        let original = "UTF-8のBOM付きテスト\n日本語"
        var data = Data([0xEF, 0xBB, 0xBF])
        data.append(Data(original.utf8))
        let url = try writeTemp(data)
        guard case .text(let text, let encodingName) = try TextFileLoader.load(url: url) else {
            return XCTFail("UTF-8 (BOM)として読めるべき")
        }
        XCTAssertEqual(text, original)
        XCTAssertEqual(encodingName, "UTF-8 (BOM)")
    }
}
