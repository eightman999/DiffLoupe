// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: 3-wayテキスト差分(base/left/right)の読み取り専用ビュー。
// NSOpenPanelで3ファイルを選択し、3列テーブルと状態の色・競合数・truncated警告を表示する。
// マージ・保存・適用のボタンは持たない(読み取り専用)。

import SwiftUI
#if canImport(DiffLoupeCore)
import DiffLoupeCore
#endif

public struct ThreeWayComparisonView: View {
    @EnvironmentObject private var model: AppModel
    @Environment(\.dismiss) private var dismiss

    private enum LoadState {
        case idle
        case loading
        case loaded(ThreeWayDiff)
        case failed(String)
    }

    @State private var loadState: LoadState = .idle

    public init() {}

    public var body: some View {
        VStack(spacing: 0) {
            header
            Divider()
            content
        }
        .frame(minWidth: 900, minHeight: 560)
        .task(id: loadID) {
            await reload()
        }
    }

    // MARK: - ヘッダ

    private var loadID: String {
        let base = model.threeWayBaseURL?.path ?? ""
        let left = model.threeWayLeftURL?.path ?? ""
        let right = model.threeWayRightURL?.path ?? ""
        return "\(base)|\(left)|\(right)"
    }

    private var header: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack(spacing: 10) {
                Text("3-way差分（読み取り専用）")
                    .font(.headline)
                Spacer()
                if case .loaded(let diff) = loadState {
                    summaryText(diff)
                        .foregroundColor(.secondary)
                        .font(.callout)
                }
                Button("キャンセル") {
                    model.closeThreeWayComparison()
                    dismiss()
                }
                .keyboardShortcut(.cancelAction)
            }
            HStack(spacing: 16) {
                SideFilePicker(label: "ベース", url: model.threeWayBaseURL) {
                    model.pickThreeWayFile(side: .base)
                }
                SideFilePicker(label: "左", url: model.threeWayLeftURL) {
                    model.pickThreeWayFile(side: .left)
                }
                SideFilePicker(label: "右", url: model.threeWayRightURL) {
                    model.pickThreeWayFile(side: .right)
                }
            }
            if case .loaded(let diff) = loadState {
                if diff.truncated {
                    Label(
                        "上限(3側合計\(ThreeWayDiff.maxTotalLines)行)を超えたため表示を中断しました",
                        systemImage: "doc.badge.ellipsis"
                    )
                    .font(.caption)
                    .foregroundColor(.orange)
                } else {
                    HStack(spacing: 12) {
                        LegendLabel(color: .blue, text: "左のみ変更 \(diff.leftChangedCount)")
                        LegendLabel(color: .orange, text: "右のみ変更 \(diff.rightChangedCount)")
                        LegendLabel(color: .green, text: "両者同一変更 \(diff.bothChangedSameCount)")
                        LegendLabel(color: .red, text: "競合 \(diff.conflictCount)")
                    }
                    .font(.caption)
                }
            }
        }
        .padding(10)
    }

    private func summaryText(_ diff: ThreeWayDiff) -> some View {
        HStack(spacing: 12) {
            Text("行 \(diff.lines.count)")
            Text("競合 \(diff.conflictCount)")
                .foregroundColor(diff.conflictCount > 0 ? .red : .secondary)
        }
    }

    // MARK: - コンテンツ

    @ViewBuilder
    private var content: some View {
        let allSelected = model.threeWayBaseURL != nil
            && model.threeWayLeftURL != nil
            && model.threeWayRightURL != nil
        if !allSelected {
            placeholder("ベース・左・右の3つのファイルを選択してください")
        } else {
            switch loadState {
            case .idle, .loading:
                VStack(spacing: 10) {
                    ProgressView()
                    Text("3-way差分を計算中…")
                        .foregroundColor(.secondary)
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            case .failed(let message):
                placeholder(message)
            case .loaded(let diff):
                if diff.truncated {
                    placeholder("表示を中断しました（ファイル行数が上限を超えています）")
                } else {
                    diffTable(diff)
                }
            }
        }
    }

    private func placeholder(_ message: String) -> some View {
        VStack(spacing: 10) {
            Image(systemName: "arrow.3.trianglepath")
                .font(.system(size: 36))
                .foregroundColor(.secondary.opacity(0.6))
            Text(message)
                .foregroundColor(.secondary)
                .multilineTextAlignment(.center)
        }
        .padding()
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    private func diffTable(_ diff: ThreeWayDiff) -> some View {
        Table(diff.lines) {
            TableColumn("状態") { line in
                Text(statusText(line.status))
                    .foregroundColor(statusColor(line.status))
            }
            .width(min: 100, ideal: 120)
            TableColumn("ベース") { line in
                LineCell(text: line.base, number: line.baseNumber, status: line.status)
            }
            .width(min: 200, ideal: 320)
            TableColumn("左") { line in
                LineCell(text: line.left, number: line.leftNumber, status: line.status)
            }
            .width(min: 200, ideal: 320)
            TableColumn("右") { line in
                LineCell(text: line.right, number: line.rightNumber, status: line.status)
            }
            .width(min: 200, ideal: 320)
        }
    }

    private func statusText(_ status: ThreeWayDiffStatus) -> String {
        switch status {
        case .unchanged: return "同一"
        case .leftChanged: return "左のみ変更"
        case .rightChanged: return "右のみ変更"
        case .bothChangedSame: return "両者同一変更"
        case .conflict: return "競合"
        }
    }

    private func statusColor(_ status: ThreeWayDiffStatus) -> Color {
        switch status {
        case .unchanged: return .secondary
        case .leftChanged: return .blue
        case .rightChanged: return .orange
        case .bothChangedSame: return .green
        case .conflict: return .red
        }
    }

    private func reload() async {
        guard let baseURL = model.threeWayBaseURL,
              let leftURL = model.threeWayLeftURL,
              let rightURL = model.threeWayRightURL else {
            loadState = .idle
            return
        }
        loadState = .loading
        let contentComparison = model.currentContentComparison
        let result: Result<ThreeWayDiff, Error> = await Task.detached(priority: .userInitiated) {
            Result {
                try ThreeWayComparisonLoader.load(
                    baseURL: baseURL,
                    leftURL: leftURL,
                    rightURL: rightURL,
                    contentComparison: contentComparison
                )
            }
        }.value
        guard !Task.isCancelled else { return }
        switch result {
        case .success(let diff):
            loadState = .loaded(diff)
        case .failure(let error):
            loadState = .failed("読み込みエラー: \(error.localizedDescription)")
        }
    }
}

/// 3ファイルをTextFileLoaderで読み、3-way差分を計算する読み込み担当(読み取り専用・書き込みなし)。
/// バックグラウンド(Task.detached)から呼ばれるため、MainActor非依存の別型にする。
private enum ThreeWayComparisonLoader {
    static func load(
        baseURL: URL,
        leftURL: URL,
        rightURL: URL,
        contentComparison: ContentComparison
    ) throws -> ThreeWayDiff {
        let base = try loadText(url: baseURL, label: "ベース")
        let left = try loadText(url: leftURL, label: "左")
        let right = try loadText(url: rightURL, label: "右")
        return ThreeWayDiff.make(
            base: base,
            left: left,
            right: right,
            contentComparison: contentComparison
        )
    }

    private static func loadText(url: URL, label: String) throws -> String {
        switch try TextFileLoader.load(url: url) {
        case .text(let text, _):
            return text
        case .binary:
            throw ThreeWayLoadError.binary(label: label, url: url)
        case .tooLarge:
            throw ThreeWayLoadError.tooLarge(label: label)
        }
    }
}

/// 3-way差分用の読み込みエラー(読み取り専用のためエラー表示のみ)
enum ThreeWayLoadError: LocalizedError {
    case binary(label: String, url: URL)
    case tooLarge(label: String)

    var errorDescription: String? {
        switch self {
        case .binary(let label, let url):
            return "\(label)「\(url.lastPathComponent)」はテキストとして読み込めません"
        case .tooLarge(let label):
            return "\(label)ファイルが大きすぎて読み込めません"
        }
    }
}

/// 1列のセル(行番号 + 原文)。対応する行が無い列は空表示。競合行は薄い赤の背景。
private struct LineCell: View {
    let text: String?
    let number: Int?
    let status: ThreeWayDiffStatus

    var body: some View {
        HStack(spacing: 6) {
            if let number {
                Text("\(number)")
                    .font(.system(size: 10))
                    .foregroundColor(.secondary)
                    .frame(width: 36, alignment: .trailing)
            } else {
                Spacer().frame(width: 36)
            }
            Text(text ?? "")
                .font(.system(size: 11, design: .monospaced))
                .foregroundColor(text == nil ? .secondary.opacity(0.5) : .primary)
                .lineLimit(1)
                .truncationMode(.tail)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(.vertical, 1)
        .background(status == .conflict ? Color.red.opacity(0.10) : Color.clear)
    }
}

/// ファイル選択ボタン。選択パスを表示する。
private struct SideFilePicker: View {
    let label: String
    let url: URL?
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            HStack(spacing: 6) {
                Text(label)
                    .fontWeight(.semibold)
                Text(url?.path ?? "未選択")
                    .foregroundColor(url == nil ? .secondary : .primary)
                    .lineLimit(1)
                    .truncationMode(.middle)
            }
        }
        .buttonStyle(.borderless)
        .help(url?.path ?? "")
    }
}

/// 状態の凡例(色 + ラベル)
private struct LegendLabel: View {
    let color: Color
    let text: String

    var body: some View {
        HStack(spacing: 4) {
            Circle().fill(color).frame(width: 7, height: 7)
            Text(text)
        }
    }
}
