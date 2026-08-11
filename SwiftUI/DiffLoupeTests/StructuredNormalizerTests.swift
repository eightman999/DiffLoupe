// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: 構造化比較(JSON/XML/CSV)のフォルダ分類テスト。
// StructuredNormalizerを内容比較へ統合し、表記揺れ(キー順・属性順・引用)が
// 一致扱いになること・実値の違いや不正ファイルは従来どおり扱われることを検証する。
// 既定(disabled)では従来の厳密挙動を維持する。

import XCTest
@testable import DiffLoupe

final class StructuredNormalizerTests: XCTestCase {

    // MARK: - 一時ディレクトリヘルパ

    private func makeTempPair() throws -> (left: URL, right: URL, cleanup: () -> Void) {
        let base = FileManager.default.temporaryDirectory
            .appendingPathComponent("StructuredNormalizerTests-\(UUID().uuidString)")
        let left = base.appendingPathComponent("left")
        let right = base.appendingPathComponent("right")
        try FileManager.default.createDirectory(at: left, withIntermediateDirectories: true)
        try FileManager.default.createDirectory(at: right, withIntermediateDirectories: true)
        return (left, right, { try? FileManager.default.removeItem(at: base) })
    }

    private func write(_ data: Data, to dir: URL, _ relative: String) throws {
        let url = dir.appendingPathComponent(relative)
        try FileManager.default.createDirectory(
            at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
        try data.write(to: url)
    }

    private func write(_ text: String, to dir: URL, _ relative: String) throws {
        try write(Data(text.utf8), to: dir, relative)
    }

    private func compare(
        left: URL, right: URL,
        options: FolderComparator.Options = .init(),
        progress: FolderComparator.ProgressHandler? = nil
    ) async throws -> ComparisonResult {
        try await FolderComparator().compare(left: left, right: right, options: options, progress: progress)
    }

    private func structuredOptions(_ mode: StructuredComparisonMode) -> FolderComparator.Options {
        var options = FolderComparator.Options()
        options.contentComparison.structuredMode = mode
        return options
    }

    // MARK: - JSON

    func testJSONKeyOrderIsIdenticalUnderStructuredMode() async throws {
        let (left, right, cleanup) = try makeTempPair()
        defer { cleanup() }
        try write("{\"b\":2,\"a\":1}", to: left, "data.json")
        try write("{\"a\":1,\"b\":2}", to: right, "data.json")

        // 既定(disabled)は従来どおり厳密比較で「相違」
        let strict = try await compare(left: left, right: right)
        XCTAssertEqual(strict.entriesByPath["data.json"]?.status, .different)

        // 構造化比較ではキー順の表記揺れを正規化して「一致」
        let structured = try await compare(left: left, right: right, options: structuredOptions(.json))
        XCTAssertEqual(structured.entriesByPath["data.json"]?.status, .identical)
    }

    func testJSONRealDifferenceStillDetected() async throws {
        let (left, right, cleanup) = try makeTempPair()
        defer { cleanup() }
        try write("{\"a\":1,\"b\":2}", to: left, "data.json")
        try write("{\"a\":1,\"b\":99}", to: right, "data.json")

        let result = try await compare(left: left, right: right, options: structuredOptions(.json))
        XCTAssertEqual(result.entriesByPath["data.json"]?.status, .different)
    }

    func testJSONArrayOrderDifferenceStillDetected() async throws {
        let (left, right, cleanup) = try makeTempPair()
        defer { cleanup() }
        try write("{\"a\":[1,2,3]}", to: left, "data.json")
        try write("{\"a\":[3,2,1]}", to: right, "data.json")

        // 配列の順序は意味を持つため構造化比較でも「相違」
        let result = try await compare(left: left, right: right, options: structuredOptions(.json))
        XCTAssertEqual(result.entriesByPath["data.json"]?.status, .different)
    }

    // MARK: - XML

    func testXMLAttributeOrderIsIdenticalUnderStructuredMode() async throws {
        let (left, right, cleanup) = try makeTempPair()
        defer { cleanup() }
        try write("<a b=\"2\" a=\"1\"/>", to: left, "data.xml")
        try write("<a a=\"1\" b=\"2\"/>", to: right, "data.xml")

        let strict = try await compare(left: left, right: right)
        XCTAssertEqual(strict.entriesByPath["data.xml"]?.status, .different)

        let structured = try await compare(left: left, right: right, options: structuredOptions(.xml))
        XCTAssertEqual(structured.entriesByPath["data.xml"]?.status, .identical)
    }

    func testXMLIndentationOnlyDifferenceIsIdentical() async throws {
        let (left, right, cleanup) = try makeTempPair()
        defer { cleanup() }
        try write("<a>\n  <b/>\n  <c>x</c>\n</a>", to: left, "data.xml")
        try write("<a><b/><c>x</c></a>", to: right, "data.xml")

        let structured = try await compare(left: left, right: right, options: structuredOptions(.xml))
        XCTAssertEqual(structured.entriesByPath["data.xml"]?.status, .identical)
    }

    // MARK: - CSV

    func testCSVQuotingDifferenceIsIdenticalUnderStructuredMode() async throws {
        let (left, right, cleanup) = try makeTempPair()
        defer { cleanup() }
        try write("a,b\n\"x,y\",z\n", to: left, "data.csv")
        try write("a,b\n\"x,y\",z\n", to: right, "data.csv")

        let structured = try await compare(left: left, right: right, options: structuredOptions(.csv))
        XCTAssertEqual(structured.entriesByPath["data.csv"]?.status, .identical)
    }

    // MARK: - モード / フォールバック

    func testAutomaticModeUsesExtension() async throws {
        let (left, right, cleanup) = try makeTempPair()
        defer { cleanup() }
        try write("{\"b\":2,\"a\":1}", to: left, "data.json")
        try write("{\"a\":1,\"b\":2}", to: right, "data.json")

        let result = try await compare(left: left, right: right, options: structuredOptions(.automatic))
        XCTAssertEqual(result.entriesByPath["data.json"]?.status, .identical)
    }

    func testMalformedStructuredFileFallsBackToRawCompare() async throws {
        let (left, right, cleanup) = try makeTempPair()
        defer { cleanup() }
        // 壊れたJSONは正規化できず、バイト厳密比較へフォールバック
        try write("{bad json", to: left, "data.json")
        try write("{bad json", to: right, "data.json")

        let same = try await compare(left: left, right: right, options: structuredOptions(.json))
        XCTAssertEqual(same.entriesByPath["data.json"]?.status, .identical)

        try write("{bad json!", to: right, "data.json")
        let different = try await compare(left: left, right: right, options: structuredOptions(.json))
        XCTAssertEqual(different.entriesByPath["data.json"]?.status, .different)
    }

    func testHashEqualFilesStayIdenticalUnderStructuredMode() async throws {
        let (left, right, cleanup) = try makeTempPair()
        defer { cleanup() }
        // ハッシュ一致のファイルは構造化比較でも従来どおり「一致」(回帰防止)
        try write("same bytes", to: left, "data.json")
        try write("same bytes", to: right, "data.json")

        let result = try await compare(left: left, right: right, options: structuredOptions(.json))
        XCTAssertEqual(result.entriesByPath["data.json"]?.status, .identical)
    }

    func testDisabledModePreservesStrictBehavior() async throws {
        let (left, right, cleanup) = try makeTempPair()
        defer { cleanup() }
        try write("{\"b\":2,\"a\":1}", to: left, "data.json")
        try write("{\"a\":1,\"b\":2}", to: right, "data.json")

        // disabled は従来の厳密比較のまま(既定と同一)
        let result = try await compare(left: left, right: right, options: structuredOptions(.disabled))
        XCTAssertEqual(result.entriesByPath["data.json"]?.status, .different)
    }

    // MARK: - 精度・BOM・片側失敗フォールバック(GL M5.2監査の回帰テスト)

    func testJSONNearbyDoublesDetectedUnderStructuredMode() async throws {
        let (left, right, cleanup) = try makeTempPair()
        defer { cleanup() }
        // %g(既定6桁)では両者とも 0.123457 に丸まって一致扱いになる境界値。
        // 構造化正規化後も区別され「相違」になること。
        try write("{\"a\":0.1234567}", to: left, "data.json")
        try write("{\"a\":0.1234568}", to: right, "data.json")

        let result = try await compare(left: left, right: right, options: structuredOptions(.json))
        XCTAssertEqual(result.entriesByPath["data.json"]?.status, .different)
    }

    func testCSVUTF8BOMIsEquivalentToNoBOMUnderStructuredMode() async throws {
        let (left, right, cleanup) = try makeTempPair()
        defer { cleanup() }
        // 共通normalize入口でBOMを除去するため、BOM有無は表記揺れとして正規化し「一致」。
        var bom = Data([0xEF, 0xBB, 0xBF])
        bom.append(Data("a,b\n\"x,y\",z\n".utf8))
        try write(bom, to: left, "data.csv")
        try write("a,b\n\"x,y\",z\n", to: right, "data.csv")

        let result = try await compare(left: left, right: right, options: structuredOptions(.csv))
        XCTAssertEqual(result.entriesByPath["data.csv"]?.status, .identical)
    }

    func testStructuredMixedSuccessFallsBackToRawComparison() async throws {
        let (left, right, cleanup) = try makeTempPair()
        defer { cleanup() }
        // 左は正規化成功・右はパース失敗。成功側のcanonicalと失敗側のrawを混在比較せず、
        // structuredMode 無効化の従来比較と同じ判定になること(フォールバック契約)。
        try write("{\"a\":1}", to: left, "data.json")
        try write("{bad json", to: right, "data.json")

        let structured = try await compare(left: left, right: right, options: structuredOptions(.json))
        let disabled = try await compare(left: left, right: right, options: structuredOptions(.disabled))
        XCTAssertEqual(structured.entriesByPath["data.json"]?.status, .different)
        XCTAssertEqual(
            structured.entriesByPath["data.json"]?.status,
            disabled.entriesByPath["data.json"]?.status
        )
    }

    // MARK: - 詳細差分と設定署名

    func testStructuredModeDisablesStrictFlag() {
        var comparison = ContentComparison()
        XCTAssertTrue(comparison.isStrict)
        XCTAssertEqual(comparison.settingsSignature, "0000")
        comparison.structuredMode = .json
        XCTAssertFalse(comparison.isStrict)
        // 構造化モードは署名の末尾へ追記される
        XCTAssertEqual(comparison.settingsSignature, "0000|json")
    }

    func testStructuredModePersistsInUserDefaults() throws {
        let suiteName = "StructuredNormalizerTests-\(UUID().uuidString)"
        let defaults = UserDefaults(suiteName: suiteName)!
        defer { defaults.removePersistentDomain(forName: suiteName) }
        defaults.register(defaults: AppModel.defaultSettings)

        // 既定は disabled(後方互換)
        XCTAssertEqual(AppModel.structuredMode(defaults: defaults), .disabled)

        defaults.set("xml", forKey: AppModel.structuredModeKey)
        XCTAssertEqual(AppModel.structuredMode(defaults: defaults), .xml)

        // 不明な値は disabled へフォールバック
        defaults.set("bogus", forKey: AppModel.structuredModeKey)
        XCTAssertEqual(AppModel.structuredMode(defaults: defaults), .disabled)
    }
}
