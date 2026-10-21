// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: 比較レポートフォーマッタ(ComparisonReport)の純粋テスト。
// JSON構造・CSVのRFC4180相当引用・Markdown/HTMLのエスケープ・行数・ISO8601日時を検証する

import XCTest
@testable import DiffLoupe

final class ComparisonReportTests: XCTestCase {

    private func entry(
        path: String,
        status: DiffStatus,
        isDirectory: Bool = false,
        leftSize: Int64 = 0,
        rightSize: Int64 = 0,
        leftDate: Date? = nil,
        rightDate: Date? = nil,
        errorMessage: String? = nil,
        renameCandidate: RenameCandidate? = nil
    ) -> ComparedEntry {
        ComparedEntry(
            relativePath: path,
            name: (path as NSString).lastPathComponent,
            isDirectory: isDirectory,
            status: status,
            leftURL: nil,
            rightURL: nil,
            leftSize: leftSize,
            rightSize: rightSize,
            errorMessage: errorMessage,
            leftModificationDate: leftDate,
            rightModificationDate: rightDate,
            leftPOSIX: nil,
            rightPOSIX: nil,
            renameCandidate: renameCandidate
        )
    }

    private func makeResult(entries: [ComparedEntry] = []) -> ComparisonResult {
        var summary = ComparisonResult.Summary()
        for e in entries where !e.isDirectory {
            switch e.status {
            case .identical: summary.identicalFiles += 1
            case .different: summary.differentFiles += 1
            case .leftOnly: summary.leftOnlyFiles += 1
            case .rightOnly: summary.rightOnlyFiles += 1
            case .error: summary.errorFiles += 1
            }
        }
        return ComparisonResult(
            leftRoot: URL(fileURLWithPath: "/tmp/left", isDirectory: true),
            rightRoot: URL(fileURLWithPath: "/tmp/right", isDirectory: true),
            entries: entries,
            entriesByPath: Dictionary(uniqueKeysWithValues: entries.map { ($0.relativePath, $0) }),
            rootNode: DiffNode(id: "", name: "", isDirectory: true, status: .different, children: nil),
            summary: summary
        )
    }

    private func report(
        entries: [ComparedEntry] = [],
        date: Date = Date(timeIntervalSince1970: 1_700_000_000)
    ) -> ComparisonReport {
        ComparisonReport(result: makeResult(entries: entries), generatedAt: date)
    }

    // MARK: - JSON

    func testJSONStructure() throws {
        let entries = [
            entry(path: "a.txt", status: .identical, leftSize: 3, rightSize: 3),
            entry(path: "sub/b,1.txt", status: .different, leftSize: 1, rightSize: 2,
                  leftDate: Date(timeIntervalSince1970: 1_700_000_000),
                  rightDate: Date(timeIntervalSince1970: 1_700_000_001),
                  errorMessage: "読み込みエラー"),
        ]
        let output = report(entries: entries).render(as: .json)
        let data = try XCTUnwrap(output.data(using: .utf8))
        let object = try XCTUnwrap(JSONSerialization.jsonObject(with: data) as? [String: Any])

        XCTAssertEqual(object["format"] as? String, "DiffLoupeComparisonReport")
        XCTAssertEqual(object["version"] as? Int, 1)
        XCTAssertEqual(object["generatedAt"] as? String, "2023-11-14T22:13:20Z")
        let left = try XCTUnwrap(object["left"] as? [String: Any])
        XCTAssertEqual(left["path"] as? String, "/tmp/left")
        XCTAssertTrue(left["gitRef"] is NSNull)
        let summary = try XCTUnwrap(object["summary"] as? [String: Any])
        XCTAssertEqual(summary["identicalFiles"] as? Int, 1)
        XCTAssertEqual(summary["differentFiles"] as? Int, 1)
        XCTAssertEqual(summary["totalFiles"] as? Int, 2)

        let entriesJSON = try XCTUnwrap(object["entries"] as? [[String: Any]])
        XCTAssertEqual(entriesJSON.count, 2)
        let first = try XCTUnwrap(entriesJSON.first { $0["relativePath"] as? String == "sub/b,1.txt" })
        XCTAssertEqual(first["status"] as? String, "different")
        XCTAssertEqual(first["isDirectory"] as? Bool, false)
        XCTAssertEqual(first["leftSize"] as? Int, 1)
        XCTAssertEqual(first["rightSize"] as? Int, 2)
        XCTAssertEqual(first["leftModificationDate"] as? String, "2023-11-14T22:13:20Z")
        XCTAssertEqual(first["rightModificationDate"] as? String, "2023-11-14T22:13:21Z")
        XCTAssertEqual(first["errorMessage"] as? String, "読み込みエラー")
        XCTAssertTrue(first["renameCandidate"] is NSNull)
    }

    func testJSONIsPrettyPrintedAndDeterministic() {
        let entries = [
            entry(path: "b.txt", status: .different),
            entry(path: "a.txt", status: .identical),
        ]
        let first = report(entries: entries).render(as: .json)
        let second = report(entries: entries).render(as: .json)
        XCTAssertEqual(first, second)
        XCTAssertTrue(first.contains("\n"))
    }

    func testJSONIncludesRenameCandidate() throws {
        let candidate = RenameCandidate(counterpartPath: "new.txt", isUniqueMatch: false, totalMatches: 3)
        let output = report(entries: [entry(path: "old.txt", status: .leftOnly, renameCandidate: candidate)])
            .render(as: .json)
        let object = try XCTUnwrap(
            JSONSerialization.jsonObject(with: try XCTUnwrap(output.data(using: .utf8))) as? [String: Any]
        )
        let entriesJSON = try XCTUnwrap(object["entries"] as? [[String: Any]])
        let rename = try XCTUnwrap(entriesJSON[0]["renameCandidate"] as? [String: Any])
        XCTAssertEqual(rename["counterpartPath"] as? String, "new.txt")
        XCTAssertEqual(rename["isUniqueMatch"] as? Bool, false)
        XCTAssertEqual(rename["totalMatches"] as? Int, 3)
    }

    func testJSONIncludesGitRefsWhenProvided() throws {
        let output = ComparisonReport(
            result: makeResult(),
            generatedAt: Date(timeIntervalSince1970: 1_700_000_000),
            leftGitRef: "main",
            rightGitRef: "HEAD"
        ).render(as: .json)
        let object = try XCTUnwrap(
            JSONSerialization.jsonObject(with: try XCTUnwrap(output.data(using: .utf8))) as? [String: Any]
        )
        let left = try XCTUnwrap(object["left"] as? [String: Any])
        let right = try XCTUnwrap(object["right"] as? [String: Any])
        XCTAssertEqual(left["gitRef"] as? String, "main")
        XCTAssertEqual(right["gitRef"] as? String, "HEAD")
    }

    // MARK: - CSV

    func testCSVRowCountAndHeader() {
        let entries = [
            entry(path: "a.txt", status: .identical),
            entry(path: "b.txt", status: .different),
        ]
        let output = report(entries: entries).render(as: .csv)
        let lines = output.components(separatedBy: "\n")
        XCTAssertEqual(lines.count, 13 + entries.count)
        XCTAssertTrue(lines[0].hasPrefix("項目,値"))
        XCTAssertEqual(
            lines[12],
            "相対パス,状態,種別,左サイズ,右サイズ,左更新日時,右更新日時,リネーム候補,エラー"
        )
    }

    func testCSVEscapesCommaQuoteAndNewline() {
        let entries = [
            entry(path: "a,\"b\".txt", status: .different, errorMessage: "bad, \"quote\"\nline"),
        ]
        let output = report(entries: entries).render(as: .csv)
        // カンマ・ダブルクォートを含む相対パスは引用され、中のクォートは2重化される
        XCTAssertTrue(output.contains("\"a,\"\"b\"\".txt\",相違,ファイル"))
        // エラー欄はカンマ・改行込みで引用される
        XCTAssertTrue(output.contains("\"bad, \"\"quote\"\"\nline\""))
    }

    func testCSVQuotesOnlyWhenNeeded() {
        let entries = [entry(path: "plain.txt", status: .identical)]
        let output = report(entries: entries).render(as: .csv)
        XCTAssertTrue(output.contains("plain.txt,一致,ファイル"))
    }

    // MARK: - Markdown

    func testMarkdownRowCountAndEscaping() {
        let entries = [
            entry(path: "a|b.txt", status: .different, errorMessage: "x|y"),
            entry(path: "plain.txt", status: .identical),
        ]
        let output = report(entries: entries).render(as: .markdown)
        let lines = output.components(separatedBy: "\n")
        XCTAssertTrue(output.hasPrefix("# DiffLoupe比較レポート"))
        XCTAssertTrue(output.contains("## サマリ"))
        XCTAssertTrue(output.contains("## 詳細"))
        // パイプはエスケープされる
        XCTAssertTrue(output.contains("a\\|b.txt"))
        XCTAssertTrue(output.contains("x\\|y"))
        // テーブル行数 = サマリ(1ヘッダ+1セパレータ+5件+合計=8) + 詳細(1ヘッダ+1セパレータ+entries)
        let tableRows = lines.filter { $0.hasPrefix("|") }
        XCTAssertEqual(tableRows.count, 10 + entries.count)
    }

    // MARK: - HTML

    func testHTMLEscapesSpecialCharacters() {
        let entries = [
            entry(path: "a<b>&\"c'.txt", status: .different, errorMessage: "& < > \" '"),
        ]
        let output = report(entries: entries).render(as: .html)
        XCTAssertTrue(output.contains("a&lt;b&gt;&amp;&quot;c&#39;.txt"))
        XCTAssertTrue(output.contains("&amp; &lt; &gt; &quot; &#39;"))
        // 未エスケープの生パスが残らない
        XCTAssertFalse(output.contains("a<b>&"))
    }

    func testHTMLStandaloneStructure() {
        let output = report(entries: [entry(path: "a.txt", status: .identical)]).render(as: .html)
        XCTAssertTrue(output.hasPrefix("<!DOCTYPE html>"))
        XCTAssertTrue(output.contains("<html"))
        XCTAssertTrue(output.contains("</html>"))
        XCTAssertTrue(output.contains("<meta charset=\"utf-8\">"))
        XCTAssertTrue(output.contains("<table>"))
        XCTAssertTrue(output.contains("相対パス"))
    }

    func testHTMLRowClassForStatus() {
        let entries = [
            entry(path: "a.txt", status: .identical),
            entry(path: "b.txt", status: .error),
            entry(path: "c.txt", status: .leftOnly),
        ]
        let output = report(entries: entries).render(as: .html)
        XCTAssertTrue(output.contains("<tr class=\"identical\">"))
        XCTAssertTrue(output.contains("<tr class=\"error\">"))
        XCTAssertTrue(output.contains("<tr class=\"leftonly\">"))
    }

    func testHTMLMetaIncludesSidesAndRefs() {
        let output = ComparisonReport(
            result: makeResult(),
            generatedAt: Date(timeIntervalSince1970: 1_700_000_000),
            leftGitRef: "main",
            rightGitRef: nil
        ).render(as: .html)
        XCTAssertTrue(output.contains("/tmp/left @ main"))
        XCTAssertTrue(output.contains("/tmp/right"))
        XCTAssertTrue(output.contains("2023-11-14T22:13:20Z"))
    }

    // MARK: - 共通

    func testGeneratedAtIsLocaleIndependentISO8601() {
        let output = report().render(as: .json)
        XCTAssertTrue(output.contains("2023-11-14T22:13:20Z"))
    }

    func testReportFormatsCoverAllCases() {
        XCTAssertEqual(ReportFormat.allCases.count, 4)
        XCTAssertEqual(ReportFormat.html.fileExtension, "html")
        XCTAssertEqual(ReportFormat.csv.fileExtension, "csv")
        XCTAssertEqual(ReportFormat.json.fileExtension, "json")
        XCTAssertEqual(ReportFormat.markdown.fileExtension, "md")
    }
}
