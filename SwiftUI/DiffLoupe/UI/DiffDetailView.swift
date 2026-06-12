// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: 選択ファイルの横並び行差分ビュー。差分計算はバックグラウンドで行いキャンセルに追随する

import SwiftUI

struct DiffDetailView: View {
    let entry: ComparedEntry?

    private enum DetailState: Sendable {
        case empty
        case loading
        case message(String, systemImage: String)
        case diff([DiffRow])
    }

    @State private var state: DetailState = .empty

    var body: some View {
        VStack(spacing: 0) {
            header
            Divider()
            content
        }
        .task(id: entry?.relativePath) {
            await load()
        }
    }

    @ViewBuilder
    private var header: some View {
        HStack {
            if let entry {
                Image(systemName: entry.isDirectory ? "folder.fill" : "doc.text")
                    .foregroundColor(.secondary)
                Text(entry.relativePath)
                    .font(.system(.callout, design: .monospaced))
                    .lineLimit(1)
                    .truncationMode(.middle)
                Spacer()
                StatusBadge(status: entry.status, isDirectory: entry.isDirectory)
            } else {
                Text("ファイル差分")
                    .foregroundColor(.secondary)
                Spacer()
            }
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 6)
    }

    @ViewBuilder
    private var content: some View {
        switch state {
        case .empty:
            placeholder("ツリーからファイルを選択してください", systemImage: "cursorarrow.rays")
        case .loading:
            VStack(spacing: 10) {
                ProgressView()
                Text("差分を計算中…").foregroundColor(.secondary)
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
        case .message(let text, let systemImage):
            placeholder(text, systemImage: systemImage)
        case .diff(let rows):
            DiffRowsView(rows: rows)
        }
    }

    private func placeholder(_ text: String, systemImage: String) -> some View {
        VStack(spacing: 10) {
            Image(systemName: systemImage)
                .font(.system(size: 36))
                .foregroundColor(.secondary.opacity(0.6))
            Text(text)
                .foregroundColor(.secondary)
                .multilineTextAlignment(.center)
        }
        .padding()
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    // MARK: - 差分計算

    private func load() async {
        guard let entry else {
            state = .empty
            return
        }
        if entry.isDirectory {
            state = .message("フォルダです。配下のファイルを選択してください", systemImage: "folder")
            return
        }
        switch entry.status {
        case .identical:
            state = .message("内容は一致しています", systemImage: "checkmark.circle")
            return
        case .leftOnly:
            state = .message("左フォルダにのみ存在します", systemImage: "arrow.left.circle")
            return
        case .rightOnly:
            state = .message("右フォルダにのみ存在します", systemImage: "arrow.right.circle")
            return
        case .error:
            state = .message("読み込みエラー: \(entry.errorMessage ?? "不明")", systemImage: "exclamationmark.triangle")
            return
        case .different:
            break
        }
        guard let leftURL = entry.leftURL, let rightURL = entry.rightURL else {
            state = .message("比較対象がありません", systemImage: "questionmark.circle")
            return
        }

        state = .loading
        let computed = await Self.computeDiff(leftURL: leftURL, rightURL: rightURL)
        if !Task.isCancelled {
            state = computed
        }
    }

    private static func computeDiff(leftURL: URL, rightURL: URL) async -> DetailState {
        let task = Task.detached(priority: .userInitiated) { () -> DetailState in
            do {
                let left = try TextFileLoader.load(url: leftURL)
                let right = try TextFileLoader.load(url: rightURL)
                switch (left, right) {
                case (.binary, _), (_, .binary):
                    return .message("バイナリファイルのため差分表示できません", systemImage: "doc.zipper")
                case (.tooLarge, _), (_, .tooLarge):
                    return .message("ファイルが大きすぎるため差分表示できません(上限32MiB)", systemImage: "doc.badge.ellipsis")
                case (.text(let leftText), .text(let rightText)):
                    if let rows = LineDiff.rows(leftText: leftText, rightText: rightText) {
                        return .diff(rows)
                    }
                    return .message("行数が多すぎるため差分計算を省略しました(上限 計\(LineDiff.maxTotalLines)行)", systemImage: "doc.badge.ellipsis")
                }
            } catch {
                return .message("読み込みエラー: \(error.localizedDescription)", systemImage: "exclamationmark.triangle")
            }
        }
        return await task.value
    }
}

/// 整列済み行の横並び描画
private struct DiffRowsView: View {
    let rows: [DiffRow]

    var body: some View {
        ScrollView(.vertical) {
            LazyVStack(spacing: 0) {
                ForEach(rows) { row in
                    DiffRowView(row: row)
                }
            }
        }
        .background(Color(nsColor: .textBackgroundColor))
    }
}

private struct DiffRowView: View {
    let row: DiffRow

    var body: some View {
        HStack(alignment: .top, spacing: 0) {
            LineCell(number: row.leftNumber, text: row.leftText, background: leftBackground)
            Divider()
            LineCell(number: row.rightNumber, text: row.rightText, background: rightBackground)
        }
        .fixedSize(horizontal: false, vertical: true)
    }

    // 配色: 追加=緑系、削除=赤系、変更=黄系(SPEC.md §2.6)
    private var leftBackground: Color {
        switch row.kind {
        case .context: return .clear
        case .removed: return .red.opacity(0.16)
        case .added: return Color.secondary.opacity(0.06)
        case .modified: return .yellow.opacity(0.18)
        }
    }

    private var rightBackground: Color {
        switch row.kind {
        case .context: return .clear
        case .removed: return Color.secondary.opacity(0.06)
        case .added: return .green.opacity(0.16)
        case .modified: return .yellow.opacity(0.18)
        }
    }
}

private struct LineCell: View {
    let number: Int?
    let text: String?
    let background: Color

    var body: some View {
        HStack(alignment: .top, spacing: 6) {
            Text(number.map(String.init) ?? "")
                .font(.system(.caption, design: .monospaced))
                .foregroundColor(.secondary)
                .frame(width: 42, alignment: .trailing)
            Text(text ?? "")
                .font(.system(.callout, design: .monospaced))
                .textSelection(.enabled)
                .frame(maxWidth: .infinity, alignment: .leading)
        }
        .padding(.vertical, 1)
        .padding(.horizontal, 4)
        .background(background)
    }
}
