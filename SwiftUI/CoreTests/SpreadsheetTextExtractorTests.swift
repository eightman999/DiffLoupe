// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: SpreadsheetTextExtractor(表計算テキスト抽出ファサード)のフォーカステスト。
// 最小のxlsxを/usr/bin/zipでテスト内に組み立て、「## シート名」見出しとタブ区切り行が出ることを検証する。

import XCTest
@testable import DiffLoupeCore

final class SpreadsheetTextExtractorTests: XCTestCase {

    private var base: URL!

    override func setUpWithError() throws {
        base = FileManager.default.temporaryDirectory
            .appendingPathComponent("DiffLoupeSheetTests-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: base, withIntermediateDirectories: true)
    }

    override func tearDownWithError() throws {
        try? FileManager.default.removeItem(at: base)
    }

    private func inline(_ ref: String, _ text: String) -> String {
        "<c r=\"\(ref)\" t=\"inlineStr\"><is><t>\(text)</t></is></c>"
    }

    /// シート(名前, sheetData内のXML)から最小のxlsxを作る。
    private func makeXlsx(sheets: [(name: String, rows: String)]) throws -> URL {
        let src = base.appendingPathComponent("xlsx-src")
        let fm = FileManager.default
        try fm.createDirectory(at: src.appendingPathComponent("_rels"), withIntermediateDirectories: true)
        try fm.createDirectory(at: src.appendingPathComponent("xl/_rels"), withIntermediateDirectories: true)
        try fm.createDirectory(at: src.appendingPathComponent("xl/worksheets"), withIntermediateDirectories: true)

        var overrides = "", wbSheets = "", rels = ""
        for (i, sheet) in sheets.enumerated() {
            let n = i + 1
            overrides += "<Override PartName=\"/xl/worksheets/sheet\(n).xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml\"/>"
            wbSheets += "<sheet name=\"\(sheet.name)\" sheetId=\"\(n)\" r:id=\"rId\(n)\"/>"
            rels += "<Relationship Id=\"rId\(n)\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet\" Target=\"worksheets/sheet\(n).xml\"/>"
            try Data("<?xml version=\"1.0\" encoding=\"UTF-8\"?><worksheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\"><sheetData>\(sheet.rows)</sheetData></worksheet>".utf8)
                .write(to: src.appendingPathComponent("xl/worksheets/sheet\(n).xml"))
        }
        let styleID = sheets.count + 1
        rels += "<Relationship Id=\"rId\(styleID)\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles\" Target=\"styles.xml\"/>"
        let files: [String: String] = [
            "[Content_Types].xml": "<?xml version=\"1.0\" encoding=\"UTF-8\"?><Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/><Default Extension=\"xml\" ContentType=\"application/xml\"/><Override PartName=\"/xl/workbook.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml\"/><Override PartName=\"/xl/styles.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml\"/>\(overrides)</Types>",
            "_rels/.rels": "<?xml version=\"1.0\" encoding=\"UTF-8\"?><Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" Target=\"xl/workbook.xml\"/></Relationships>",
            "xl/workbook.xml": "<?xml version=\"1.0\" encoding=\"UTF-8\"?><workbook xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\"><sheets>\(wbSheets)</sheets></workbook>",
            "xl/_rels/workbook.xml.rels": "<?xml version=\"1.0\" encoding=\"UTF-8\"?><Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\(rels)</Relationships>",
            "xl/styles.xml": "<?xml version=\"1.0\" encoding=\"UTF-8\"?><styleSheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\"><fonts count=\"1\"><font><sz val=\"11\"/><name val=\"Calibri\"/></font></fonts><fills count=\"2\"><fill><patternFill patternType=\"none\"/></fill><fill><patternFill patternType=\"gray125\"/></fill></fills><borders count=\"1\"><border><left/><right/><top/><bottom/><diagonal/></border></borders><cellStyleXfs count=\"1\"><xf numFmtId=\"0\" fontId=\"0\" fillId=\"0\" borderId=\"0\"/></cellStyleXfs><cellXfs count=\"1\"><xf numFmtId=\"0\" fontId=\"0\" fillId=\"0\" borderId=\"0\" xfId=\"0\"/></cellXfs></styleSheet>",
        ]
        for (path, body) in files {
            try Data(body.utf8).write(to: src.appendingPathComponent(path))
        }
        let out = base.appendingPathComponent("book.xlsx")
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/zip")
        process.currentDirectoryURL = src
        process.arguments = ["-q", "-r", "-X", out.path, "."]
        try process.run()
        process.waitUntilExit()
        XCTAssertEqual(process.terminationStatus, 0)
        return out
    }

    func testIsSpreadsheetByExtension() {
        for name in ["a.xlsx", "a.XLSM", "a.xls", "b.ODS"] {
            XCTAssertTrue(SpreadsheetTextExtractor.isSpreadsheet(url: URL(fileURLWithPath: "/tmp/\(name)")), name)
        }
        for name in ["a.csv", "a.docx", "a.pdf"] {
            XCTAssertFalse(SpreadsheetTextExtractor.isSpreadsheet(url: URL(fileURLWithPath: "/tmp/\(name)")), name)
        }
    }

    func testExtractsSheetHeadingsAndTabSeparatedRows() throws {
        let sheet1 = "<row r=\"1\">\(inline("A1", "名前"))\(inline("B1", "点数"))</row>"
            + "<row r=\"2\">\(inline("A2", "山田"))<c r=\"B2\"><v>42</v></c></row>"
        let sheet2 = "<row r=\"1\">\(inline("A1", "end"))</row>"
        let url = try makeXlsx(sheets: [("シート1", sheet1), ("Second", sheet2)])
        let text = try SpreadsheetTextExtractor.extractText(from: url)
        XCTAssertTrue(text.contains("## シート1"), text)
        XCTAssertTrue(text.contains("名前\t点数"), text)
        XCTAssertTrue(text.contains("山田\t42"), text)
        XCTAssertTrue(text.contains("## Second"), text)
    }

    func testBrokenFileThrowsExtractError() throws {
        let url = base.appendingPathComponent("broken.xlsx")
        try Data("not a spreadsheet".utf8).write(to: url)
        XCTAssertThrowsError(try SpreadsheetTextExtractor.extractText(from: url)) { error in
            XCTAssertTrue(error is SpreadsheetTextExtractor.ExtractError)
        }
    }
}
