// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: CLIの --encoding / --list-encodings のテスト。引数解析(正規名への解決・未知の名前のエラー・
// 一覧出力の形式)と、Shift_JISのフィクスチャを自動判定・指定の両方で比較できることを検証する。

import XCTest
@testable import DiffLoupeCLI
@testable import DiffLoupeCore

final class EncodingOptionTests: XCTestCase {

    private var base: URL!

    override func setUpWithError() throws {
        base = FileManager.default.temporaryDirectory
            .appendingPathComponent("DiffLoupeCLIEnc-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: base.appendingPathComponent("l"), withIntermediateDirectories: true)
        try FileManager.default.createDirectory(at: base.appendingPathComponent("r"), withIntermediateDirectories: true)
    }

    override func tearDownWithError() throws { try? FileManager.default.removeItem(at: base) }

    func testEncodingOptionResolvesCanonicalName() throws {
        guard case .options(let o) = CLIOptionsParser.parse(["--encoding", "shift_jis", "a", "b"]) else {
            return XCTFail("options expected")
        }
        XCTAssertEqual(o.contentComparison.textEncoding, TextEncodingCatalog.canonicalName("Shift_JIS"))
        XCTAssertNotNil(o.contentComparison.textEncoding)
        guard case .options(let inline) = CLIOptionsParser.parse(["--encoding=EUC-JP", "a", "b"]) else {
            return XCTFail("options expected")
        }
        XCTAssertEqual(inline.contentComparison.textEncoding, "EUC-JP")
        guard case .options(let none) = CLIOptionsParser.parse(["a", "b"]) else { return XCTFail() }
        XCTAssertNil(none.contentComparison.textEncoding)
    }

    func testUnknownEncodingAndMissingValueAreErrors() {
        XCTAssertEqual(CLIOptionsParser.parse(["--encoding", "nope", "a", "b"]), .error(.unknownEncoding("nope")))
        XCTAssertEqual(CLIOptionsParser.parse(["--encoding"]), .error(.missingValue("--encoding")))
        XCTAssertEqual(CLIError.message(for: .unknownEncoding("nope")),
                       "未対応の文字コードです: nope (--list-encodings で一覧を確認してください)")
    }

    func testListEncodingsNeedsNoPathsAndFormatsTabSeparated() async {
        XCTAssertEqual(CLIOptionsParser.parse(["--list-encodings"]), .listEncodings)
        XCTAssertEqual(CLIOptionsParser.parse(["--help", "--list-encodings"]), .help)
        let lines = TextEncodingCatalog.listLines
        XCTAssertFalse(lines.isEmpty)
        XCTAssertTrue(lines.allSatisfy { $0.split(separator: "\t", omittingEmptySubsequences: false).count == 2 })
        let code = await CLIRunner.run(arguments: ["--list-encodings"], cancellation: CancellationToken())
        XCTAssertEqual(code, 0)
    }

    func testShiftJISFixtureComparesWithAutoAndWithExplicitEncoding() async throws {
        // 大文字小文字だけ違うShift_JISファイル(--ignore-case なら一致)
        try "日本語 ABC\n".data(using: .shiftJIS)!.write(to: base.appendingPathComponent("l/a.txt"))
        try "日本語 abc\n".data(using: .shiftJIS)!.write(to: base.appendingPathComponent("r/a.txt"))
        let l = base.appendingPathComponent("l").path, r = base.appendingPathComponent("r").path
        let strict = await CLIRunner.run(arguments: ["-q", l, r], cancellation: CancellationToken())
        XCTAssertEqual(strict, 1, "厳密比較では相違")
        let auto = await CLIRunner.run(arguments: ["-q", "--ignore-case", l, r], cancellation: CancellationToken())
        XCTAssertEqual(auto, 1, "自動判定ではUTF-8でない内容は従来どおりバイト比較にフォールバックする")
        let forced = await CLIRunner.run(arguments: ["-q", "--ignore-case", "--encoding", "Shift_JIS", l, r],
                                         cancellation: CancellationToken())
        XCTAssertEqual(forced, 0, "文字コードを指定すると正規化比較が効いて一致")
    }
}
