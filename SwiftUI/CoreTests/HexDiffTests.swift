// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: Hex差分エンジン(HexDiff)のSwiftPMフォーカステスト。UIを含まないコアエンジンの
// 等値・変更・挿入/削除/サイズ差・空入力・上限読み込み(truncated)を検証する
// (XcodeのUIテストスイートの重複はせず、SwiftPMだけで検証すべき最小限のコア挙動を扱う)

import XCTest
@testable import DiffLoupeCore

final class HexDiffTests: XCTestCase {

    func testIdenticalBytesAreEqual() {
        let result = HexDiff.compare(left: Data([0x00, 0x01, 0x02]), right: Data([0x00, 0x01, 0x02]))
        XCTAssertTrue(result.isEqual)
        XCTAssertEqual(result.rowCount, 1)
        XCTAssertTrue(result.row(at: 0).differingByteOffsets.isEmpty)
    }

    func testChangedByteIsReportedPerRow() {
        let result = HexDiff.compare(left: Data([0x41, 0x42, 0x43]), right: Data([0x41, 0x58, 0x43]))
        XCTAssertFalse(result.isEqual)
        XCTAssertEqual(result.row(at: 0).differingByteOffsets, [1])
        XCTAssertEqual(result.row(at: 0).leftBytes?[1], 0x42)
        XCTAssertEqual(result.row(at: 0).rightBytes?[1], 0x58)
    }

    func testInsertAndDeleteShiftRemainder() {
        let left = Data([0x41, 0x42, 0x43, 0x44, 0x45])
        let inserted = Data([0x41, 0x42, 0x58, 0x43, 0x44, 0x45])
        let deleted = Data([0x43, 0x44, 0x45])
        XCTAssertEqual(HexDiff.compare(left: left, right: inserted).row(at: 0).differingByteOffsets, [2, 3, 4])
        XCTAssertEqual(HexDiff.compare(left: left, right: deleted).row(at: 0).differingByteOffsets, [0, 1, 2])
    }

    func testUnequalLengthsShorterSideBecomesNil() {
        let left = Data((0..<20).map { UInt8($0) })
        let right = Data((0..<12).map { UInt8($0) })
        let result = HexDiff.compare(left: left, right: right)
        XCTAssertFalse(result.isEqual)
        XCTAssertEqual(result.rowCount, 2)
        XCTAssertTrue(result.row(at: 0).differingByteOffsets.isEmpty)
        XCTAssertEqual(result.row(at: 1).offset, 16)
        XCTAssertEqual(result.row(at: 1).leftBytes, [16, 17, 18, 19])
        XCTAssertNil(result.row(at: 1).rightBytes)
    }

    func testEmptyInputsAreEqual() {
        let result = HexDiff.compare(left: Data(), right: Data())
        XCTAssertTrue(result.isEqual)
        XCTAssertEqual(result.rowCount, 0)
        XCTAssertFalse(result.truncated)
    }

    func testBoundedReadsReportTruncation() {
        let big = Data(repeating: 0xAB, count: 100)
        var changed = big
        changed[50] = 0x01
        let result = HexDiff.compare(left: big, right: changed, maxBytes: 64)
        XCTAssertTrue(result.truncated)
        XCTAssertEqual(result.leftData.count, 64)
        XCTAssertEqual(result.rightData.count, 64)
        XCTAssertEqual(result.rowCount, 4)
        // 差分(50バイト目)は4行目(offset 48)の行内インデックス2に現れる
        XCTAssertEqual(result.row(at: 3).offset, 48)
        XCTAssertEqual(result.row(at: 3).differingByteOffsets, [2])
    }

    func testExactCapSizeIsNotTruncated() {
        let data = Data(repeating: 0x00, count: 64)
        XCTAssertFalse(HexDiff.compare(left: data, right: data, maxBytes: 64).truncated)
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

    func testDefaultCapIsEightMiB() {
        XCTAssertEqual(HexDiff.defaultByteCap, 8 * 1024 * 1024)
        XCTAssertEqual(HexDiff.bytesPerRow, 16)
    }
}
