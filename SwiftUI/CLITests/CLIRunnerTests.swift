// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: CLIRunner.run(公開エントリポイント)の統合テスト。
// 一時フォルダ対を作成し、終了コード・再帰既定/-r・レポート作成と上書き拒否・
// 非ディレクトリ引数エラー・WinMergeスラッシュスイッチ(/or)を実ファイルで検証する。
// stdoutのインターセプトは行わず、戻り値の終了コードとファイルシステム状態を検証する。

import XCTest
import DiffLoupeCLI
import DiffLoupeCore

final class CLIRunnerTests: XCTestCase {

    private var tempBase: URL!
    private var left: URL!
    private var right: URL!

    override func setUpWithError() throws {
        tempBase = FileManager.default.temporaryDirectory
            .appendingPathComponent("DiffLoupeCLIRunner-\(UUID().uuidString)", isDirectory: true)
        left = tempBase.appendingPathComponent("left")
        right = tempBase.appendingPathComponent("right")
        try FileManager.default.createDirectory(at: left, withIntermediateDirectories: true)
        try FileManager.default.createDirectory(at: right, withIntermediateDirectories: true)
    }

    override func tearDownWithError() throws {
        try? FileManager.default.removeItem(at: tempBase)
    }

    private func write(_ text: String, to root: URL, _ relative: String) throws {
        let url = root.appendingPathComponent(relative)
        try FileManager.default.createDirectory(
            at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
        try text.data(using: .utf8)!.write(to: url)
    }

    private func run(_ arguments: [String]) async -> Int32 {
        await CLIRunner.run(arguments: arguments, cancellation: CancellationToken())
    }

    // MARK: - 終了コード(0=一致 / 1=差異)

    func testEqualTreesReturnSuccess() async throws {
        try write("same", to: left, "a.txt")
        try write("same", to: right, "a.txt")
        let code = await run(["-q", left.path, right.path])
        XCTAssertEqual(code, 0)
    }

    func testDifferentFilesReturnDifferences() async throws {
        try write("AAA", to: left, "a.txt")
        try write("BBB", to: right, "a.txt")
        let code = await run(["-q", left.path, right.path])
        XCTAssertEqual(code, 1)
    }

    // MARK: - 構造化比較(--structured)

    func testStructuredJSONKeyOrderTreatedAsIdentical() async throws {
        try write("{\"b\":2,\"a\":1}", to: left, "data.json")
        try write("{\"a\":1,\"b\":2}", to: right, "data.json")

        // 既定(厳密)ではキー順の違いを差異として検出
        let strict = await run(["-q", left.path, right.path])
        XCTAssertEqual(strict, 1)

        // --structured json ではキー順の表記揺れを正規化して一致扱い
        let structured = await run(["-q", "--structured", "json", left.path, right.path])
        XCTAssertEqual(structured, 0)

        // エイリアス --normalize-json も同様
        let alias = await run(["-q", "--normalize-json", left.path, right.path])
        XCTAssertEqual(alias, 0)
    }

    func testStructuredXMLAttributeOrderTreatedAsIdentical() async throws {
        try write("<a b=\"2\" a=\"1\"/>", to: left, "data.xml")
        try write("<a a=\"1\" b=\"2\"/>", to: right, "data.xml")

        let strict = await run(["-q", left.path, right.path])
        XCTAssertEqual(strict, 1)

        let structured = await run(["-q", "--structured", "xml", left.path, right.path])
        XCTAssertEqual(structured, 0)
    }

    func testStructuredCSVQuotingTreatedAsIdentical() async throws {
        try write("a,b\n\"x,y\",z\n", to: left, "data.csv")
        try write("a,b\n\"x,y\",z\n", to: right, "data.csv")

        // 同一内容は厳密でも一致
        let strict = await run(["-q", left.path, right.path])
        XCTAssertEqual(strict, 0)

        let structured = await run(["-q", "--structured", "csv", left.path, right.path])
        XCTAssertEqual(structured, 0)
    }

    func testStructuredModeStillDetectsRealDifference() async throws {
        // キー順だけでなく実値の違いは構造化モードでも検出される
        try write("{\"a\":1,\"b\":2}", to: left, "data.json")
        try write("{\"a\":1,\"b\":99}", to: right, "data.json")

        let structured = await run(["-q", "--structured", "json", left.path, right.path])
        XCTAssertEqual(structured, 1)
    }

    func testStructuredAutoModeUsesExtension() async throws {
        try write("{\"b\":2,\"a\":1}", to: left, "data.json")
        try write("{\"a\":1,\"b\":2}", to: right, "data.json")

        // auto は拡張子 .json から構造化比較を適用して一致扱い
        let auto = await run(["-q", "--structured", "auto", left.path, right.path])
        XCTAssertEqual(auto, 0)
    }

    // MARK: - 再帰既定 vs -r / /r

    func testNonRecursiveDefaultIgnoresNestedDifference() async throws {
        try write("AAA", to: left, "sub/nested.txt")
        try write("BBB", to: right, "sub/nested.txt")

        // 既定は非再帰: トップレベルのディレクトリのみ比較し、中身は見ない → 一致
        let defaultCode = await run(["-q", left.path, right.path])
        XCTAssertEqual(defaultCode, 0)

        // -r で再帰: ネストした相違を検出 → 1
        let recursiveCode = await run(["-q", "-r", left.path, right.path])
        XCTAssertEqual(recursiveCode, 1)

        // WinMerge /r スラッシュエイリアスも再帰を有効化 → 1
        let slashCode = await run(["-q", "/r", left.path, right.path])
        XCTAssertEqual(slashCode, 1)
    }

    // MARK: - エラー系

    func testNonDirectoryArgumentReturnsUsageError() async throws {
        try write("file", to: left, "a.txt")
        let code = await run(["-q", left.path + "/a.txt", right.path])
        XCTAssertEqual(code, 2)
    }

    // MARK: - /or レポート出力と上書き拒否

    func testOrReportFileCreatedAndOverwriteRefused() async throws {
        try write("AAA", to: left, "a.txt")
        try write("BBB", to: right, "a.txt")
        let reportPath = tempBase.appendingPathComponent("report.html").path

        // /or でレポートをファイルへ書き込み、差異があるので1
        let first = await run(["-q", "/or", reportPath, left.path, right.path])
        XCTAssertEqual(first, 1)
        XCTAssertTrue(FileManager.default.fileExists(atPath: reportPath))

        // 既存ファイルへの上書きは拒否 → 2(検証は比較前に行われる)
        let second = await run(["-q", "/or", reportPath, left.path, right.path])
        XCTAssertEqual(second, 2)
    }

    func testSlashOrWithUnknownExtensionFallsBackToHTML() async throws {
        try write("AAA", to: left, "a.txt")
        try write("BBB", to: right, "a.txt")
        let reportPath = tempBase.appendingPathComponent("report.txt").path

        let code = await run(["-q", "/or", reportPath, left.path, right.path])
        XCTAssertEqual(code, 1)
        XCTAssertTrue(FileManager.default.fileExists(atPath: reportPath))

        let text = try String(contentsOfFile: reportPath, encoding: .utf8)
        XCTAssertTrue(text.contains("<!DOCTYPE html>"))
    }

    // MARK: - /enableexitcode

    func testEnableExitCodeAcceptsAndKeepsDifferencesMapping() async throws {
        try write("AAA", to: left, "a.txt")
        try write("BBB", to: right, "a.txt")
        // 差異はネイティブ1 = WinMerge写像でも1
        let code = await run(["-q", "/enableexitcode", left.path, right.path])
        XCTAssertEqual(code, 1)
    }

    func testEnableExitCodeMapsUsageErrorToWinMerge2() async throws {
        try write("file", to: left, "a.txt")
        // 非ディレクトリ引数(ネイティブ2)はWinMerge写像でも2
        let code = await run(["-q", "/enableexitcode", left.path + "/a.txt", right.path])
        XCTAssertEqual(code, 2)
    }
}
