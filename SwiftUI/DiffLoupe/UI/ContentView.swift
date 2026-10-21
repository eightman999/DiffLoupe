// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: メイン画面。フォルダ選択バー・比較ツリー・差分ビュー・ステータスバーを構成する

import SwiftUI
import UniformTypeIdentifiers
#if canImport(DiffLoupeCore)
import DiffLoupeCore
#endif

public struct ContentView: View {
    @EnvironmentObject private var model: AppModel

    public init() {}

    public var body: some View {
        VStack(spacing: 0) {
            FolderSelectionBar()
            Divider()
            HSplitView {
                TreePane()
                    .frame(minWidth: 320, idealWidth: 420)
                DiffDetailView(
                    entry: model.selectedEntry,
                    contentComparison: model.currentContentComparison
                )
                .frame(minWidth: 400, maxWidth: .infinity, maxHeight: .infinity)
            }
            Divider()
            StatusBar()
        }
        .background(WindowFrameSaver(autosaveName: "DiffLoupeMainWindow"))
        .sheet(isPresented: $model.isSyncPlanSheetPresented) {
            SyncPlanSheet()
        }
        .sheet(isPresented: $model.isThreeWayPresented) {
            // エディションが3-way表示を提供する場合はそれを、なければ従来の読み取り専用ビューを表示
            if let editionView = model.edition.threeWayView(model: model) {
                editionView
            } else {
                ThreeWayComparisonView()
            }
        }
        .alert(item: $model.entryLimitAlert) { alert in
            Alert(
                title: Text("Free版の比較上限に達しました"),
                message: Text(alert.message),
                primaryButton: .default(Text("設定を開く")) {
                    model.openSettings()
                },
                secondaryButton: .cancel(Text("閉じる"))
            )
        }
    }
}

/// 上部: 左右フォルダ選択・左右交換・履歴・比較開始
private struct FolderSelectionBar: View {
    @EnvironmentObject private var model: AppModel

    var body: some View {
        HStack(spacing: 12) {
            FolderPicker(side: .left, url: model.leftFolder)
            Button {
                model.swapSides()
            } label: {
                Image(systemName: "arrow.left.arrow.right")
            }
            .buttonStyle(.borderless)
            .help("左右を交換 (⌘⇧S)")
            .keyboardShortcut("s", modifiers: [.command, .shift])
            .disabled(model.phase.isRunning)
            FolderPicker(side: .right, url: model.rightFolder)
            HistoryMenu()

            Button {
                model.showThreeWayComparison()
            } label: {
                Label("3-way差分", systemImage: "arrow.3.trianglepath")
            }
            .help("3つのファイルを3-way差分で比較します")
            .disabled(model.phase.isRunning)

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

/// 比較履歴メニュー。選択で左右のフォルダ・git refを復元する。
/// 存在しないパスが含まれる履歴は選択不可にする。
private struct HistoryMenu: View {
    @EnvironmentObject private var model: AppModel

    var body: some View {
        Menu {
            if model.recentComparisons.isEmpty {
                Text("履歴はまだありません")
            } else {
                ForEach(model.recentComparisons) { item in
                    Button {
                        model.restoreComparison(item)
                    } label: {
                        Label(historyLabel(item), systemImage: "clock.arrow.circlepath")
                    }
                    .disabled(!historyItemExists(item))
                }
            }
        } label: {
            Label("履歴", systemImage: "clock.arrow.circlepath")
        }
        .help("比較履歴から復元")
        .disabled(model.phase.isRunning || model.recentComparisons.isEmpty)
    }

    private func historyItemExists(_ item: RecentComparison) -> Bool {
        AppModel.directoryExists(item.leftPath) && AppModel.directoryExists(item.rightPath)
    }

    private func historyLabel(_ item: RecentComparison) -> String {
        let leftName = (item.leftPath as NSString).lastPathComponent
        let rightName = (item.rightPath as NSString).lastPathComponent
        return "\(leftName)\(refSuffix(item.leftGitRef)) ⇄ \(rightName)\(refSuffix(item.rightGitRef))"
    }

    private func refSuffix(_ ref: String?) -> String {
        guard let ref else { return "" }
        return " @ \(ref)"
    }
}

private struct FolderPicker: View {
    @EnvironmentObject private var model: AppModel
    let side: AppModel.Side
    let url: URL?

    @State private var isDropTargeted = false

    private var gitRepo: GitRepository? {
        side == .left ? model.leftGitRepo : model.rightGitRepo
    }

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

            if gitRepo != nil {
                GitRefPicker(side: side)
            }
        }
        .frame(maxWidth: .infinity)
        .padding(.vertical, 3)
        .padding(.horizontal, 4)
        .background(
            RoundedRectangle(cornerRadius: 6)
                .fill(isDropTargeted ? Color.accentColor.opacity(0.15) : Color.clear)
        )
        .overlay(
            RoundedRectangle(cornerRadius: 6)
                .strokeBorder(isDropTargeted ? Color.accentColor : Color.clear, lineWidth: 1.5)
        )
        .disabled(model.phase.isRunning)
        .onDrop(of: [.fileURL], isTargeted: $isDropTargeted) { providers in
            handleDrop(providers)
        }
    }

    /// Finderドラッグ&ドロップ。フォルダ1件だけを受け付け、複数・ファイルは拒否する。
    /// 受け付けたURLはAppModelのacceptDroppedFolder経由でgit検出とrefリセットを通す。
    private func handleDrop(_ providers: [NSItemProvider]) -> Bool {
        guard !model.phase.isRunning else { return false }
        guard providers.count == 1, let provider = providers.first else { return false }
        guard provider.hasItemConformingToTypeIdentifier(UTType.folder.identifier) else { return false }
        let side = self.side
        let model = self.model
        _ = provider.loadObject(ofClass: URL.self) { url, _ in
            guard let url else { return }
            Task { @MainActor in
                model.acceptDroppedFolder(url: url, side: side)
            }
        }
        return true
    }
}

/// gitリポジトリ検出時のみ表示されるref選択UI。「作業ツリー」(デフォルト) + ブランチ/タグ一覧 + 任意ref手入力。
private struct GitRefPicker: View {
    @EnvironmentObject private var model: AppModel
    let side: AppModel.Side

    @State private var showCustomRefInput = false
    @State private var customRefText = ""

    private var repo: GitRepository? {
        side == .left ? model.leftGitRepo : model.rightGitRepo
    }

    private var currentRef: String? {
        side == .left ? model.leftGitRef : model.rightGitRef
    }

    var body: some View {
        Menu {
            Button {
                model.selectGitRef(nil, side: side)
            } label: {
                if currentRef == nil {
                    Label("作業ツリー", systemImage: "checkmark")
                } else {
                    Text("作業ツリー")
                }
            }

            Divider()

            ForEach(repo?.listRefs() ?? [], id: \.self) { ref in
                Button {
                    model.selectGitRef(ref, side: side)
                } label: {
                    if currentRef == ref {
                        Label(ref, systemImage: "checkmark")
                    } else {
                        Text(ref)
                    }
                }
            }

            Divider()

            Button("任意のrefを入力…") {
                customRefText = currentRef ?? ""
                showCustomRefInput = true
            }
        } label: {
            Label(currentRef ?? "作業ツリー", systemImage: "arrow.triangle.branch")
        }
        .disabled(model.phase.isRunning)
        .popover(isPresented: $showCustomRefInput) {
            VStack(alignment: .leading, spacing: 8) {
                Text("ref名を入力")
                    .font(.headline)
                TextField("例: origin/main, v1.0.0, コミットハッシュ", text: $customRefText)
                    .textFieldStyle(.roundedBorder)
                    .frame(minWidth: 240)
                    .onSubmit { applyCustomRef() }
                HStack {
                    Spacer()
                    Button("キャンセル") { showCustomRefInput = false }
                    Button("適用") { applyCustomRef() }
                        .keyboardShortcut(.defaultAction)
                        .disabled(customRefText.trimmingCharacters(in: .whitespaces).isEmpty)
                }
            }
            .padding(12)
        }
    }

    private func applyCustomRef() {
        let trimmed = customRefText.trimmingCharacters(in: .whitespaces)
        guard !trimmed.isEmpty else { return }
        model.selectGitRef(trimmed, side: side)
        showCustomRefInput = false
    }
}

/// 左ペイン: フィルタトグルと比較結果ツリー
private struct TreePane: View {
    @EnvironmentObject private var model: AppModel

    var body: some View {
        VStack(spacing: 0) {
            HStack(spacing: 6) {
                Image(systemName: "magnifyingglass")
                    .foregroundColor(.secondary)
                TextField("相対パスを検索", text: $model.resultSearchText)
                    .textFieldStyle(.roundedBorder)
                if !model.resultSearchText.isEmpty {
                    Button {
                        model.resultSearchText = ""
                    } label: {
                        Image(systemName: "xmark.circle.fill")
                    }
                    .buttonStyle(.borderless)
                    .foregroundColor(.secondary)
                    .help("検索をクリア")
                }
                Picker("表示", selection: $model.resultFilter) {
                    ForEach(AppModel.ResultFilter.allCases) { filter in
                        Text(filter.displayName).tag(filter)
                    }
                }
                .labelsHidden()
                .frame(width: 112)
                .help("表示する状態を選択")
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
            case .renaming(let done, let total):
                ProgressView(value: Double(done), total: Double(max(total, 1)))
                    .frame(width: 160)
                Text("リネーム/移動を検出中… \(done)/\(total)")
                    .foregroundColor(.secondary)
            }
            if model.phase == .idle, model.result == nil, model.errorMessage == nil {
                Spacer()
            } else if model.phase.isRunning {
                Spacer()
            }
            if model.watcherActive {
                Label("自動再比較 監視中", systemImage: "eye")
                    .foregroundColor(.secondary)
                    .help("フォルダの変更を監視しています。変更を検知すると自動で再比較します。")
            } else if let reason = model.autoRefreshPausedReason {
                Label("自動再比較 一時停止", systemImage: "pause.circle")
                    .foregroundColor(.orange)
                    .help(reason)
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

/// コピー計画プレビュー(読み取り専用)。コピー・削除・上書きの実行ボタンは持たない。
/// 方向の切替でAppModel.prepareSyncPlanを呼び直して計画を再生成する。
private struct SyncPlanSheet: View {
    @EnvironmentObject private var model: AppModel
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        VStack(spacing: 0) {
            HStack {
                Text("コピー計画プレビュー")
                    .font(.headline)
                Spacer()
                Picker("方向", selection: Binding(
                    get: { model.syncPlan?.direction ?? .leftToRight },
                    set: { model.prepareSyncPlan(direction: $0) }
                )) {
                    ForEach(SyncDirection.allCases) { d in
                        Text(d.displayName).tag(d)
                    }
                }
                .labelsHidden()
                .fixedSize()
                Button {
                    model.isSyncPlanSheetPresented = false
                    dismiss()
                } label: {
                    Image(systemName: "xmark.circle")
                }
                .buttonStyle(.borderless)
                .help("閉じる (⌘W)")
                .keyboardShortcut("w", modifiers: .command)
            }
            .padding(12)

            Divider()

            if let plan = model.syncPlan {
                SummaryStrip(plan: plan)
                Divider()
                ItemsTable(plan: plan)
                // エディションが同期の実行操作などを提供する場合のみ表示(Freeは読み取り専用)
                if let actions = model.edition.syncPlanActions(model: model, plan: plan) {
                    Divider()
                    actions
                }
            } else {
                VStack {
                    Spacer()
                    Text("コピー計画を表示するには「コピー計画」メニューから生成してください")
                        .foregroundColor(.secondary)
                    Spacer()
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            }
        }
        .frame(minWidth: 700, minHeight: 480)
    }
}

/// 計画のサマリ行(読み取り専用)
private struct SummaryStrip: View {
    let plan: SyncPlan

    var body: some View {
        HStack(spacing: 12) {
            Text("\(plan.sourceRoot.lastPathComponent) → \(plan.destinationRoot.lastPathComponent)")
                .foregroundColor(.secondary)
                .lineLimit(1)
                .truncationMode(.middle)
            Spacer()
            SummaryLabel(color: .blue, text: "コピー \(plan.summary.copyFiles)")
            SummaryLabel(color: .teal, text: "ディレクトリ \(plan.summary.createDirectories)")
            SummaryLabel(color: .orange, text: "差分 \(plan.summary.conflicts)")
            SummaryLabel(color: .secondary, text: "保持 \(plan.summary.preservedDestinations)")
            if plan.summary.blocked > 0 {
                SummaryLabel(color: .red, text: "エラー \(plan.summary.blocked)")
            }
            if plan.summary.totalBytes > 0 {
                Text(byteCountText(plan.summary.totalBytes))
                    .foregroundColor(.secondary)
            }
        }
        .font(.callout)
        .padding(.horizontal, 12)
        .padding(.vertical, 8)
    }

    private func byteCountText(_ bytes: Int64) -> String {
        ByteCountFormatter.string(fromByteCount: bytes, countStyle: .file)
    }
}

/// 計画項目のテーブル(読み取り専用)
private struct ItemsTable: View {
    let plan: SyncPlan

    var body: some View {
        Table(plan.items) {
            TableColumn("状態") { item in
                Text(item.action.displayName)
                    .foregroundColor(color(for: item.action))
            }
            .width(min: 110, ideal: 140)
            TableColumn("相対パス") { item in
                Text(item.relativePath)
                    .lineLimit(1)
                    .truncationMode(.middle)
            }
            .width(min: 220, ideal: 320)
            TableColumn("コピー元") { item in
                Text(item.source?.path ?? "—")
                    .lineLimit(1)
                    .truncationMode(.middle)
                    .foregroundColor(.secondary)
            }
            .width(min: 160, ideal: 260)
            TableColumn("サイズ") { item in
                Text(item.bytes > 0 ? ByteCountFormatter.string(fromByteCount: item.bytes, countStyle: .file) : "—")
                    .foregroundColor(.secondary)
            }
            .width(min: 70, ideal: 80)
            TableColumn("備考") { item in
                Text(item.reason)
                    .lineLimit(1)
                    .truncationMode(.middle)
                    .foregroundColor(.secondary)
            }
            .width(min: 140, ideal: 220)
        }
    }

    private func color(for action: SyncAction) -> Color {
        switch action {
        case .copyFile: return .blue
        case .createDirectory: return .teal
        case .conflict: return .orange
        case .preservedDestination: return .secondary
        case .blocked: return .red
        }
    }
}
