// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: 文字コードの手動指定(TextFileLoader.load(url:encoding:))と対応一覧(TextEncodingCatalog)、
// 詳細表示の一時的な指定状態(TextEncodingOverrides)のフォーカステスト。

import XCTest
@testable import DiffLoupeCore

final class TextEncodingTests: XCTestCase {

    private var base: URL!

    override func setUpWithError() throws {
        base = FileManager.default.temporaryDirectory
            .appendingPathComponent("DiffLoupeEncodingTests-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: base, withIntermediateDirectories: true)
    }

    override func tearDownWithError() throws {
        try? FileManager.default.removeItem(at: base)
    }

    private func write(_ bytes: [UInt8], _ name: String = "f.txt") throws -> URL {
        let url = base.appendingPathComponent(name)
        try Data(bytes).write(to: url)
        return url
    }

    /// カタログ内の識別名を表示名の部分一致で引く(識別名の綴りに依存しないため)
    private func name(containing key: String) throws -> String {
        try XCTUnwrap(TextEncodingCatalog.all.first {
            $0.name.localizedCaseInsensitiveContains(key) || $0.displayName.localizedCaseInsensitiveContains(key)
        }, key).name
    }

    private func loadText(_ url: URL, _ encoding: String) throws -> (String, Bool) {
        guard case .text(let text, _, let replaced) = try TextFileLoader.load(url: url, encoding: encoding) else {
            XCTFail("text expected"); return ("", false)
        }
        return (text, replaced)
    }

    func testShiftJISAndEUCJP() throws {
        // 「日本語」
        let sjis = try write([0x93, 0xFA, 0x96, 0x7B, 0x8C, 0xEA])
        let (t1, r1) = try loadText(sjis, try name(containing: "Shift"))
        XCTAssertEqual(t1, "日本語"); XCTAssertFalse(r1)
        let euc = try write([0xC6, 0xFC, 0xCB, 0xDC, 0xB8, 0xEC], "e.txt")
        let (t2, r2) = try loadText(euc, try name(containing: "EUC-JP"))
        XCTAssertEqual(t2, "日本語"); XCTAssertFalse(r2)
    }

    func testUTF16LEWithoutBOMAndWindows1252() throws {
        let utf16 = try write([0x41, 0x00, 0x42, 0x00, 0x43, 0x00])
        XCTAssertEqual(try loadText(utf16, try name(containing: "UTF-16 LE")).0, "ABC")
        let win = try write([0x63, 0x61, 0x66, 0xE9], "w.txt") // café
        XCTAssertEqual(try loadText(win, try name(containing: "1252")).0, "café")
    }

    func testReplacementFlagAndUnknownEncoding() throws {
        // UTF-8 として不正なバイト(0xFF)を含む
        let bad = try write([0x41, 0xFF, 0x42])
        let (text, replaced) = try loadText(bad, try name(containing: "UTF-8"))
        XCTAssertTrue(replaced)
        XCTAssertTrue(text.contains("\u{FFFD}"))
        XCTAssertThrowsError(try TextFileLoader.load(url: bad, encoding: "no-such-encoding"))
        XCTAssertNil(TextFileLoader.decode(Data([0x41]), encoding: "no-such-encoding"))
        let decoded = try XCTUnwrap(TextFileLoader.decode(Data([0x93, 0xFA]), encoding: try name(containing: "Shift")))
        XCTAssertEqual(decoded.text, "日"); XCTAssertFalse(decoded.hadReplacements)
    }

    func testCatalogContainsJapaneseAndUTF8AndGroups() {
        let names = TextEncodingCatalog.all.map(\.name).joined(separator: "|").lowercased()
        for key in ["utf-8", "shift", "euc-jp", "iso-2022-jp"] {
            XCTAssertTrue(names.contains(key), key)
        }
        let groups = TextEncodingCatalog.groups
        XCTAssertEqual(groups.flatMap(\.options).count, TextEncodingCatalog.all.count)
        XCTAssertEqual(Set(groups.map(\.name)).count, groups.count, "groupは重複しない")
    }

    func testOverridesResetWhenEntryChanges() {
        var o = TextEncodingOverrides()
        XCTAssertNil(o.effective(for: "a.txt").left)
        o.select("Shift_JIS", side: .left, entryPath: "a.txt")
        XCTAssertEqual(o.effective(for: "a.txt").left, "Shift_JIS")
        XCTAssertNil(o.effective(for: "a.txt").right)
        XCTAssertNil(o.effective(for: "b.txt").left, "別エントリでは自動判定")
        o.select("EUC-JP", side: .right, entryPath: "b.txt")
        XCTAssertNil(o.effective(for: "b.txt").left, "前エントリの指定は持ち越さない")
        XCTAssertEqual(o.effective(for: "b.txt").right, "EUC-JP")
        o.select(nil, side: .right, entryPath: "b.txt")
        XCTAssertNil(o.effective(for: "b.txt").right)
    }

    // MARK: - 3-way

    func testThreeWayLoaderReadsOnlySpecifiedFileWithEncoding() throws {
        let sjis = try write([0x93, 0xFA, 0x96, 0x7B, 0x8C, 0xEA], "base.txt")
        let utf8 = try write(Array("日本語".utf8), "left.txt")
        // 自動判定(Shift_JISとして読める)
        let auto = try ThreeWayTextLoader.load(url: sjis, encoding: nil, label: "ベース")
        XCTAssertEqual(auto.text, "日本語"); XCTAssertFalse(auto.hadReplacements)
        // 指定ありのファイルだけ指定どおりに読み、他方は自動判定のまま
        let forced = try ThreeWayTextLoader.load(url: sjis, encoding: try name(containing: "Shift"), label: "ベース")
        XCTAssertEqual(forced.text, "日本語")
        let other = try ThreeWayTextLoader.load(url: utf8, encoding: nil, label: "左")
        XCTAssertEqual(other.text, "日本語")
        // UTF-8として読めないバイトは置換される
        let replaced = try ThreeWayTextLoader.load(url: sjis, encoding: try name(containing: "UTF-8"), label: "ベース")
        XCTAssertTrue(replaced.hadReplacements)
        // 自動判定でバイナリ扱いのファイルはエラー、文字コードを指定すれば読める
        let bin = try write([0x00, 0x01, 0x41, 0x00, 0x02], "bin.dat")
        XCTAssertThrowsError(try ThreeWayTextLoader.load(url: bin, encoding: nil, label: "右"))
        XCTAssertNoThrow(try ThreeWayTextLoader.load(url: bin, encoding: try name(containing: "UTF-8"), label: "右"))
    }

    func testThreeWayOverridesFollowPath() {
        var o = ThreeWayEncodingOverrides()
        o.select("Shift_JIS", role: .left, path: "/a.txt")
        XCTAssertEqual(o.effective(.left, path: "/a.txt"), "Shift_JIS")
        XCTAssertNil(o.effective(.left, path: "/b.txt"), "ファイルを選び直したら自動判定")
        XCTAssertNil(o.effective(.right, path: "/a.txt"))
        o.select(nil, role: .left, path: "/a.txt")
        XCTAssertNil(o.effective(.left, path: "/a.txt"))
    }
}
