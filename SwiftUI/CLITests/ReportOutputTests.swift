// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: レポート出力(文字列化・stdout条件・ファイル書き込み条件)の純粋テスト。
// 既存のComparisonReport再利用・上書き拒否・親ディレクトリ欠如エラーを検証する。

import XCTest
@testable import DiffLoupeCLI
@testable import DiffLoupeCore

final class ReportOutputTests: XCTestCase {

    private var tempDirectory: URL!

    override func setUpWithError() throws {
        tempDirectory = FileManager.default.temporaryDirectory
            .appendingPathComponent("DiffLoupeCLITests-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(
            at: tempDirectory, withIntermediateDirectories: true)
    }

    override func tearDownWithError() throws {
        if let tempDirectory {
            try? FileManager.default.removeItem(at: tempDirectory)
        }
    }

    private func sampleResult() -> DiffLoupeCore.ComparisonResult {
        TestResultFactory.result(entries: [
            TestResultFactory.entry(path: "same.txt", status: .identical),
            TestResultFactory.entry(path: "changed.txt", status: .different),
            TestResultFactory.entry(path: "only_left.txt", status: .leftOnly),
            TestResultFactory.entry(path: "only_right.txt", status: .rightOnly),
            TestResultFactory.entry(path: "broken.txt", status: .error, errorMessage: "読み込み不可"),
        ])
    }

    // MARK: - 文字列化(既存フォーマッタ再利用)

    func testJSONReportTextContainsSummary() {
        let text = CLIReporter.reportText(result: sampleResult(), format: .json)
        XCTAssertTrue(text.contains("DiffLoupeComparisonReport"))
        XCTAssertTrue(text.contains("\"differentFiles\" : 1"))
        XCTAssertTrue(text.contains("\"identicalFiles\" : 1"))
    }

    func testCSVReportTextContainsHeader() {
        let text = CLIReporter.reportText(result: sampleResult(), format: .csv)
        XCTAssertTrue(text.contains("レポート種別,DiffLoupe比較レポート"))
        XCTAssertTrue(text.contains("相対パス,状態,種別,左サイズ,右サイズ"))
        XCTAssertTrue(text.contains("changed.txt,相違"))
    }

    func testMarkdownReportTextContainsSummaryTable() {
        let text = CLIReporter.reportText(result: sampleResult(), format: .markdown)
        XCTAssertTrue(text.contains("## サマリ"))
        XCTAssertTrue(text.contains("## 詳細"))
        XCTAssertTrue(text.contains("| 一致 | 1 |"))
    }

    func testHTMLReportTextIsStandalone() {
        let text = CLIReporter.reportText(result: sampleResult(), format: .html)
        XCTAssertTrue(text.hasPrefix("<!DOCTYPE html>"))
        XCTAssertTrue(text.contains("<h1>DiffLoupe比較レポート</h1>"))
        XCTAssertTrue(text.contains("</html>"))
    }

    // MARK: - ファイル書き込み条件

    func testWriteCreatesFileWithUTF8Text() throws {
        let path = tempDirectory.appendingPathComponent("report.csv").path
        try CLIReporter.write("abc,def\n", to: path)
        let data = try Data(contentsOf: URL(fileURLWithPath: path))
        XCTAssertEqual(String(data: data, encoding: .utf8), "abc,def\n")
    }

    func testWriteRefusesExistingFile() {
        let path = tempDirectory.appendingPathComponent("report.json").path
        XCTAssertNoThrow(try CLIReporter.write("first", to: path))
        XCTAssertThrowsError(try CLIReporter.write("second", to: path)) { error in
            XCTAssertEqual(error as? CLIOutputError, .fileExists(path))
        }
        // 既存ファイルは上書きされないこと
        let data = try? Data(contentsOf: URL(fileURLWithPath: path))
        XCTAssertEqual(String(data: data ?? Data(), encoding: .utf8), "first")
    }

    func testWriteRefusesMissingParentDirectory() {
        let path = tempDirectory
            .appendingPathComponent("no-such-dir/report.json", isDirectory: false).path
        XCTAssertThrowsError(try CLIReporter.write("x", to: path)) { error in
            XCTAssertEqual(error as? CLIOutputError, .parentDirectoryMissing(path))
        }
    }

    func testValidateOutputAcceptsNewPathInExistingDirectory() throws {
        let path = tempDirectory.appendingPathComponent("new.md").path
        XCTAssertNoThrow(try CLIReporter.validateOutput(path: path))
    }
}
