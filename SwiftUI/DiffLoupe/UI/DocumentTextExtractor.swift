// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: PDF / Word / RTF / ODT / 表計算ファイルから比較用のテキストを抽出する(読み取り専用)。
// PDFKit・AppKit に依存するため Core ではなく UI 側に置く(ImageDiffEngine と同じ扱い)。
// 書式・画像・レイアウトは対象外で、抽出したテキストの行差分だけを見せる。
// 抽出は呼び出しスレッドで同期実行する(HTML は WebKit のメインスレッド制約があるため扱わない)。
// 表計算は Rust コアのファサード(SpreadsheetTextExtractor)へ委譲する。

import Foundation
import AppKit
import PDFKit
#if canImport(DiffLoupeCore)
import DiffLoupeCore
#endif

/// 文書テキスト抽出の失敗。`message` は日本語でそのまま表示できる。
public struct DocumentExtractError: LocalizedError, Sendable, Equatable {
    public let message: String
    public init(_ message: String) { self.message = message }
    public var errorDescription: String? { message }
}

public enum DocumentTextExtractor {

    /// 抽出対象とする入力ファイルの上限(64 MiB)
    public static let maxFileBytes: Int64 = 64 * 1024 * 1024

    private static let spreadsheetExtensions: Set<String> = ["xlsx", "xlsm", "xls", "ods"]
    private static let wordProcessorExtensions: Set<String> = ["docx", "doc", "rtf", "odt"]

    /// 拡張子(大文字小文字無視)から、テキスト抽出で比較する文書ファイルか判定する。
    public static func isDocumentFile(_ url: URL) -> Bool {
        let ext = url.pathExtension.lowercased()
        return ext == "pdf"
            || wordProcessorExtensions.contains(ext)
            || spreadsheetExtensions.contains(ext)
    }

    /// 文書からテキストを抽出する。改行は LF に正規化する。
    public static func extractText(from url: URL) -> Result<String, DocumentExtractError> {
        let size = (try? url.resourceValues(forKeys: [.fileSizeKey]))?.fileSize ?? 0
        if Int64(size) > maxFileBytes {
            return .failure(DocumentExtractError("ファイルが大きすぎます(上限64MiB)"))
        }
        let ext = url.pathExtension.lowercased()
        let raw: Result<String, DocumentExtractError>
        if ext == "pdf" {
            raw = extractPDF(url)
        } else if spreadsheetExtensions.contains(ext) {
            do {
                raw = .success(try SpreadsheetTextExtractor.extractText(from: url))
            } catch {
                raw = .failure(DocumentExtractError(error.localizedDescription))
            }
        } else if wordProcessorExtensions.contains(ext) {
            raw = extractWordProcessor(url, ext: ext)
        } else {
            raw = .failure(DocumentExtractError("対応していない文書形式です"))
        }
        return raw.map(normalizeLineEndings)
    }

    // MARK: - PDF

    private static func extractPDF(_ url: URL) -> Result<String, DocumentExtractError> {
        guard let document = PDFDocument(url: url) else {
            return .failure(DocumentExtractError("PDFを開けませんでした"))
        }
        if document.isLocked {
            return .failure(DocumentExtractError("パスワード付き PDF には対応していません"))
        }
        var parts: [String] = []
        var hasText = false
        for index in 0..<document.pageCount {
            let text = document.page(at: index)?.string ?? ""
            if !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty { hasText = true }
            parts.append("--- ページ \(index + 1) ---\n" + text)
        }
        guard hasText else {
            return .failure(DocumentExtractError("テキストを含まない PDF です(スキャン画像の可能性)"))
        }
        return .success(parts.joined(separator: "\n"))
    }

    // MARK: - Word / RTF / ODT

    private static func extractWordProcessor(_ url: URL, ext: String) -> Result<String, DocumentExtractError> {
        let type: NSAttributedString.DocumentType
        switch ext {
        case "docx": type = .officeOpenXML
        case "doc": type = .docFormat
        case "rtf": type = .rtf
        default: type = .openDocument
        }
        do {
            let attributed = try NSAttributedString(
                url: url,
                options: [.documentType: type],
                documentAttributes: nil
            )
            // 画像などの添付ファイル位置を示す U+FFFC は比較対象にしない
            let text = attributed.string.replacingOccurrences(of: "\u{FFFC}", with: "")
            return .success(text)
        } catch {
            return .failure(DocumentExtractError("文書を読み込めませんでした: \(error.localizedDescription)"))
        }
    }

    private static func normalizeLineEndings(_ text: String) -> String {
        text.replacingOccurrences(of: "\r\n", with: "\n")
            .replacingOccurrences(of: "\r", with: "\n")
    }
}
