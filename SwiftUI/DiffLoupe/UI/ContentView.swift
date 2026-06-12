// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: メイン画面。フォルダ選択バー・比較ツリー・差分ビュー・ステータスバーを構成する

import SwiftUI

struct ContentView: View {
    @EnvironmentObject private var model: AppModel

    var body: some View {
        VStack(spacing: 0) {
            FolderSelectionBar()
            Divider()
            HSplitView {
                TreePane()
                    .frame(minWidth: 320, idealWidth: 420)
                DiffDetailView(entry: model.selectedEntry)
                    .frame(minWidth: 400, maxWidth: .infinity, maxHeight: .infinity)
            }
            Divider()
            StatusBar()
        }
        .background(WindowFrameSaver(autosaveName: "DiffLoupeMainWindow"))
    }
}

/// 上部: 左右フォルダ選択と比較開始
private struct FolderSelectionBar: View {
    @EnvironmentObject private var model: AppModel

    var body: some View {
        HStack(spacing: 12) {
            FolderPicker(side: .left, url: model.leftFolder)
            Image(systemName: "arrow.left.arrow.right")
                .foregroundColor(.secondary)
            FolderPicker(side: .right, url: model.rightFolder)

            if model.phase.isRunning {
                Button(role: .cancel) {
                    model.cancelCompare()
                } label: {
                    Label("キャンセル", systemImage: "xmark.circle")
                }
            } else {
                Button {
                    model.startCompare()
                } label: {
                    Label("比較", systemImage: "play.fill")
                }
                .keyboardShortcut("r", modifiers: .command)
                .disabled(!model.canCompare)
                .buttonStyle(.borderedProminent)
            }
        }
        .padding(10)
    }
}

private struct FolderPicker: View {
    @EnvironmentObject private var model: AppModel
    let side: AppModel.Side
    let url: URL?

    var body: some View {
        HStack(spacing: 6) {
            Button {
                model.pickFolder(side: side)
            } label: {
                Label(side == .left ? "左フォルダ" : "右フォルダ", systemImage: "folder")
            }
            .disabled(model.phase.isRunning)

            Text(url?.path ?? "未選択")
                .lineLimit(1)
                .truncationMode(.middle)
                .foregroundColor(url == nil ? .secondary : .primary)
                .frame(maxWidth: .infinity, alignment: .leading)
                .help(url?.path ?? "")
        }
        .frame(maxWidth: .infinity)
    }
}

/// 左ペイン: フィルタトグルと比較結果ツリー
private struct TreePane: View {
    @EnvironmentObject private var model: AppModel

    var body: some View {
        VStack(spacing: 0) {
            HStack {
                Toggle("差分のみ表示", isOn: $model.showOnlyDifferences)
                    .toggleStyle(.checkbox)
                Spacer()
            }
            .padding(.horizontal, 10)
            .padding(.vertical, 6)
            Divider()
            DiffTreeView()
        }
    }
}

/// 下部: 進捗・サマリ・エラー表示
private struct StatusBar: View {
    @EnvironmentObject private var model: AppModel

    var body: some View {
        HStack(spacing: 12) {
            switch model.phase {
            case .idle:
                if let error = model.errorMessage {
                    Label(error, systemImage: "exclamationmark.triangle.fill")
                        .foregroundColor(.red)
                        .lineLimit(1)
                } else if let summary = model.result?.summary {
                    SummaryLabel(color: .green, text: "一致 \(summary.identicalFiles)")
                    SummaryLabel(color: .red, text: "相違 \(summary.differentFiles)")
                    SummaryLabel(color: .blue, text: "左のみ \(summary.leftOnlyFiles)")
                    SummaryLabel(color: .purple, text: "右のみ \(summary.rightOnlyFiles)")
                    if summary.errorFiles > 0 {
                        SummaryLabel(color: .orange, text: "エラー \(summary.errorFiles)")
                    }
                    Spacer()
                    Text("計 \(summary.totalFiles) ファイル")
                        .foregroundColor(.secondary)
                } else {
                    Text("左右のフォルダを選択して「比較」を押してください")
                        .foregroundColor(.secondary)
                }
            case .scanning(let found):
                ProgressView()
                    .controlSize(.small)
                Text("走査中… \(found) 件")
                    .foregroundColor(.secondary)
            case .comparing(let done, let total):
                ProgressView(value: Double(done), total: Double(max(total, 1)))
                    .frame(width: 160)
                Text("内容比較中… \(done)/\(total)")
                    .foregroundColor(.secondary)
            }
            if model.phase == .idle, model.result == nil, model.errorMessage == nil {
                Spacer()
            } else if model.phase.isRunning {
                Spacer()
            }
        }
        .font(.callout)
        .padding(.horizontal, 10)
        .padding(.vertical, 5)
    }
}

private struct SummaryLabel: View {
    let color: Color
    let text: String

    var body: some View {
        HStack(spacing: 4) {
            Circle().fill(color).frame(width: 8, height: 8)
            Text(text)
        }
    }
}
