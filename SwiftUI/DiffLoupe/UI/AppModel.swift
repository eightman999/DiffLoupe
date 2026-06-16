// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: アプリ全体の状態管理。フォルダ選択(NSOpenPanel)・比較実行・進捗・キャンセルを担う

import SwiftUI
import AppKit

@MainActor
final class AppModel: ObservableObject {

    enum Side {
        case left, right
    }

    enum Phase: Equatable {
        case idle
        case scanning(found: Int)
        case comparing(done: Int, total: Int)

        var isRunning: Bool { self != .idle }
    }

    static let useDefaultExcludesKey = "useDefaultExcludes"

#if PRO
    struct LicenseLimitAlert: Identifiable {
        let id = UUID()
        let message: String
    }
#endif

    @Published var leftFolder: URL?
    @Published var rightFolder: URL?
    @Published var phase: Phase = .idle
    @Published var result: ComparisonResult?
    @Published var errorMessage: String?
    @Published var selection: String?
    @Published var showOnlyDifferences = false
#if PRO
    @Published var licenseLimitAlert: LicenseLimitAlert?
#endif

    private var compareTask: Task<Void, Never>?
#if PRO
    private let licenseManager: LicenseManager

    init(licenseManager: LicenseManager) {
        self.licenseManager = licenseManager
        UserDefaults.standard.register(defaults: [Self.useDefaultExcludesKey: true])
    }
#else
    init() {
        UserDefaults.standard.register(defaults: [Self.useDefaultExcludesKey: true])
    }
#endif

    var canCompare: Bool {
        leftFolder != nil && rightFolder != nil && !phase.isRunning
    }

    var selectedEntry: ComparedEntry? {
        guard let selection else { return nil }
        return result?.entriesByPath[selection]
    }

    /// ツリー表示用ルート(「差分のみ表示」フィルタ適用後)
    var displayNodes: [DiffNode] {
        guard let root = result?.rootNode else { return [] }
        guard showOnlyDifferences else { return root.children ?? [] }
        return (root.children ?? []).compactMap(Self.filterDifferences)
    }

    private static func filterDifferences(_ node: DiffNode) -> DiffNode? {
        if node.status == .identical { return nil }
        var filtered = node
        if let children = node.children {
            filtered.children = children.compactMap(filterDifferences)
        }
        return filtered
    }

    // MARK: - 操作

    /// フォルダアクセスは必ずNSOpenPanel経由で取得する(技術制約)
    func pickFolder(side: Side) {
        let panel = NSOpenPanel()
        panel.canChooseFiles = false
        panel.canChooseDirectories = true
        panel.allowsMultipleSelection = false
        panel.prompt = "選択"
        panel.message = side == .left ? "左側のフォルダを選択してください" : "右側のフォルダを選択してください"
        guard panel.runModal() == .OK, let url = panel.url else { return }
        switch side {
        case .left: leftFolder = url
        case .right: rightFolder = url
        }
    }

    func startCompare() {
        guard let left = leftFolder, let right = rightFolder, !phase.isRunning else { return }

#if PRO
        let maxEntriesForCompare = licenseManager.isPro ? nil : FolderComparator.Options.freeMaxEntries
#else
        let maxEntriesForCompare: Int? = nil
#endif
        let options = FolderComparator.Options(
            useDefaultExcludes: UserDefaults.standard.bool(forKey: Self.useDefaultExcludesKey),
            maxEntries: maxEntriesForCompare
        )
        phase = .scanning(found: 0)
        result = nil
        selection = nil
        errorMessage = nil
#if PRO
        licenseLimitAlert = nil
#endif

        compareTask = Task.detached { [weak self] in
            do {
                let comparison = try await FolderComparator().compare(
                    left: left, right: right, options: options
                ) { progress in
                    Task { @MainActor [weak self] in
                        guard let self, self.phase.isRunning else { return }
                        switch progress {
                        case .scanning(let found):
                            self.phase = .scanning(found: found)
                        case .comparing(let done, let total):
                            self.phase = .comparing(done: done, total: total)
                        }
                    }
                }
                await MainActor.run { self?.finish(with: comparison, error: nil) }
            } catch let error as FolderComparator.EntryLimitExceeded {
                await MainActor.run {
#if PRO
                    self?.finish(
                        with: nil,
                        error: error.localizedDescription,
                        limitAlert: .init(message: error.localizedDescription)
                    )
#else
                    self?.finish(
                        with: nil,
                        error: error.localizedDescription
                    )
#endif
                }
            } catch is CancellationError {
                await MainActor.run { self?.finish(with: nil, error: nil) }
            } catch {
                await MainActor.run { self?.finish(with: nil, error: "比較に失敗しました: \(error.localizedDescription)") }
            }
        }
    }

    func cancelCompare() {
        compareTask?.cancel()
    }

    func openSettings() {
        NSApp.sendAction(Selector(("showSettingsWindow:")), to: nil, from: nil)
    }

#if PRO
    private func finish(
        with comparison: ComparisonResult?,
        error: String?,
        limitAlert: LicenseLimitAlert? = nil
    ) {
        result = comparison
        errorMessage = error
        licenseLimitAlert = limitAlert
        phase = .idle
        compareTask = nil
    }
#else
    private func finish(
        with comparison: ComparisonResult?,
        error: String?
    ) {
        result = comparison
        errorMessage = error
        phase = .idle
        compareTask = nil
    }
#endif
}
