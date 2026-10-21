// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: 比較結果をHTML/CSV/JSON/Markdown形式へエクスポートする。
// 実装は Rust コア(diffloupe-ffi)の `diffloupeReportRender` への薄いファサード。
// generatedAt のフォーマットは「ms四捨五入(round_ties_even) → 秒を floor」で
// 従来の Swift 実装と同一になるよう変換して Rust へ渡す。

import Foundation
import UniformTypeIdentifiers

/// レポート出力形式。ファイル拡張子とUTTypeを型で管理する。
public enum ReportFormat: String, CaseIterable, Identifiable {
    case html
    case csv
    case json
    case markdown

    public var id: String { rawValue }

    public var fileExtension: String {
        switch self {
        case .html: return "html"
        case .csv: return "csv"
        case .json: return "json"
        case .markdown: return "md"
        }
    }

    var title: String {
        switch self {
        case .html: return "HTML"
        case .csv: return "CSV"
        case .json: return "JSON"
        case .markdown: return "Markdown"
        }
    }

    public var utType: UTType {
        switch self {
        case .html: return .html
        case .csv: return .commaSeparatedText
        case .json: return .json
        case .markdown: return .plainText
        }
    }
}

/// 比較レポートの純粋なフォーマッタ。`render(as:)`で形式ごとの文字列を返す。
public struct ComparisonReport {
    let result: ComparisonResult
    var generatedAt: Date = Date()
    var leftGitRef: String?
    var rightGitRef: String?

    public static let formatName = "DiffLoupe比較レポート"

    public init(
        result: ComparisonResult,
        generatedAt: Date = Date(),
        leftGitRef: String? = nil,
        rightGitRef: String? = nil
    ) {
        self.result = result
        self.generatedAt = generatedAt
        self.leftGitRef = leftGitRef
        self.rightGitRef = rightGitRef
    }

    public func render(as format: ReportFormat) -> String {
        // 従来実装と同一の丸め: ミリ秒単位で round(.toNearestOrEven)→秒を floor(秒以下のms切り捨て)
        let epochSeconds = floor((generatedAt.timeIntervalSince1970 * 1000).rounded(.toNearestOrEven) / 1000)
        return diffloupeReportRender(
            result: result.ffi,
            format: format.ffi,
            generatedAt: epochSeconds,
            leftGitRef: leftGitRef,
            rightGitRef: rightGitRef
        )
    }
}

// MARK: - FFI 変換

extension ReportFormat {
    var ffi: FfiReportFormat {
        switch self {
        case .html: return .html
        case .csv: return .csv
        case .json: return .json
        case .markdown: return .markdown
        }
    }
}
