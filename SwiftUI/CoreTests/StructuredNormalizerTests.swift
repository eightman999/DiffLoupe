// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: StructuredNormalizer のフォーカステスト。
// JSONキー順・配列順序・スカラフラグメント、XML属性順・空白・コメント、CSV引用符/
// エスケープ・区切り検出、不正入力・サイズ上限のフォールバック、モード/拡張子判定を検証する。

import XCTest
@testable import DiffLoupeCore

final class StructuredNormalizerTests: XCTestCase {

    private func normalize(
        _ text: String, urlString: String? = nil, mode: StructuredComparisonMode = .automatic
    ) -> StructuredNormalizationResult {
        let url = urlString.map { URL(fileURLWithPath: $0) }
        return StructuredNormalizer.normalize(Data(text.utf8), url: url, mode: mode)
    }

    // MARK: - 一時ディレクトリヘルパ(フォルダ比較テスト用)

    private func makeTempPair() throws -> (left: URL, right: URL, cleanup: () -> Void) {
        let base = FileManager.default.temporaryDirectory
            .appendingPathComponent("CoreStructuredNormalizerTests-\(UUID().uuidString)")
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
        left: URL, right: URL, options: FolderComparator.Options = .init()
    ) async throws -> DiffLoupeCore.ComparisonResult {
        try await FolderComparator().compare(left: left, right: right, options: options)
    }

    // MARK: - JSON

    func testJSONKeyOrderIsNormalized() {
        let compact = normalize("{\"b\":2,\"a\":1}", urlString: "/tmp/x.json", mode: .json)
        let pretty = normalize("{\n  \"a\" : 1,\n  \"b\" : 2\n}", urlString: "/tmp/y.json", mode: .json)
        XCTAssertTrue(compact.didNormalize)
        XCTAssertTrue(pretty.didNormalize)
        XCTAssertEqual(compact.normalizedText, pretty.normalizedText)
        XCTAssertEqual(compact.format, .json)
    }

    func testJSONDeterministicPrettyPrint() {
        let result = normalize("{\"b\":2,\"a\":1}", urlString: "/tmp/x.json", mode: .json)
        XCTAssertEqual(
            result.normalizedText,
            "{\n  \"a\" : 1,\n  \"b\" : 2\n}"
        )
    }

    func testJSONArrayOrderIsPreserved() {
        let left = normalize("{\"a\":[1,2,3]}", urlString: "/tmp/l.json", mode: .json)
        let right = normalize("{\"a\":[3,2,1]}", urlString: "/tmp/r.json", mode: .json)
        XCTAssertTrue(left.didNormalize)
        XCTAssertTrue(right.didNormalize)
        XCTAssertNotEqual(left.normalizedText, right.normalizedText)
    }

    func testJSONNestedStructure() {
        let result = normalize(
            "{\"c\":{\"y\":1,\"x\":2},\"a\":[3,1]}",
            urlString: "/tmp/x.json", mode: .json)
        XCTAssertEqual(
            result.normalizedText,
            "{\n  \"a\" : [\n    3,\n    1\n  ],\n  \"c\" : {\n    \"x\" : 2,\n    \"y\" : 1\n  }\n}"
        )
    }

    func testJSONScalarFragments() {
        XCTAssertEqual(normalize("42", urlString: "/tmp/n.json", mode: .json).normalizedText, "42")
        XCTAssertEqual(normalize("\"hi\"", urlString: "/tmp/s.json", mode: .json).normalizedText, "\"hi\"")
        XCTAssertEqual(normalize("true", urlString: "/tmp/b.json", mode: .json).normalizedText, "true")
        XCTAssertEqual(normalize("null", urlString: "/tmp/u.json", mode: .json).normalizedText, "null")
    }

    func testJSONMalformedFallsBack() {
        let result = normalize("{bad json", urlString: "/tmp/x.json", mode: .json)
        XCTAssertFalse(result.didNormalize)
        XCTAssertNil(result.normalizedText)
        XCTAssertNotNil(result.notice)
        XCTAssertEqual(result.format, .json)
    }

    // MARK: - JSON数値の精度(IEEE-754 の往復保持)

    func testJSONNearbyDoublesStayDistinct() {
        // %g(既定6桁)だと 0.1234567 と 0.1234568 が同一値に丸まってしまう境界値。
        // 構造化正規化後も異なるテキストになること。
        let left = normalize("{\"a\":0.1234567}", urlString: "/tmp/l.json", mode: .json)
        let right = normalize("{\"a\":0.1234568}", urlString: "/tmp/r.json", mode: .json)
        XCTAssertTrue(left.didNormalize)
        XCTAssertTrue(right.didNormalize)
        XCTAssertNotEqual(left.normalizedText, right.normalizedText)
    }

    func testJSONFullPrecisionIsRoundTripped() {
        // 0.30000000000000004 は double の有名な非丸め値。%g 6桁だと 0.3 に潰れる。
        let result = normalize("{\"a\":0.30000000000000004}", urlString: "/tmp/x.json", mode: .json)
        XCTAssertTrue(result.didNormalize)
        XCTAssertEqual(result.normalizedText, "{\n  \"a\" : 0.30000000000000004\n}")
    }

    func testJSONIntegralDoubleStillRendersAsInteger() {
        // %g 互換の既存挙動: 整数値の double は "1.0" ではなく "1" と描画する。
        XCTAssertEqual(
            normalize("{\"a\":1.0}", urlString: "/tmp/x.json", mode: .json).normalizedText,
            "{\n  \"a\" : 1\n}"
        )
        XCTAssertEqual(normalize("1.0", urlString: "/tmp/x.json", mode: .json).normalizedText, "1")
    }

    // MARK: - XML

    func testXMLAttributeOrderIsSorted() {
        let left = normalize("<a b=\"2\" a=\"1\"/>", urlString: "/tmp/l.xml", mode: .xml)
        let right = normalize("<a a=\"1\" b=\"2\"/>", urlString: "/tmp/r.xml", mode: .xml)
        XCTAssertTrue(left.didNormalize)
        XCTAssertTrue(right.didNormalize)
        XCTAssertEqual(left.normalizedText, right.normalizedText)
        XCTAssertEqual(left.normalizedText, "<a a=\"1\" b=\"2\"/>\n")
    }

    func testXMLIndentationWhitespaceRemoved() {
        let pretty = normalize("<a>\n  <b/>\n  <c>x</c>\n</a>", urlString: "/tmp/l.xml", mode: .xml)
        let compact = normalize("<a><b/><c>x</c></a>", urlString: "/tmp/r.xml", mode: .xml)
        XCTAssertEqual(pretty.normalizedText, compact.normalizedText)
        XCTAssertEqual(
            pretty.normalizedText,
            "<a>\n  <b/>\n  <c>x</c>\n</a>\n"
        )
    }

    func testXMLTextAndCommentsPreserved() {
        let result = normalize(
            "<a><!-- note --><b value=\"v\">text</b></a>",
            urlString: "/tmp/x.xml", mode: .xml)
        XCTAssertEqual(
            result.normalizedText,
            "<a>\n  <!-- note -->\n  <b value=\"v\">text</b>\n</a>\n"
        )
    }

    func testXMLMixedContentKeepsMeaningfulWhitespace() {
        let result = normalize("<p>Hello <b>world</b>!</p>", urlString: "/tmp/x.xml", mode: .xml)
        // 混合コンテンツでは意味のある空白を保持する(要素専用コンテンツの字下げとは区別)
        XCTAssertTrue(result.didNormalize)
        XCTAssertEqual(result.normalizedText, "<p>\n  Hello \n  <b>world</b>\n  !\n</p>\n")
    }

    func testXMLMalformedFallsBack() {
        let result = normalize("<a><b></a>", urlString: "/tmp/x.xml", mode: .xml)
        XCTAssertFalse(result.didNormalize)
        XCTAssertNil(result.normalizedText)
        XCTAssertNotNil(result.notice)
        XCTAssertEqual(result.format, .xml)
    }

    // MARK: - CSV / TSV

    func testCSVQuotedCommasAndEscapedQuotes() {
        let quotedComma = normalize("a,\"x,y\"\n", urlString: "/tmp/x.csv", mode: .csv)
        XCTAssertTrue(quotedComma.didNormalize)
        XCTAssertEqual(quotedComma.normalizedText, "a x,y\n")

        let escapedQuotes = normalize("a,\"said \"\"hi\"\"\"\n", urlString: "/tmp/x.csv", mode: .csv)
        XCTAssertTrue(escapedQuotes.didNormalize)
        XCTAssertEqual(escapedQuotes.normalizedText, "a \"said \"\"hi\"\"\"\n")
    }

    func testCSVDelimiterDetection() {
        let comma = normalize("a,b\nc,d\n", urlString: "/tmp/x.csv", mode: .automatic)
        XCTAssertEqual(comma.format, .csv)
        XCTAssertEqual(comma.normalizedText, "a b\nc d\n")

        let tab = normalize("a\tb\nc\td\n", urlString: "/tmp/x.tsv", mode: .automatic)
        XCTAssertEqual(tab.format, .csv)
        XCTAssertEqual(tab.normalizedText, "a b\nc d\n")

        let semicolon = normalize("a;b\n", urlString: "/tmp/x.csv", mode: .automatic)
        XCTAssertEqual(semicolon.normalizedText, "a b\n")

        let quotedCommaNotDelimiter = normalize("\"a,b\"\n", urlString: "/tmp/x.csv", mode: .automatic)
        XCTAssertEqual(quotedCommaNotDelimiter.normalizedText, "a,b\n")
    }

    func testCSVUnterminatedQuoteFallsBack() {
        let result = normalize("a,\"unterminated\n", urlString: "/tmp/x.csv", mode: .csv)
        XCTAssertFalse(result.didNormalize)
        XCTAssertNil(result.normalizedText)
        XCTAssertNotNil(result.notice)
        XCTAssertEqual(result.format, .csv)
    }

    func testCSVWhitespaceAroundUnquotedFieldsTrimmed() {
        let left = normalize("a, b \n", urlString: "/tmp/l.csv", mode: .csv)
        let right = normalize("a,b\n", urlString: "/tmp/r.csv", mode: .csv)
        XCTAssertEqual(left.normalizedText, right.normalizedText)
    }

    // MARK: - サイズ上限

    func testSizeCapFallsBackToRaw() {
        let oversized = Data(repeating: 0x20, count: StructuredNormalizer.maxParseBytes + 1)
        let result = StructuredNormalizer.normalize(oversized, url: URL(fileURLWithPath: "/tmp/x.json"), mode: .json)
        XCTAssertFalse(result.didNormalize)
        XCTAssertNil(result.normalizedText)
        XCTAssertNotNil(result.notice)
        XCTAssertEqual(result.format, .json)
    }

    func testAtOrBelowCapIsParsed() {
        // 上限ちょうど手前の小さなデータは通常どおり解析される(上限は十分大きい)
        let result = normalize("{\"a\":1}", urlString: "/tmp/x.json", mode: .json)
        XCTAssertTrue(result.didNormalize)
    }

    // MARK: - モード/拡張子判定

    func testAutomaticModeDetectsByExtension() {
        XCTAssertEqual(normalize("{\"a\":1}", urlString: "/tmp/x.json", mode: .automatic).format, .json)
        XCTAssertEqual(normalize("<a/>", urlString: "/tmp/x.xml", mode: .automatic).format, .xml)
        XCTAssertEqual(normalize("a,b\n", urlString: "/tmp/x.csv", mode: .automatic).format, .csv)
        XCTAssertEqual(normalize("a\tb\n", urlString: "/tmp/x.tsv", mode: .automatic).format, .csv)
    }

    func testAutomaticModeUnknownExtensionFallsBack() {
        let result = normalize("just text", urlString: "/tmp/x.txt", mode: .automatic)
        XCTAssertFalse(result.didNormalize)
        XCTAssertNil(result.format)
        XCTAssertNil(result.notice)
    }

    func testExplicitModeOverridesExtension() {
        let result = normalize("{\"a\":1}", urlString: "/tmp/x.txt", mode: .json)
        XCTAssertTrue(result.didNormalize)
        XCTAssertEqual(result.format, .json)
    }

    func testDisabledModeIsNoOp() {
        let result = normalize("{\"b\":2,\"a\":1}", urlString: "/tmp/x.json", mode: .disabled)
        XCTAssertFalse(result.didNormalize)
        XCTAssertNil(result.format)
        XCTAssertNil(result.normalizedText)
        XCTAssertNil(result.notice)
    }

    // MARK: - UTF-8 BOM 除去(共通入口)

    func testCSVWithAndWithoutUTF8BOMNormalizeEqual() {
        // BOM付きCSVは Data 直接読込経路で先頭フィールドに U+FEFF が残り、
        // BOMなしCSVと構造化比較で相違になる問題の回帰テスト。
        let noBom = normalize("a,b\n\"x,y\",z\n", urlString: "/tmp/n.csv", mode: .csv)
        var bomData = Data([0xEF, 0xBB, 0xBF])
        bomData.append(Data("a,b\n\"x,y\",z\n".utf8))
        let withBom = StructuredNormalizer.normalize(
            bomData, url: URL(fileURLWithPath: "/tmp/b.csv"), mode: .csv)
        XCTAssertTrue(noBom.didNormalize)
        XCTAssertTrue(withBom.didNormalize)
        XCTAssertEqual(noBom.normalizedText, withBom.normalizedText)
    }

    func testUTF8BOMIsStrippedForAllFormats() {
        var jsonBom = Data([0xEF, 0xBB, 0xBF])
        jsonBom.append(Data("{\"b\":2,\"a\":1}".utf8))
        let json = StructuredNormalizer.normalize(
            jsonBom, url: URL(fileURLWithPath: "/tmp/b.json"), mode: .json)
        XCTAssertEqual(
            json.normalizedText,
            normalize("{\"b\":2,\"a\":1}", urlString: "/tmp/x.json", mode: .json).normalizedText
        )
    }

    func testCSVWithBOMIdenticalUnderFolderCompare() async throws {
        let (left, right, cleanup) = try makeTempPair()
        defer { cleanup() }
        var bom = Data([0xEF, 0xBB, 0xBF])
        bom.append(Data("a,b\n\"x,y\",z\n".utf8))
        try write(bom, to: left, "data.csv")
        try write("a,b\n\"x,y\",z\n", to: right, "data.csv")

        var options = FolderComparator.Options()
        options.contentComparison.structuredMode = .csv
        let result = try await compare(left: left, right: right, options: options)
        XCTAssertEqual(result.entriesByPath["data.csv"]?.status, .identical)
    }

    // MARK: - 片側のみパース失敗時のフォールバック(P2-1)

    func testStructuredCanonicalPairRequiresBothSidesSuccess() {
        var comparison = ContentComparison()
        comparison.structuredMode = .json
        let valid = Data("{\"a\":1}".utf8)
        let invalid = Data("{bad json".utf8)
        let leftURL = URL(fileURLWithPath: "/tmp/l.json")
        let rightURL = URL(fileURLWithPath: "/tmp/r.json")

        // 両側成功時のみカノニカルペアが返る
        XCTAssertNotNil(comparison.structuredCanonicalPair(valid, url: leftURL, valid, url: rightURL))
        // 片側でもパース失敗したら nil(呼び出し側は従来比較へ左右両方フォールバック)
        XCTAssertNil(comparison.structuredCanonicalPair(valid, url: leftURL, invalid, url: rightURL))
        XCTAssertNil(comparison.structuredCanonicalPair(invalid, url: leftURL, valid, url: rightURL))
        XCTAssertNil(comparison.structuredCanonicalPair(invalid, url: leftURL, invalid, url: rightURL))

        // disabled では構造化判定を一切行わない
        var disabled = ContentComparison()
        disabled.structuredMode = .disabled
        XCTAssertNil(disabled.structuredCanonicalPair(valid, url: leftURL, valid, url: rightURL))
    }

    func testMixedStructuredPairFallsBackUnderFolderCompare() async throws {
        let (left, right, cleanup) = try makeTempPair()
        defer { cleanup() }
        // 左は正規化成功・右はパース失敗。成功側のcanonicalと失敗側のrawを混在比較せず、
        // structuredMode 無効化の従来比較と同じ判定になること。なお、カノニカル出力は
        // 常に自身のフォーマットとして有効な入力になるため、不正JSONのrawがカノニカルと
        // 偶然一致して「一致」と誤判定されるケースは構造化比較では構成不可能であり、
        // ここではフォールバック契約(両側とも同じ規則で比較)を直接検証する。
        try write("{\"a\":1}", to: left, "data.json")
        try write("{bad json", to: right, "data.json")

        var structuredOptions = FolderComparator.Options()
        structuredOptions.contentComparison.structuredMode = .json
        let structured = try await compare(left: left, right: right, options: structuredOptions)
        let disabled = try await compare(left: left, right: right)
        XCTAssertEqual(structured.entriesByPath["data.json"]?.status, .different)
        XCTAssertEqual(
            structured.entriesByPath["data.json"]?.status,
            disabled.entriesByPath["data.json"]?.status
        )
    }

    // MARK: - 内部CSVパーサ(区切り検出)

    func testDetectDelimiterPreferenceOnTie() {
        // 同数の場合は カンマ > タブ > セミコロン の優先順
        XCTAssertEqual(StructuredNormalizer.detectCSVDelimiter("a,b;c\n"), ",")
        XCTAssertEqual(StructuredNormalizer.detectCSVDelimiter("a\tb;c\n"), "\t")
        XCTAssertEqual(StructuredNormalizer.detectCSVDelimiter("a,b\tc\n"), ",")
        XCTAssertEqual(StructuredNormalizer.detectCSVDelimiter("no delimiter\n"), ",")
    }

    func testCSVParserQuotedMultilineField() throws {
        let rows = try StructuredNormalizer.parseCSV(
            "a,\"multi\nline\"\n", delimiter: ",")
        XCTAssertEqual(rows, [["a", "multi\nline"]])
    }
}
