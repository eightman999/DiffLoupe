// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: 比較結果をツリー表示する。ステータスごとに色分けし、選択で差分ビューに連動する

import SwiftUI

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
                }
            }
            .listStyle(.inset)
        }
    }

    private var emptyMessage: String {
        if model.result == nil {
            return "比較結果がここに表示されます"
        }
        return model.showOnlyDifferences ? "差分はありません" : "対象ファイルがありません"
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
