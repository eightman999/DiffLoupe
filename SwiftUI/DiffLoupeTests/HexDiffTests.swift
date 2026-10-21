// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: Hex差分エンジン(HexDiff)の仕様検証。等値・変更・挿入/削除・サイズ差・空入力・
// 上限読み込み(truncated)・ASCII表現・行単位の差分情報を検証する

import XCTest
@testable import DiffLoupe

final class HexDiffTests: XCTestCase {

    // MARK: - 等値

    func testIdenticalBytesAreEqual() {
        let left = Data([0x00, 0x01, 0x02, 0xFF, 0x41])
        let right = Data([0x00, 0x01, 0x02, 0xFF, 0x41])
        let result = HexDiff.compare(left: left, right: right)
        XCTAssertTrue(result.isEqual)
        XCTAssertFalse(result.truncated)
        XCTAssertEqual(result.rowCount, 1)
        XCTAssertTrue(result.row(at: 0).differingByteOffsets.isEmpty)
    }

    func testDefaultConstants() {
        XCTAssertEqual(HexDiff.bytesPerRow, 16)
        XCTAssertEqual(HexDiff.defaultByteCap, 8 * 1024 * 1024)
    }

    // MARK: - バイト変更

    func testChangedByte() {
        let left = Data([0x41, 0x42, 0x43, 0x44, 0x45, 0x46])
        let right = Data([0x41, 0x42, 0x58, 0x44, 0x45, 0x46])
        let result = HexDiff.compare(left: left, right: right)
        XCTAssertFalse(result.isEqual)
        let row = result.row(at: 0)
        XCTAssertEqual(row.differingByteOffsets, [2])
        XCTAssertEqual(row.leftBytes?[2], 0x43)
        XCTAssertEqual(row.rightBytes?[2], 0x58)
    }

    func testChangedBytesAcrossMultipleRows() {
        let left = Data((0..<40).map { UInt8($0) })
        var right = left
        right[16] = 0xFF
        right[17] = 0xFE
        let result = HexDiff.compare(left: left, right: right)
        XCTAssertEqual(result.rowCount, 3)
        XCTAssertEqual(result.row(at: 0).differingByteOffsets, [])
        XCTAssertEqual(result.row(at: 1).differingByteOffsets, [0, 1])
        XCTAssertEqual(result.row(at: 2).differingByteOffsets, [])
        XCTAssertEqual(result.row(at: 1).offset, 16)
    }

    func testRowExposesBytesAndASCII() {
        let result = HexDiff.compare(left: Data([0x41, 0x42, 0x43]), right: Data([0x41, 0x58, 0x43]))
        let row = result.row(at: 0)
        XCTAssertEqual(row.offset, 0)
        XCTAssertEqual(row.leftBytes, [0x41, 0x42, 0x43])
        XCTAssertEqual(row.rightBytes, [0x41, 0x58, 0x43])
        XCTAssertEqual(row.leftASCII, "ABC")
        XCTAssertEqual(row.rightASCII, "AXC")
        XCTAssertEqual(row.differingByteOffsets, [1])
    }

    // MARK: - 挿入/削除(サイズ差のあるシフト)

    func testInsertionShiftsRemainder() {
        // 左: 41 42 43 44 45 46 / 右: 41 42 58 43 44 45 46 (Xが挿入)
        let left = Data([0x41, 0x42, 0x43, 0x44, 0x45, 0x46])
        let right = Data([0x41, 0x42, 0x58, 0x43, 0x44, 0x45, 0x46])
        let result = HexDiff.compare(left: left, right: right)
        XCTAssertFalse(result.isEqual)
        // 先頭2バイトのみ一致し、以降は行内でずれて異なる
        XCTAssertEqual(result.row(at: 0).differingByteOffsets, [2, 3, 4, 5])
    }

    func testDeletionShiftsRemainder() {
        // 左: 41 42 58 43 44 45 46 / 右: 41 42 43 44 45 46 (Xが削除)
        let left = Data([0x41, 0x42, 0x58, 0x43, 0x44, 0x45, 0x46])
        let right = Data([0x41, 0x42, 0x43, 0x44, 0x45, 0x46])
        let result = HexDiff.compare(left: left, right: right)
        XCTAssertFalse(result.isEqual)
        XCTAssertEqual(result.row(at: 0).differingByteOffsets, [2, 3, 4, 5])
    }

    func testUnequalLengthsShorterSideBecomesNil() {
        let left = Data((0..<20).map { UInt8($0) })
        let right = Data((0..<12).map { UInt8($0) })
        let result = HexDiff.compare(left: left, right: right)
        XCTAssertFalse(result.isEqual)
        XCTAssertEqual(result.rowCount, 2)

        // 1行目: 両側にデータがあり、一致している
        let row0 = result.row(at: 0)
        XCTAssertNotNil(row0.leftBytes)
        XCTAssertNotNil(row0.rightBytes)
        XCTAssertTrue(row0.differingByteOffsets.isEmpty)

        // 2行目: 左のみ存在(offset 16..<20)。右は欠損として nil
        let row1 = result.row(at: 1)
        XCTAssertEqual(row1.offset, 16)
        XCTAssertEqual(row1.leftBytes, [16, 17, 18, 19])
        XCTAssertNil(row1.rightBytes)
        XCTAssertNotNil(row1.leftASCII)
        XCTAssertNil(row1.rightASCII)
    }

    // MARK: - 空入力

    func testEmptyInputsAreEqual() {
        let result = HexDiff.compare(left: Data(), right: Data())
        XCTAssertTrue(result.isEqual)
        XCTAssertEqual(result.rowCount, 0)
        XCTAssertFalse(result.truncated)
    }

    func testEmptyLeftAgainstNonEmpty() {
        let result = HexDiff.compare(left: Data(), right: Data([0x01, 0x02]))
        XCTAssertFalse(result.isEqual)
        XCTAssertEqual(result.rowCount, 1)
        let row = result.row(at: 0)
        XCTAssertNil(row.leftBytes)
        XCTAssertNotNil(row.rightBytes)
        // 片側のみの欠損は「差分」ではなく欠損として扱う
        XCTAssertTrue(row.differingByteOffsets.isEmpty)
    }

    // MARK: - ASCII表現

    func testASCIIRepresentation() {
        XCTAssertEqual(HexDiff.ascii([0x41, 0x42, 0x0A, 0x00, 0x7F, 0x20]), "AB... ")
        // 境界: 0x1F(非印字) / 0x7E(印字 '~') / 0xFF(非印字)
        XCTAssertEqual(HexDiff.ascii([0x1F, 0x7E, 0xFF]), ".~.")
    }

    // MARK: - 上限読み込み(truncated)

    func testExplicitCapTruncatesLargerFiles() {
        let big = Data(repeating: 0xAB, count: 100)
        var changed = big
        changed[50] = 0x01
        let result = HexDiff.compare(left: big, right: changed, maxBytes: 64)
        XCTAssertTrue(result.truncated)
        XCTAssertEqual(result.leftData.count, 64)
        XCTAssertEqual(result.rightData.count, 64)
        XCTAssertEqual(result.rowCount, 4)

        // 差分(50バイト目)は4行目(offset 48)の行内インデックス2に現れる
        let row = result.row(at: 3)
        XCTAssertEqual(row.offset, 48)
        XCTAssertEqual(row.differingByteOffsets, [2])
        XCTAssertEqual(row.leftBytes?[2], 0xAB)
        XCTAssertEqual(row.rightBytes?[2], 0x01)
    }

    func testDefaultCapTruncatesAtEightMiB() {
        let big = Data(repeating: 0x00, count: HexDiff.defaultByteCap + 1)
        let result = HexDiff.compare(left: big, right: big)
        XCTAssertTrue(result.truncated)
        XCTAssertEqual(result.leftData.count, HexDiff.defaultByteCap)
        XCTAssertEqual(result.rightData.count, HexDiff.defaultByteCap)
        // 先頭8MiBは一致していても、truncatedのため完全一致とは判定しない(P1: 上限以降が不明)
        XCTAssertFalse(result.isEqual)
    }

    func testTruncatedSameSizeDifferenceBeyondCapIsNotEqual() {
        // 同サイズ・cap内プレフィックスは一致するが、capを超えた位置でバイトが異なるケース。
        // 読み込んでいない部分が不明なため truncated は決して完全一致を主張しない(P1回帰テスト)
        let left = Data(repeating: 0xAB, count: 100)
        var right = left
        right[80] = 0x01   // cap(64)を超えた位置の差分
        let result = HexDiff.compare(left: left, right: right, maxBytes: 64)
        XCTAssertTrue(result.truncated)
        XCTAssertEqual(result.leftData, result.rightData)   // 先頭64バイトは一致
        XCTAssertFalse(result.isEqual)                      // それでも完全一致とは判定しない
    }

    func testExactCapSizeIsNotTruncated() {
        let data = Data(repeating: 0x00, count: 64)
        let result = HexDiff.compare(left: data, right: data, maxBytes: 64)
        XCTAssertFalse(result.truncated)
        XCTAssertEqual(result.leftData.count, 64)
    }

    // MARK: - URL読み込み

    func testURLCompareEqualFiles() throws {
        let (dir, left, right) = try makeTempPair()
        defer { try? FileManager.default.removeItem(at: dir) }
        try Data([0x00, 0x01, 0x02]).write(to: left)
        try Data([0x00, 0x01, 0x02]).write(to: right)

        let result = try HexDiff.compare(leftURL: left, rightURL: right)
        XCTAssertTrue(result.isEqual)
        XCTAssertEqual(result.leftTotalSize, 3)
        XCTAssertEqual(result.rightTotalSize, 3)
        XCTAssertFalse(result.truncated)
    }

    func testURLCompareReportsTotalSizesBeyondCap() throws {
        let (dir, left, right) = try makeTempPair()
        defer { try? FileManager.default.removeItem(at: dir) }
        try Data(repeating: 0xAB, count: 100).write(to: left)
        try Data(repeating: 0xAB, count: 100).write(to: right)

        let result = try HexDiff.compare(leftURL: left, rightURL: right, maxBytes: 64)
        XCTAssertTrue(result.truncated)
        XCTAssertEqual(result.leftTotalSize, 100)
        XCTAssertEqual(result.rightTotalSize, 100)
        XCTAssertEqual(result.leftData.count, 64)
    }

    func testURLMissingFileThrows() {
        let url = URL(fileURLWithPath: "/nonexistent/\(UUID().uuidString).bin")
        XCTAssertThrowsError(try HexDiff.compare(leftURL: url, rightURL: url))
    }

    // MARK: - ヘルパー

    private func makeTempPair() throws -> (dir: URL, left: URL, right: URL) {
        let dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("HexDiffTests-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        return (
            dir,
            dir.appendingPathComponent("left.bin"),
            dir.appendingPathComponent("right.bin")
        )
    }
}
