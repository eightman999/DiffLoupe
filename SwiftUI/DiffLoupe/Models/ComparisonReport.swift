// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: 比較結果をJSON/CSV/Markdown/スタンドアロンHTMLへ整形する純粋なフォーマッタ。
// ファイルI/O・UIに依存せず文字列を返す(テスト可能)。日時はロケール非依存のISO8601(UTC)で統一する。

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

    var utType: UTType {
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
        switch format {
        case .json: return renderJSON()
        case .csv: return renderCSV()
        case .markdown: return renderMarkdown()
        case .html: return renderHTML()
        }
    }

    // MARK: - 共通ヘルパ

    /// 日時をISO8601(UTC・ロケール非依存)の文字列にする。nilは空文字で表す。
    private static func isoString(_ date: Date?) -> String {
        guard let date else { return "" }
        return isoDateFormatter.string(from: date)
    }

    /// ロケール・タイムゾーンに依存しないISO8601フォーマッタ(秒精度・Z指定)
    private static let isoDateFormatter: ISO8601DateFormatter = {
        let formatter = ISO8601DateFormatter()
        formatter.timeZone = TimeZone(identifier: "UTC")
        return formatter
    }()

    private static func sideMeta(_ path: String, ref: String?) -> String {
        if let ref { return "\(path) @ \(ref)" }
        return path
    }

    private static func sizeText(_ size: Int64) -> String {
        size >= 0 ? String(size) : "-"
    }

    private static func kindText(_ isDirectory: Bool) -> String {
        isDirectory ? "ディレクトリ" : "ファイル"
    }

    private static func statusLabel(_ status: DiffStatus) -> String {
        switch status {
        case .identical: return "一致"
        case .different: return "相違"
        case .leftOnly: return "左のみ"
        case .rightOnly: return "右のみ"
        case .error: return "エラー"
        }
    }

    /// リネーム候補の表示文字列。1対1確定は相手パスのみ、複数候補時は候補件数を付与する。
    private static func renameCandidateText(_ candidate: RenameCandidate?) -> String {
        guard let candidate else { return "" }
        if candidate.isUniqueMatch {
            return candidate.counterpartPath
        }
        return "\(candidate.counterpartPath)（候補\(candidate.totalMatches)件）"
    }

    private static let entryHeader = [
        "相対パス", "状態", "種別", "左サイズ", "右サイズ",
        "左更新日時", "右更新日時", "リネーム候補", "エラー",
    ]

    private var summaryRows: [(label: String, count: String)] {
        [
            ("一致", String(result.summary.identicalFiles)),
            ("相違", String(result.summary.differentFiles)),
            ("左のみ", String(result.summary.leftOnlyFiles)),
            ("右のみ", String(result.summary.rightOnlyFiles)),
            ("エラー", String(result.summary.errorFiles)),
        ]
    }

    private func entryRow(_ entry: ComparedEntry) -> [String] {
        [
            entry.relativePath,
            Self.statusLabel(entry.status),
            Self.kindText(entry.isDirectory),
            Self.sizeText(entry.leftSize),
            Self.sizeText(entry.rightSize),
            Self.isoString(entry.leftModificationDate),
            Self.isoString(entry.rightModificationDate),
            Self.renameCandidateText(entry.renameCandidate),
            entry.errorMessage ?? "",
        ]
    }

    // MARK: - JSON(prettyPrinted + sortedKeysで安定した順序)

    private func renderJSON() -> String {
        let root: [String: Any] = [
            "format": "DiffLoupeComparisonReport",
            "version": 1,
            "generatedAt": Self.isoString(generatedAt),
            "left": [
                "path": result.leftRoot.path,
                "gitRef": leftGitRef ?? NSNull(),
            ] as [String: Any],
            "right": [
                "path": result.rightRoot.path,
                "gitRef": rightGitRef ?? NSNull(),
            ] as [String: Any],
            "summary": [
                "identicalFiles": result.summary.identicalFiles,
                "differentFiles": result.summary.differentFiles,
                "leftOnlyFiles": result.summary.leftOnlyFiles,
                "rightOnlyFiles": result.summary.rightOnlyFiles,
                "errorFiles": result.summary.errorFiles,
                "totalFiles": result.summary.totalFiles,
            ],
            "entries": result.entries.map { entry in
                [
                    "relativePath": entry.relativePath,
                    "status": entry.status.rawValue,
                    "isDirectory": entry.isDirectory,
                    "leftSize": entry.leftSize,
                    "rightSize": entry.rightSize,
                    "leftModificationDate": entry.leftModificationDate.map { Self.isoString($0) } ?? NSNull(),
                    "rightModificationDate": entry.rightModificationDate.map { Self.isoString($0) } ?? NSNull(),
                    "renameCandidate": Self.renameCandidateJSON(entry.renameCandidate) ?? NSNull(),
                    "errorMessage": entry.errorMessage ?? NSNull(),
                ] as [String: Any]
            },
        ]
        let data = try? JSONSerialization.data(
            withJSONObject: root,
            options: [.prettyPrinted, .sortedKeys]
        )
        return String(data: data ?? Data(), encoding: .utf8) ?? ""
    }

    private static func renameCandidateJSON(_ candidate: RenameCandidate?) -> [String: Any]? {
        guard let candidate else { return nil }
        return [
            "counterpartPath": candidate.counterpartPath,
            "isUniqueMatch": candidate.isUniqueMatch,
            "totalMatches": candidate.totalMatches,
        ]
    }

    // MARK: - CSV(RFC4180相当の引用)

    private func renderCSV() -> String {
        var lines: [String] = []
        lines.append(Self.csvRow(["項目", "値"]))
        lines.append(Self.csvRow(["レポート種別", Self.formatName]))
        lines.append(Self.csvRow(["左", Self.sideMeta(result.leftRoot.path, ref: leftGitRef)]))
        lines.append(Self.csvRow(["右", Self.sideMeta(result.rightRoot.path, ref: rightGitRef)]))
        lines.append(Self.csvRow(["日時", Self.isoString(generatedAt)]))
        for (label, count) in summaryRows {
            lines.append(Self.csvRow([label, count]))
        }
        lines.append(Self.csvRow(["合計", String(result.summary.totalFiles)]))
        lines.append("")
        lines.append(Self.csvRow(Self.entryHeader))
        for entry in result.entries {
            lines.append(Self.csvRow(entryRow(entry)))
        }
        return lines.joined(separator: "\n")
    }

    /// RFC4180相当の引用。カンマ・ダブルクォート・改行を含むフィールドは
    /// ダブルクォートで囲み、中のダブルクォートは2つに重ねる。
    private static func csvRow(_ fields: [String]) -> String {
        fields.map(csvField).joined(separator: ",")
    }

    private static func csvField(_ value: String) -> String {
        let needsQuoting = value.contains(",") || value.contains("\"")
            || value.contains("\n") || value.contains("\r")
        guard needsQuoting else { return value }
        return "\"" + value.replacingOccurrences(of: "\"", with: "\"\"") + "\""
    }

    // MARK: - Markdown

    private func renderMarkdown() -> String {
        var lines: [String] = []
        lines.append("# \(Self.formatName)")
        lines.append("")
        lines.append("- 左: \(Self.mdEscape(Self.sideMeta(result.leftRoot.path, ref: leftGitRef)))")
        lines.append("- 右: \(Self.mdEscape(Self.sideMeta(result.rightRoot.path, ref: rightGitRef)))")
        lines.append("- 日時: \(Self.isoString(generatedAt))")
        lines.append("")
        lines.append("## サマリ")
        lines.append("")
        lines.append(Self.mdRow(["項目", "件数"]))
        lines.append(Self.mdSeparator(2))
        for (label, count) in summaryRows {
            lines.append(Self.mdRow([label, count]))
        }
        lines.append(Self.mdRow(["合計", String(result.summary.totalFiles)]))
        lines.append("")
        lines.append("## 詳細")
        lines.append("")
        lines.append(Self.mdRow(Self.entryHeader))
        lines.append(Self.mdSeparator(Self.entryHeader.count))
        for entry in result.entries {
            lines.append(Self.mdRow(entryRow(entry).map(Self.mdEscape)))
        }
        return lines.joined(separator: "\n")
    }

    /// Markdownテーブル用エスケープ。パイプを`\|`に、改行は除去/空白化する。
    private static func mdEscape(_ text: String) -> String {
        text.replacingOccurrences(of: "|", with: "\\|")
            .replacingOccurrences(of: "\r", with: "")
            .replacingOccurrences(of: "\n", with: " ")
    }

    private static func mdRow(_ fields: [String]) -> String {
        "| " + fields.joined(separator: " | ") + " |"
    }

    private static func mdSeparator(_ columnCount: Int) -> String {
        "| " + Array(repeating: "---", count: columnCount).joined(separator: " | ") + " |"
    }

    // MARK: - スタンドアロンHTML

    private func renderHTML() -> String {
        let summaryRowsHTML = summaryRows.map { label, count in
            "<tr><td>\(Self.htmlEscape(label))</td><td class=\"num\">\(count)</td></tr>"
        }.joined(separator: "\n")
        let entriesHTML = result.entries.map { entry in
            let cells = entryRow(entry)
                .map(Self.htmlEscape)
                .map { "<td>\($0)</td>" }
                .joined()
            return "<tr class=\"\(Self.statusRowClass(entry.status))\">\(cells)</tr>"
        }.joined(separator: "\n")
        let headerCells = Self.entryHeader.map(Self.htmlEscape).map { "<th>\($0)</th>" }.joined()
        let leftMeta = Self.htmlEscape(Self.sideMeta(result.leftRoot.path, ref: leftGitRef))
        let rightMeta = Self.htmlEscape(Self.sideMeta(result.rightRoot.path, ref: rightGitRef))

        return """
        <!DOCTYPE html>
        <html lang="ja">
        <head>
        <meta charset="utf-8">
        <meta name="viewport" content="width=device-width, initial-scale=1">
        <title>\(Self.htmlEscape(Self.formatName))</title>
        <style>
        body { font-family: -apple-system, BlinkMacSystemFont, sans-serif; margin: 24px; color: #1a1a1a; }
        h1 { font-size: 1.3rem; margin: 0 0 8px; }
        h2 { font-size: 1.1rem; margin: 24px 0 8px; }
        .meta { color: #555; font-size: 0.9rem; margin: 0 0 16px; }
        table { border-collapse: collapse; width: 100%; font-size: 0.85rem; }
        th, td { border: 1px solid #ccc; padding: 4px 8px; text-align: left; vertical-align: top; }
        th { background: #f2f2f2; }
        td.num { text-align: right; white-space: nowrap; }
        tr.identical { color: #666; }
        tr.leftonly td:first-child, tr.rightonly td:first-child { background: #fff8e6; }
        tr.error td { color: #b00020; }
        </style>
        </head>
        <body>
        <h1>\(Self.htmlEscape(Self.formatName))</h1>
        <p class="meta">左: \(leftMeta)<br>右: \(rightMeta)<br>日時: \(Self.isoString(generatedAt))</p>
        <h2>サマリ</h2>
        <table>
        <tr><th>項目</th><th>件数</th></tr>
        \(summaryRowsHTML)
        <tr><td>合計</td><td class="num">\(result.summary.totalFiles)</td></tr>
        </table>
        <h2>詳細</h2>
        <table>
        <thead><tr>\(headerCells)</tr></thead>
        <tbody>
        \(entriesHTML)
        </tbody>
        </table>
        </body>
        </html>
        """
    }

    private static func statusRowClass(_ status: DiffStatus) -> String {
        switch status {
        case .identical: return "identical"
        case .different: return "different"
        case .leftOnly: return "leftonly"
        case .rightOnly: return "rightonly"
        case .error: return "error"
        }
    }

    private static func htmlEscape(_ text: String) -> String {
        text.replacingOccurrences(of: "&", with: "&amp;")
            .replacingOccurrences(of: "<", with: "&lt;")
            .replacingOccurrences(of: ">", with: "&gt;")
            .replacingOccurrences(of: "\"", with: "&quot;")
            .replacingOccurrences(of: "'", with: "&#39;")
    }
}
