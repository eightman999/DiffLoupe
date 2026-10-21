// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: 比較結果をツリー表示する。ステータスごとに色分けし、選択で差分ビューに連動する

import SwiftUI
#if canImport(DiffLoupeCore)
import DiffLoupeCore
#endif

struct DiffTreeView: View {
    @EnvironmentObject private var model: AppModel

    var body: some View {
        let nodes = model.displayNodes
        if nodes.isEmpty {
            VStack {
                Spacer()
                Text(emptyMessage)
                    .foregroundColor(.secondary)
                    .multilineTextAlignment(.center)
                Spacer()
            }
            .frame(maxWidth: .infinity)
        } else {
            List(selection: $model.selection) {
                OutlineGroup(nodes, children: \.children) { node in
                    DiffNodeRow(node: node)
                        .tag(node.id)
                        .contextMenu {
                            entryContextMenu(for: node)
                        }
                }
            }
            .listStyle(.inset)
        }
    }

    @ViewBuilder
    private func entryContextMenu(for node: DiffNode) -> some View {
        if let entry = model.result?.entriesByPath[node.id] {
            if entry.leftURL != nil {
                Menu("左側") {
                    Button {
                        _ = model.openEntry(entry, side: .left)
                    } label: {
                        Label("開く", systemImage: "arrow.up.forward.app")
                    }
                    Button {
                        _ = model.revealEntryInFinder(entry, side: .left)
                    } label: {
                        Label("Finderに表示", systemImage: "folder")
                    }
                    Button {
                        _ = model.copyEntryPath(entry, side: .left)
                    } label: {
                        Label("パスをコピー", systemImage: "doc.on.doc")
                    }
                }
            }
            if entry.rightURL != nil {
                Menu("右側") {
                    Button {
                        _ = model.openEntry(entry, side: .right)
                    } label: {
                        Label("開く", systemImage: "arrow.up.forward.app")
                    }
                    Button {
                        _ = model.revealEntryInFinder(entry, side: .right)
                    } label: {
                        Label("Finderに表示", systemImage: "folder")
                    }
                    Button {
                        _ = model.copyEntryPath(entry, side: .right)
                    } label: {
                        Label("パスをコピー", systemImage: "doc.on.doc")
                    }
                }
            }
        }
    }

    private var emptyMessage: String {
        if model.result == nil {
            return "比較結果がここに表示されます"
        }
        let hasSearch = !model.resultSearchText.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
        if !hasSearch, model.resultFilter == .differences {
            return "差分はありません"
        }
        if hasSearch || model.resultFilter != .all {
            return "絞り込み条件に一致する項目はありません"
        }
        return "対象ファイルがありません"
    }
}

struct DiffNodeRow: View {
    let node: DiffNode

    var body: some View {
        HStack(spacing: 6) {
            Image(systemName: node.isDirectory ? "folder.fill" : "doc")
                .foregroundColor(node.isDirectory ? Color(nsColor: .systemBlue).opacity(0.7) : .secondary)
            Text(node.name)
                .lineLimit(1)
            Spacer()
            StatusBadge(status: node.status, isDirectory: node.isDirectory)
        }
    }
}

struct StatusBadge: View {
    let status: DiffStatus
    let isDirectory: Bool

    var body: some View {
        Text(label)
            .font(.caption2.weight(.medium))
            .padding(.horizontal, 6)
            .padding(.vertical, 1)
            .background(color.opacity(0.18))
            .foregroundColor(color)
            .clipShape(Capsule())
    }

    private var label: String {
        switch status {
        case .identical: return "一致"
        case .different: return isDirectory ? "差分あり" : "相違"
        case .leftOnly: return "左のみ"
        case .rightOnly: return "右のみ"
        case .error: return "エラー"
        }
    }

    private var color: Color {
        switch status {
        case .identical: return .green
        case .different: return .red
        case .leftOnly: return .blue
        case .rightOnly: return .purple
        case .error: return .orange
        }
    }
}
