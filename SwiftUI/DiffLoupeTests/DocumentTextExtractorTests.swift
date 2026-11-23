// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: DocumentTextExtractor(PDF/Word/RTF/表計算のテキスト抽出)のテスト。
// PDFKit・AppKitに依存するためXcode専用のテストターゲットに置く。
// rtfは文字列から生成して往復、pdfはCoreGraphics/CoreTextでテキスト入りPDFを生成して抽出する。

import XCTest
import AppKit
import CoreText
@testable import DiffLoupe

final class DocumentTextExtractorTests: XCTestCase {

    private var base: URL!

    override func setUpWithError() throws {
        base = FileManager.default.temporaryDirectory
            .appendingPathComponent("DiffLoupeDocTests-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: base, withIntermediateDirectories: true)
    }

    override func tearDownWithError() throws {
        try? FileManager.default.removeItem(at: base)
    }

    func testIsDocumentFileByExtensionCaseInsensitive() {
        for name in ["a.pdf", "a.PDF", "a.docx", "a.doc", "a.rtf", "a.odt", "a.xlsx", "a.XLSM", "a.xls", "a.ods"] {
            XCTAssertTrue(DocumentTextExtractor.isDocumentFile(URL(fileURLWithPath: "/tmp/\(name)")), name)
        }
        for name in ["a.txt", "a.png", "a.zip", "pdf", "a.html"] {
            XCTAssertFalse(DocumentTextExtractor.isDocumentFile(URL(fileURLWithPath: "/tmp/\(name)")), name)
        }
    }

    func testRTFRoundTrip() throws {
        let attributed = NSAttributedString(string: "こんにちは\nHello RTF\r\nthird")
        let data = try attributed.data(
            from: NSRange(location: 0, length: attributed.length),
            documentAttributes: [.documentType: NSAttributedString.DocumentType.rtf])
        let url = base.appendingPathComponent("sample.rtf")
        try data.write(to: url)
        let text = try DocumentTextExtractor.extractText(from: url).get()
        XCTAssertTrue(text.contains("こんにちは"), text)
        XCTAssertTrue(text.contains("Hello RTF"), text)
        XCTAssertFalse(text.contains("\r"), "改行はLFに正規化される")
    }

    func testPDFTextExtraction() throws {
        let url = base.appendingPathComponent("sample.pdf")
        try Self.writePDF(pages: ["First page text", "Second page text"], to: url)
        let text = try DocumentTextExtractor.extractText(from: url).get()
        XCTAssertTrue(text.contains("--- ページ 1 ---"), text)
        XCTAssertTrue(text.contains("--- ページ 2 ---"), text)
        XCTAssertTrue(text.contains("First page text"), text)
        XCTAssertTrue(text.contains("Second page text"), text)
    }

    func testPDFWithoutTextFails() throws {
        let url = base.appendingPathComponent("blank.pdf")
        try Self.writePDF(pages: [""], to: url)
        switch DocumentTextExtractor.extractText(from: url) {
        case .success(let text): XCTFail("テキストなしPDFは失敗するはず: \(text)")
        case .failure(let error): XCTAssertTrue(error.message.contains("テキストを含まない PDF"), error.message)
        }
    }

    func testBrokenDocumentFails() throws {
        let url = base.appendingPathComponent("broken.pdf")
        try Data("not a pdf".utf8).write(to: url)
        if case .success = DocumentTextExtractor.extractText(from: url) {
            XCTFail("壊れたPDFは失敗するはず")
        }
    }

    /// CoreGraphics/CoreTextで1ページ1文字列のPDFを書き出す。空文字列のページは何も描かない。
    private static func writePDF(pages: [String], to url: URL) throws {
        var box = CGRect(x: 0, y: 0, width: 612, height: 792)
        guard let context = CGContext(url as CFURL, mediaBox: &box, nil) else {
            throw NSError(domain: "DocumentTextExtractorTests", code: 1)
        }
        let font = CTFontCreateWithName("Helvetica" as CFString, 24, nil)
        for page in pages {
            context.beginPDFPage(nil)
            if !page.isEmpty {
                let attributed = NSAttributedString(string: page, attributes: [.font: font])
                let line = CTLineCreateWithAttributedString(attributed)
                context.textPosition = CGPoint(x: 72, y: 700)
                CTLineDraw(line, context)
            }
            context.endPDFPage()
        }
        context.closePDF()
    }
}
