// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: 人間向けサマリの整形(純粋関数)。比較結果の集計と非一致ファイル一覧を
// プレーンテキストで返す。stdoutに出す(レポートをstdoutへ出す場合はstderrへ回る)。

import DiffLoupeCore

enum HumanSummary {

    static func render(result: ComparisonResult) -> String {
        let summary = result.summary
        var lines: [String] = []
        lines.append("DiffLoupe比較結果")
        lines.append("左: \(result.leftRoot.path)")
        lines.append("右: \(result.rightRoot.path)")
        lines.append(
            "一致: \(summary.identicalFiles)  相違: \(summary.differentFiles)  "
            + "左のみ: \(summary.leftOnlyFiles)  右のみ: \(summary.rightOnlyFiles)  "
            + "エラー: \(summary.errorFiles)  合計: \(summary.totalFiles)"
        )

        let differing = result.entries.filter { $0.status != .identical && !$0.isDirectory }
        if !differing.isEmpty {
            lines.append("")
            let order: [DiffStatus] = [.different, .leftOnly, .rightOnly, .error]
            for status in order {
                let group = differing.filter { $0.status == status }
                if group.isEmpty { continue }
                lines.append("\(label(for: status)):")
                lines.append(contentsOf: group.map { detailLine($0, status: status) })
            }
        }
        return lines.joined(separator: "\n")
    }

    private static func detailLine(_ entry: ComparedEntry, status: DiffStatus) -> String {
        if status == .error, let message = entry.errorMessage {
            return "  \(entry.relativePath)  [\(message)]"
        }
        return "  \(entry.relativePath)"
    }

    private static func label(for status: DiffStatus) -> String {
        switch status {
        case .identical: return "一致"
        case .different: return "相違"
        case .leftOnly: return "左のみ"
        case .rightOnly: return "右のみ"
        case .error: return "エラー"
        }
    }
}
