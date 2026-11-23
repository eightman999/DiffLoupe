// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: 「テキストを貼り付けて比較」シート。ファイルを作らずに、左右のTextEditorへ貼り付けた
// 2つのテキストの行差分を表示する。入力変更から300ms待ってバックグラウンドで差分を計算し
// (MainActorを塞がない・古い計算は.task(id:)のキャンセルで破棄)、既存のDiffRowsViewで描画する。
// 比較オプション(改行・空白・大文字小文字の無視)は設定を反映し、構造化比較は扱わない。
// 入力テキストは@Stateのみで保持し、UserDefaults等へは保存しない。

import AppKit
import SwiftUI
#if canImport(DiffLoupeCore)
import DiffLoupeCore
#endif

public struct PasteCompareView: View {
    @EnvironmentObject private var model: AppModel
    @Environment(\.dismiss) private var dismiss

    private enum Result: Equatable {
        case empty
        case computing
        case tooLarge
        case diff(rows: [DiffRow], highlights: [Int: InlineHighlight], summary: DiffDisplay.RowSummary)
    }

    @State private var leftText = ""
    @State private var rightText = ""
    @State private var result: Result = .empty

    // DiffRowsViewが要求するバインディング(貼り付け比較では検索・ジャンプを使わない)
    @State private var expandedGroups: Set<Int> = []
    @State private var showAllContextRows = false
    @State private var scrollTarget: Int?

    public init() {}

    /// 入力と比較オプションが変わったときだけ再計算するためのキー
    private struct ComputeKey: Equatable {
        let left: String
        let right: String
        let comparison: ContentComparison
    }

    private var comparison: ContentComparison {
        var value = model.currentContentComparison
        value.structuredMode = .disabled
        return value
    }

    public var body: some View {
        VStack(spacing: 0) {
            header
            Divider()
            VSplitView {
                HStack(spacing: 0) {
                    editorPane(title: "左", text: $leftText)
                    Divider()
                    editorPane(title: "右", text: $rightText)
                }
                .frame(minHeight: 160)
                resultPane
                    .frame(minHeight: 200)
            }
        }
        .frame(minWidth: 900, minHeight: 600)
        .task(id: ComputeKey(left: leftText, right: rightText, comparison: comparison)) {
            await recompute()
        }
    }

    // MARK: - ヘッダ

    private var header: some View {
        HStack(spacing: 12) {
            Text("テキストを貼り付けて比較")
                .font(.headline)
            summaryText
                .font(.callout)
                .foregroundColor(.secondary)
            Spacer()
            Button {
                swap(&leftText, &rightText)
            } label: {
                Label("左右を入れ替え", systemImage: "arrow.left.arrow.right")
            }
            .help("左右のテキストを入れ替えます")
            Button("閉じる") { dismiss() }
                .keyboardShortcut(.cancelAction)
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 8)
    }

    @ViewBuilder
    private var summaryText: some View {
        switch result {
        case .empty:
            Text("左右にテキストを貼り付けると、差分を表示します")
        case .computing:
            Text("差分を計算中…")
        case .tooLarge:
            Text("行数が多すぎるため差分を表示できません")
        case .diff(_, _, let summary):
            Text(Self.summaryDescription(summary))
        }
    }

    /// 要約の文言(一致 / 相違と行数内訳)
    static func summaryDescription(_ summary: DiffDisplay.RowSummary) -> String {
        if summary.isIdentical { return "一致" }
        return "相違: 追加 \(summary.addedCount) 行 / 削除 \(summary.removedCount) 行 / 変更 \(summary.modifiedCount) 行"
    }

    // MARK: - 入力エディタ

    private func editorPane(title: String, text: Binding<String>) -> some View {
        VStack(spacing: 0) {
            HStack(spacing: 8) {
                Text(title).font(.subheadline).bold()
                Spacer()
                Button {
                    if let pasted = NSPasteboard.general.string(forType: .string) {
                        text.wrappedValue = pasted
                    }
                } label: {
                    Label("クリップボードから貼り付け", systemImage: "doc.on.clipboard")
                }
                Button {
                    text.wrappedValue = ""
                } label: {
                    Label("クリア", systemImage: "xmark.circle")
                }
                .disabled(text.wrappedValue.isEmpty)
            }
            .padding(.horizontal, 8)
            .padding(.vertical, 4)
            Divider()
            TextEditor(text: text)
                .font(.system(.callout, design: .monospaced))
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    // MARK: - 差分表示

    @ViewBuilder
    private var resultPane: some View {
        switch result {
        case .empty:
            placeholder("左右にテキストを貼り付けてください", systemImage: "doc.on.clipboard")
        case .computing:
            ProgressView("差分を計算中…")
                .frame(maxWidth: .infinity, maxHeight: .infinity)
        case .tooLarge:
            placeholder("行数が多すぎるため差分を表示できません", systemImage: "doc.badge.ellipsis")
        case .diff(let rows, let highlights, _):
            DiffRowsView(
                rows: rows,
                highlights: highlights,
                expandedGroups: $expandedGroups,
                showAllContextRows: $showAllContextRows,
                currentDifferenceID: nil,
                searchHitRowIDs: [],
                currentSearchHitID: nil,
                onExpandGroup: { expandedGroups.insert($0) },
                scrollTarget: $scrollTarget
            )
        }
    }

    private func placeholder(_ text: String, systemImage: String) -> some View {
        VStack(spacing: 8) {
            Image(systemName: systemImage)
                .font(.largeTitle)
                .foregroundColor(.secondary)
            Text(text)
                .foregroundColor(.secondary)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    // MARK: - 差分計算

    private func recompute() async {
        let left = leftText
        let right = rightText
        let options = comparison
        if left.isEmpty && right.isEmpty {
            result = .empty
            expandedGroups = []
            return
        }
        // デバウンス: 入力が続く間は.task(id:)のキャンセルでここのsleepが中断される
        do {
            try await Task.sleep(nanoseconds: 300_000_000)
        } catch {
            return
        }
        result = .computing
        let computed = await Self.compute(left: left, right: right, comparison: options)
        if !Task.isCancelled {
            expandedGroups = []
            result = computed
        }
    }

    private static func compute(left: String, right: String, comparison: ContentComparison) async -> Result {
        await Task.detached(priority: .userInitiated) { () -> Result in
            guard let rows = LineDiff.rows(leftText: left, rightText: right, contentComparison: comparison) else {
                return .tooLarge
            }
            let highlights = InlineDiff.highlightsForRows(rows, caseInsensitive: comparison.ignoreCase)
            return .diff(rows: rows, highlights: highlights, summary: DiffDisplay.summary(of: rows))
        }.value
    }
}
