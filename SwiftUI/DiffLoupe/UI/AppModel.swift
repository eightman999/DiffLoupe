// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: アプリ全体の状態管理。フォルダ選択(NSOpenPanel)・比較実行・進捗・キャンセルを担う

import SwiftUI
import AppKit
import UniformTypeIdentifiers
#if canImport(DiffLoupeCore)
import DiffLoupeCore
#endif

/// Foundationの`ComparisonResult`と衝突しないよう、フォルダ比較結果(Core)を指す別名。
/// モジュール分割時(DiffLoupeCore)と単一モジュール(Xcode)の両方で同じ型を指す。
#if canImport(DiffLoupeCore)
public typealias FolderComparisonResult = DiffLoupeCore.ComparisonResult
#else
public typealias FolderComparisonResult = ComparisonResult
#endif

@MainActor
public final class AppModel: ObservableObject {

    public enum Side {
        case left, right
    }

    public enum ResultFilter: String, CaseIterable, Identifiable {
        case all
        case differences
        case different
        case leftOnly
        case rightOnly
        case error
        case identical

        public var id: String { rawValue }

        public var displayName: String {
            switch self {
            case .all: return "すべて"
            case .differences: return "差分すべて"
            case .different: return "相違"
            case .leftOnly: return "左のみ"
            case .rightOnly: return "右のみ"
            case .error: return "エラー"
            case .identical: return "一致"
            }
        }

        public func matches(_ status: DiffStatus) -> Bool {
            switch self {
            case .all: return true
            case .differences: return status != .identical
            case .different: return status == .different
            case .leftOnly: return status == .leftOnly
            case .rightOnly: return status == .rightOnly
            case .error: return status == .error
            case .identical: return status == .identical
            }
        }
    }

    /// 3-way差分で選択するファイルの役割
    public enum ThreeWaySide: String, Sendable {
        case base, left, right
    }

    public enum Phase: Equatable {
        case idle
        case scanning(found: Int)
        case comparing(done: Int, total: Int)
        case renaming(done: Int, total: Int)

        public var isRunning: Bool { self != .idle }
    }

    // MARK: - 比較設定の永続化キー(UI・比較実行・テストで共用)

    public nonisolated static let recursiveComparisonKey = "recursiveComparison"
    public nonisolated static let useDefaultExcludesKey = "useDefaultExcludes"
    public nonisolated static let userExcludePatternsKey = "userExcludePatterns"
    public nonisolated static let userIncludePatternsKey = "userIncludePatterns"
    public nonisolated static let filterPresetsKey = "filterPresets"
    public nonisolated static let ignoreLineEndingsKey = "contentIgnoreLineEndings"
    public nonisolated static let ignoreTrailingWhitespaceKey = "contentIgnoreTrailingWhitespace"
    public nonisolated static let normalizeWhitespaceKey = "contentNormalizeWhitespace"
    public nonisolated static let ignoreCaseKey = "contentIgnoreCase"
    public nonisolated static let structuredModeKey = "contentStructuredMode"
    public nonisolated static let detectRenamesKey = "detectRenames"
    /// opt-inの自動再比較(フォルダ変更の監視)。既定オフで、オンにすると比較完了後にフォルダを
    /// 監視し、ファイルの追加・削除・変更を検知して自動で再比較する。
    public nonisolated static let autoRefreshKey = "autoRefreshEnabled"

    /// 自動再比較の連続実行上限。超過すると一時停止する(ビルド中などの連続変化による無限連鎖防止)。
    /// 設定トグルの再ONまたは手動比較でカウントと一時停止を解除する。
    public nonisolated static let maxAutoRefreshRuns = 3

    /// 連続自動再比較回数が上限を超えたら一時停止すべきか。純粋ロジック(テスト対象)。
    public nonisolated static func shouldPauseAutoRefresh(afterConsecutiveRuns runs: Int) -> Bool {
        runs > maxAutoRefreshRuns
    }

    /// 設定画面・比較実行で使うデフォルト値群(初回起動時・未設定時に有効)。
    public static let defaultSettings: [String: Any] = [
        recursiveComparisonKey: true,
        useDefaultExcludesKey: true,
        userExcludePatternsKey: "",
        userIncludePatternsKey: "",
        filterPresetsKey: "",
        ignoreLineEndingsKey: false,
        ignoreTrailingWhitespaceKey: false,
        normalizeWhitespaceKey: false,
        ignoreCaseKey: false,
        structuredModeKey: StructuredComparisonMode.disabled.rawValue,
        detectRenamesKey: false,
        autoRefreshKey: false
    ]

    // MARK: - 設定の純粋関数(テスト対象)

    /// パターン入力(1行1パターン・改行区切り)を正規化する。
    /// 前後空白の除去・空行の除去・空パターンの除去・重複の除去を行い、出現順は保つ。
    public nonisolated static func normalizePatterns(_ raw: String) -> [String] {
        var seen = Set<String>()
        var result: [String] = []
        for rawLine in raw.split(separator: "\n", omittingEmptySubsequences: false) {
            let line = rawLine.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !line.isEmpty else { continue }
            if seen.insert(line).inserted {
                result.append(line)
            }
        }
        return result
    }

    /// ユーザー除外globの入力(1行1パターン・改行区切り)を正規化する。
    public nonisolated static func normalizeExcludePatterns(_ raw: String) -> [String] {
        normalizePatterns(raw)
    }

    /// ユーザーインクルードglobの入力(1行1パターン・改行区切り)を正規化する。
    public nonisolated static func normalizeIncludePatterns(_ raw: String) -> [String] {
        normalizePatterns(raw)
    }

    /// UserDefaultsに保存された除外glob(改行区切り)を正規化して返す。
    public nonisolated static func savedExcludePatterns(defaults: UserDefaults) -> [String] {
        normalizeExcludePatterns(defaults.string(forKey: userExcludePatternsKey) ?? "")
    }

    /// UserDefaultsに保存されたインクルードglob(改行区切り)を正規化して返す。
    public nonisolated static func savedIncludePatterns(defaults: UserDefaults) -> [String] {
        normalizeIncludePatterns(defaults.string(forKey: userIncludePatternsKey) ?? "")
    }

    /// UserDefaultsから選択済みプリセットIDを返す(カンマ区切り・順序はFilterPreset.all準拠)。
    public nonisolated static func selectedFilterPresetIDs(defaults: UserDefaults) -> [String] {
        let ids = Set(
            (defaults.string(forKey: filterPresetsKey) ?? "")
                .split(separator: ",")
                .map { $0.trimmingCharacters(in: .whitespacesAndNewlines) }
                .filter { !$0.isEmpty }
        )
        guard !ids.isEmpty else { return [] }
        return FilterPreset.all.map(\.id).filter { ids.contains($0) }
    }

    /// UserDefaultsから選択済みプリセットを返す。
    public nonisolated static func selectedFilterPresets(defaults: UserDefaults) -> [FilterPreset] {
        selectedFilterPresetIDs(defaults: defaults).compactMap { FilterPreset.preset(id: $0) }
    }

    /// UserDefaultsから内容比較モードを復元する(全てオフ・構造化オフ = 厳密比較・後方互換)。
    public nonisolated static func contentComparison(defaults: UserDefaults) -> ContentComparison {
        ContentComparison(
            ignoreLineEndings: defaults.bool(forKey: ignoreLineEndingsKey),
            ignoreTrailingWhitespace: defaults.bool(forKey: ignoreTrailingWhitespaceKey),
            normalizeWhitespace: defaults.bool(forKey: normalizeWhitespaceKey),
            ignoreCase: defaults.bool(forKey: ignoreCaseKey),
            structuredMode: Self.structuredMode(defaults: defaults)
        )
    }

    /// UserDefaultsから構造化比較モードを復元する。未知・未設定は disabled(後方互換)。
    public nonisolated static func structuredMode(defaults: UserDefaults) -> StructuredComparisonMode {
        let raw = defaults.string(forKey: structuredModeKey) ?? ""
        return StructuredComparisonMode(rawValue: raw) ?? .disabled
    }

    /// UserDefaultsの全設定をFolderComparator.Optionsへ一括伝播する。
    /// 初期値は厳密比較・リネーム検出オフ・既定除外オン(既存挙動と同一)。
    /// 選択済みプリセットの除外パターンはユーザー除外へ展開して追加する(opt-inのみ)。
    public nonisolated static func makeOptions(defaults: UserDefaults, maxEntries: Int?) -> FolderComparator.Options {
        var excludePatterns = savedExcludePatterns(defaults: defaults)
        for preset in selectedFilterPresets(defaults: defaults) {
            excludePatterns.append(contentsOf: preset.excludePatterns)
        }
        return FolderComparator.Options(
            recursive: defaults.bool(forKey: recursiveComparisonKey),
            useDefaultExcludes: defaults.bool(forKey: useDefaultExcludesKey),
            maxEntries: maxEntries,
            userExcludePatterns: excludePatterns,
            userIncludePatterns: savedIncludePatterns(defaults: defaults),
            contentComparison: contentComparison(defaults: defaults),
            detectRenames: defaults.bool(forKey: detectRenamesKey)
        )
    }

    /// 件数上限(edition.maxFolderEntries)超過時に表示するアラートの内容
    public struct EntryLimitAlert: Identifiable {
        public let id = UUID()
        public let message: String
    }

    @Published public var leftFolder: URL?
    @Published public var rightFolder: URL?
    @Published public var phase: Phase = .idle
    @Published public var result: FolderComparisonResult?
    @Published public var errorMessage: String?
    @Published public var selection: String?
    @Published public var resultFilter: ResultFilter = .all
    @Published public var resultSearchText = ""
    /// コピー計画プレビュー(resultから生成される読み取り専用。適用ボタンは持たない)
    @Published public var syncPlan: SyncPlan?
    /// コピー計画シートの表示状態
    @Published public var isSyncPlanSheetPresented = false
    /// フォルダ変更の監視(watcher)が稼働中かどうか。自動再比較が有効で左右フォルダが
    /// 揃っている間だけtrueになる(ステータス表示用)。
    @Published public var watcherActive = false
    /// 自動再比較が上限超過で一時停止中かどうか。nilなら通常稼働。
    /// 非nilのときはwatcherを止め、理由をステータス表示する(「自動再比較 監視中」と区別)。
    @Published public var autoRefreshPausedReason: String?
    /// 3-way差分ビューの表示状態
    @Published public var isThreeWayPresented = false
    /// 3-way差分で選択したファイル(共通祖先/左/右)。読み取り専用で変更はしない。
    @Published public var threeWayBaseURL: URL?
    @Published public var threeWayLeftURL: URL?
    @Published public var threeWayRightURL: URL?

    /// 各サイドで検出されたgitリポジトリ(非リポジトリならnil)
    @Published public var leftGitRepo: GitRepository?
    @Published public var rightGitRepo: GitRepository?
    /// 各サイドで選択中のgit ref。nilなら「作業ツリー」(従来通りFileManager直参照)。
    @Published public var leftGitRef: String?
    @Published public var rightGitRef: String?
    /// 比較履歴(最近の比較ペア。先頭が最新。最大10件)
    @Published public var recentComparisons: [RecentComparison] = []
    @Published public var entryLimitAlert: EntryLimitAlert?

    private var compareTask: Task<Void, Never>?
    /// 自動再比較用のフォルダ監視器。左右フォルダが揃い設定が有効な間だけ保持する。
    private var directoryWatcher: DirectoryWatcher?
    /// 比較中にwatcherが変更を検知したことを示すフラグ。比較完了後に一度だけ再比較する。
    private var autoRefreshPending = false
    /// 比較完了時に復元する選択パス(recompare(preservingSelection:)用)
    private var selectionToRestore: String?
    /// 未保存の編集があるため自動再比較を保留中かどうか
    public private(set) var autoRefreshDeferred = false
    /// 連続して実行された自動再比較の回数。上限(maxAutoRefreshRuns)超過で一時停止する。
    /// 手動比較・設定トグルの再ONでリセットする。
    private var autoRefreshRunCount = 0
    /// 表示中の比較結果が参照しているgitスナップショットの一時ディレクトリ。
    /// 結果ツリーからの詳細表示(テキスト/画像diff)がこの中のURLを読むため、
    /// 比較完了後も破棄せず、次回比較開始まで保持する。
    private var activeSnapshots: [URL] = []
    /// 比較履歴の保存/復元ストア
    private let recentStore = RecentComparisonStore()
    /// エディション(Free/Pro)。件数上限と拡張ビューの差し込み元
    public let edition: DiffLoupeEdition

    public init(edition: DiffLoupeEdition) {
        self.edition = edition
        UserDefaults.standard.register(defaults: Self.defaultSettings)
        recentComparisons = recentStore.load()
    }

    deinit {
        // watcherがタイマーを保持し続けないよう、解放時に停止する。
        // (DirectoryWatcher自身のdeinitでも停止するが、明示的に止めておく)
        if let directoryWatcher {
            directoryWatcher.stop()
        }
    }

    public var canCompare: Bool {
        leftFolder != nil && rightFolder != nil && !phase.isRunning
    }

    // MARK: - 自動再比較(opt-in)

    /// フォルダ変更監視が有効かどうか。UserDefaultsから毎回読む(設定変更を即反映)。
    public var autoRefreshEnabled: Bool {
        UserDefaults.standard.bool(forKey: Self.autoRefreshKey)
    }

    /// 設定画面のトグルから呼ばれる。UserDefaultsへ保存し、watcherを開始/停止する。
    /// 再度オンにした場合は連続回数カウンタと一時停止を解除する(上限超過からの復帰)。
    public func setAutoRefreshEnabled(_ enabled: Bool) {
        UserDefaults.standard.set(enabled, forKey: Self.autoRefreshKey)
        if enabled {
            autoRefreshRunCount = 0
            autoRefreshPausedReason = nil
        }
        reconfigureWatcher()
    }

    /// 左右フォルダが揃い、自動再比較が有効なときだけwatcherを開始する。
    /// それ以外・比較実行中・一時停止中は停止する。比較完了後に一度再構成し、基準スナップショットを
    /// 取り直すことで、表示中結果との差分だけを次回検知する。
    /// git refを選択している側はスナップショット(一時ディレクトリ)との比較になるため、
    /// その側の作業ツリーは監視しない。片側だけrefなら作業ツリー側のみ監視し、両側refならwatcherを止める。
    /// スナップショットにはFolderComparatorの既定除外(.git/.DS_Store/node_modules)を適用する。
    private func reconfigureWatcher() {
        directoryWatcher?.stop()
        directoryWatcher = nil
        watcherActive = false
        guard autoRefreshEnabled, autoRefreshPausedReason == nil, !phase.isRunning else { return }
        var roots: [URL] = []
        if leftGitRef == nil, let left = leftFolder { roots.append(left) }
        if rightGitRef == nil, let right = rightFolder { roots.append(right) }
        guard !roots.isEmpty else { return }
        let excludedNames: Set<String> =
            UserDefaults.standard.bool(forKey: Self.useDefaultExcludesKey)
            ? FolderComparator.Options.defaultExcludedNames : []
        let watcher = DirectoryWatcher(
            roots: roots,
            interval: 1.0,
            excludedNames: excludedNames
        ) { [weak self] in
            Task { @MainActor in
                self?.handleWatcherChange()
            }
        }
        directoryWatcher = watcher
        watcher.start()
        watcherActive = true
    }

    /// watcherのコールバック(MainActorへ戻された後)。比較中はpendingに留め、
    /// 完了後の再比較は一度だけに抑える。待機中は即時再比較する。
    private func handleWatcherChange() {
        guard autoRefreshEnabled else { return }
        if phase.isRunning {
            autoRefreshPending = true
        } else {
            autoRecompare()
        }
    }

    /// watcher由来の再比較。startCompareのphase.isRunningガードにより多重起動しない。
    /// 連続実行回数を数え、上限超過で自動再比較を一時停止する(無限連鎖防止)。
    private func autoRecompare() {
        guard autoRefreshEnabled,
              autoRefreshPausedReason == nil,
              leftFolder != nil, rightFolder != nil,
              !phase.isRunning else { return }
        // 未保存の編集がある間は、その回を保留する(編集を失わせない。保存後に resumeDeferredAutoRefresh で再開)
        if edition.hasUnsavedChanges {
            autoRefreshDeferred = true
            return
        }
        autoRefreshDeferred = false
        autoRefreshPending = false
        autoRefreshRunCount += 1
        if Self.shouldPauseAutoRefresh(afterConsecutiveRuns: autoRefreshRunCount) {
            pauseAutoRefreshAfterExcessiveRuns()
            return
        }
        beginCompare(userInitiated: false)
    }

    /// 未保存の編集で保留していた自動再比較を再開する(保存・破棄の後にエディション側から呼ぶ)。
    /// 保留が無ければ何もしない。なお未保存が残っていれば再び保留する。
    public func resumeDeferredAutoRefresh() {
        guard autoRefreshDeferred else { return }
        autoRecompare()
    }

    /// 自動再比較の連続実行が上限に達したときの一時停止。watcherを止め、UIに理由を出す。
    /// 設定トグルの再ON(setAutoRefreshEnabled(true))または手動比較(beginCompare(userInitiated: true))で解除される。
    private func pauseAutoRefreshAfterExcessiveRuns() {
        autoRefreshPending = false
        autoRefreshPausedReason =
            "自動再比較が\(Self.maxAutoRefreshRuns)回連続したため一時停止しました。"
            + "設定のトグルを再度オンにするか、手動で比較してください。"
        directoryWatcher?.stop()
        directoryWatcher = nil
        watcherActive = false
    }

    /// ステータスバー等に表示する「フォルダ名 @ ref」形式のラベル。作業ツリー選択時はフォルダ名のみ。
    public func displayLabel(for side: Side) -> String? {
        let url = (side == .left) ? leftFolder : rightFolder
        guard let url else { return nil }
        let ref = (side == .left) ? leftGitRef : rightGitRef
        let name = url.lastPathComponent
        guard let ref else { return name }
        return "\(name) @ \(ref)"
    }

    public var selectedEntry: ComparedEntry? {
        guard let selection else { return nil }
        return result?.entriesByPath[selection]
    }

    public func entryURL(for entry: ComparedEntry, side: Side) -> URL? {
        switch side {
        case .left: return entry.leftURL
        case .right: return entry.rightURL
        }
    }

    @discardableResult
    public func openEntry(_ entry: ComparedEntry, side: Side) -> Bool {
        guard let url = entryURL(for: entry, side: side) else { return false }
        return NSWorkspace.shared.open(url)
    }

    @discardableResult
    public func revealEntryInFinder(_ entry: ComparedEntry, side: Side) -> Bool {
        guard let url = entryURL(for: entry, side: side) else { return false }
        NSWorkspace.shared.activateFileViewerSelecting([url])
        return true
    }

    @discardableResult
    public func copyEntryPath(
        _ entry: ComparedEntry,
        side: Side,
        to pasteboard: NSPasteboard = .general
    ) -> Bool {
        guard let url = entryURL(for: entry, side: side) else { return false }
        pasteboard.clearContents()
        return pasteboard.setString(url.path, forType: .string)
    }

    /// 詳細差分ビューが使う現在の内容比較モード。UserDefaultsから毎回読み、
    /// 設定変更後の再比較・再表示に反映される(ContentView再評価時に再計算される)。
    public var currentContentComparison: ContentComparison {
        Self.contentComparison(defaults: .standard)
    }

    /// ツリー表示用ルート(状態・相対パスの絞り込み適用後)
    public var displayNodes: [DiffNode] {
        guard let root = result?.rootNode else { return [] }
        let query = resultSearchText.trimmingCharacters(in: .whitespacesAndNewlines)
        return (root.children ?? []).compactMap {
            Self.filterNode($0, filter: resultFilter, query: query)
        }
    }

    private static func filterNode(
        _ node: DiffNode,
        filter: ResultFilter,
        query: String
    ) -> DiffNode? {
        let matchesQuery = query.isEmpty || node.id.localizedCaseInsensitiveContains(query)
        guard node.isDirectory else {
            return matchesQuery && filter.matches(node.status) ? node : nil
        }

        let originalChildren = node.children ?? []
        let filteredChildren = originalChildren.compactMap {
            filterNode($0, filter: filter, query: query)
        }
        let isEmptyDirectory = originalChildren.isEmpty
        guard !filteredChildren.isEmpty
                || (isEmptyDirectory && matchesQuery && filter.matches(node.status)) else {
            return nil
        }
        var filtered = node
        filtered.children = node.children == nil ? nil : filteredChildren
        return filtered
    }

    // MARK: - 操作

    /// フォルダアクセスは必ずNSOpenPanel経由で取得する(技術制約)
    public func pickFolder(side: Side) {
        guard edition.confirmDiscardUnsavedChanges() else { return }
        let panel = NSOpenPanel()
        panel.canChooseFiles = false
        panel.canChooseDirectories = true
        panel.allowsMultipleSelection = false
        panel.prompt = "選択"
        panel.message = side == .left ? "左側のフォルダを選択してください" : "右側のフォルダを選択してください"
        guard panel.runModal() == .OK, let url = panel.url else { return }
        applyFolder(url, side: side)
    }

    /// フォルダURLを左右どちらかに設定する(パネル選択・ドロップで共用)。
    /// gitリポジトリを検出してrepo/refを初期化する(従来のpickFolder挙動と同一)。
    private func applyFolder(_ url: URL, side: Side) {
        let detected = GitRepository.detect(url: url)
        switch side {
        case .left:
            leftFolder = url
            leftGitRepo = detected
            leftGitRef = nil
        case .right:
            rightFolder = url
            rightGitRepo = detected
            rightGitRef = nil
        }
        reconfigureWatcher()
    }

    /// Finderドラッグ&ドロップで受け付けたフォルダURLを設定する。
    /// フォルダ以外(ファイル・存在しないパス・実行中)は無視する。
    public func acceptDroppedFolder(url: URL, side: Side) {
        guard !phase.isRunning else { return }
        var isDirectory: ObjCBool = false
        guard FileManager.default.fileExists(atPath: url.path, isDirectory: &isDirectory),
              isDirectory.boolValue else { return }
        guard edition.confirmDiscardUnsavedChanges() else { return }
        applyFolder(url.resolvingSymlinksInPath(), side: side)
    }

    /// 左右のフォルダ・gitリポジトリ・git refを一貫して交換する。
    /// 実行中は無効。表示中の結果は左右対応が崩れるため破棄する(選択・エラーも同様にクリア)。
    public func swapSides() {
        guard !phase.isRunning else { return }
        guard edition.confirmDiscardUnsavedChanges() else { return }

        let folder = leftFolder; leftFolder = rightFolder; rightFolder = folder
        let repo = leftGitRepo; leftGitRepo = rightGitRepo; rightGitRepo = repo
        let ref = leftGitRef; leftGitRef = rightGitRef; rightGitRef = ref

        // 前回結果が参照していたgitスナップショットは、結果を破棄するこのタイミングで掃除する
        for url in activeSnapshots { GitRepository.cleanup(url) }
        activeSnapshots = []
        result = nil
        selection = nil
        errorMessage = nil
        syncPlan = nil
        isSyncPlanSheetPresented = false
        reconfigureWatcher()
    }

    /// 結果からコピー計画を生成してシートを開く。結果が無いときは何もしない。
    /// 実際のコピー・削除・上書きは行わない(プレビュー専用)。
    public func prepareSyncPlan(direction: SyncDirection) {
        guard let result else { return }
        syncPlan = SyncPlan.make(from: result, direction: direction)
        isSyncPlanSheetPresented = true
    }

    // MARK: - 3-way差分(読み取り専用)

    /// 3-way差分ビューの表示状態を開く(コマンドメニュー・ContentViewのシート起動用)。
    public func showThreeWayComparison() {
        isThreeWayPresented = true
    }

    /// 3-way差分ビューの表示状態を閉じる。選択済みのファイルは保持する(再表示が容易)。
    public func closeThreeWayComparison() {
        isThreeWayPresented = false
    }

    /// 3-way差分用のファイルをNSOpenPanelで選択する。読み取り専用で変更・書き込みはしない。
    public func pickThreeWayFile(side: ThreeWaySide) {
        let panel = NSOpenPanel()
        panel.canChooseFiles = true
        panel.canChooseDirectories = false
        panel.allowsMultipleSelection = false
        panel.prompt = "選択"
        switch side {
        case .base:
            panel.message = "共通祖先(ベース)のファイルを選択してください"
        case .left:
            panel.message = "左側のファイルを選択してください"
        case .right:
            panel.message = "右側のファイルを選択してください"
        }
        guard panel.runModal() == .OK, let url = panel.url else { return }
        switch side {
        case .base: threeWayBaseURL = url
        case .left: threeWayLeftURL = url
        case .right: threeWayRightURL = url
        }
    }

    /// UIのref選択メニューから呼ばれる。nilを渡すと「作業ツリー」(従来動作)に戻る。
    /// refの変更は監視対象にも影響するため、即座にwatcherを再構成する
    /// (ref側の作業ツリー監視をやめ/始める)。
    public func selectGitRef(_ ref: String?, side: Side) {
        switch side {
        case .left: leftGitRef = ref
        case .right: rightGitRef = ref
        }
        reconfigureWatcher()
    }

    /// 履歴メニューの項目有効判定用。パスがディレクトリとして存在する場合のみ true。
    public static func directoryExists(_ path: String) -> Bool {
        var isDirectory: ObjCBool = false
        return FileManager.default.fileExists(atPath: path, isDirectory: &isDirectory)
            && isDirectory.boolValue
    }

    /// 履歴から左右のフォルダ・git refを復元する。実行中は無効。
    /// 存在しない・ディレクトリでないパスが含まれる場合は何もしない(安全)。
    public func restoreComparison(_ item: RecentComparison) {
        guard !phase.isRunning else { return }
        guard Self.directoryExists(item.leftPath), Self.directoryExists(item.rightPath) else { return }
        guard edition.confirmDiscardUnsavedChanges() else { return }

        // 表示中の結果・スナップショットは左右が変わるため破棄する
        for url in activeSnapshots { GitRepository.cleanup(url) }
        activeSnapshots = []
        result = nil
        selection = nil
        errorMessage = nil
        syncPlan = nil
        isSyncPlanSheetPresented = false

        let leftURL = URL(fileURLWithPath: item.leftPath, isDirectory: true)
        let rightURL = URL(fileURLWithPath: item.rightPath, isDirectory: true)
        leftFolder = leftURL
        rightFolder = rightURL
        leftGitRepo = GitRepository.detect(url: leftURL)
        rightGitRepo = GitRepository.detect(url: rightURL)
        leftGitRef = item.leftGitRef
        rightGitRef = item.rightGitRef
        reconfigureWatcher()
    }

    /// 「比較」ボタンから呼ばれるユーザー操作による比較。手動比較は自動再比較の連続カウンタと
    /// 一時停止を解除する(上限超過からの復帰経路)。
    public func startCompare() {
        beginCompare(userInitiated: true)
    }

    /// 比較をやり直し、完了後に同じ相対パスが結果にあれば選択を復元する(同期実行後などに使う)。
    /// 未保存の編集の確認で中止された場合は何もしない。
    public func recompare(preservingSelection: Bool) {
        let previous = preservingSelection ? selection : nil
        beginCompare(userInitiated: true, restoringSelection: previous)
    }

    /// 比較の共通エントリポイント。userInitiated=true(手動)のときだけ自動再比較の一時停止を解除する。
    /// 自動再比較(autoRecompare)からはfalseで呼ばれ、連続カウンタを増やしたままの連鎖となる。
    private func beginCompare(userInitiated: Bool, restoringSelection: String? = nil) {
        guard let left = leftFolder, let right = rightFolder, !phase.isRunning else { return }
        // 手動比較は未保存の編集の確認を通す(自動再比較はautoRecompare側で保留判定済み)
        if userInitiated, !edition.confirmDiscardUnsavedChanges() { return }
        selectionToRestore = restoringSelection

        if userInitiated {
            autoRefreshRunCount = 0
            autoRefreshPausedReason = nil
        }

        let options = Self.makeOptions(defaults: .standard, maxEntries: edition.maxFolderEntries)
        phase = .scanning(found: 0)
        result = nil
        selection = nil
        errorMessage = nil
        syncPlan = nil
        isSyncPlanSheetPresented = false
        entryLimitAlert = nil

        // 比較開始時に履歴へ記録する(同一ペアは先頭へ移動)
        recentComparisons = recentStore.record(RecentComparison(
            leftPath: left.path,
            rightPath: right.path,
            leftGitRef: leftGitRef,
            rightGitRef: rightGitRef,
            date: Date()
        ))

        // 前回結果が参照していたgitスナップショットは、結果を差し替えるこのタイミングで破棄する
        for url in activeSnapshots { GitRepository.cleanup(url) }
        activeSnapshots = []

        // gitRefが設定されているサイドはスナップショットを実体化してそのURLを使う。
        // 未設定なら従来通り作業ツリーのURLをそのまま使う(後方互換)。
        let leftSnapshotSource: (repo: GitRepository, ref: String)? = leftGitRef.flatMap { ref in
            leftGitRepo.map { ($0, ref) }
        }
        let rightSnapshotSource: (repo: GitRepository, ref: String)? = rightGitRef.flatMap { ref in
            rightGitRepo.map { ($0, ref) }
        }

        compareTask = Task.detached { [weak self] in
            var leftSnapshot: URL?
            var rightSnapshot: URL?
            // 成功時は結果表示が一時ディレクトリ内のURLを参照し続けるため、ここでは削除せず
            // AppModel(activeSnapshots)へ引き継ぐ。失敗・キャンセル時のみこのdeferで破棄する。
            var snapshotsHandedOff = false
            defer {
                if !snapshotsHandedOff {
                    if let leftSnapshot { GitRepository.cleanup(leftSnapshot) }
                    if let rightSnapshot { GitRepository.cleanup(rightSnapshot) }
                }
            }
            do {
                let compareLeft: URL
                if let source = leftSnapshotSource {
                    let snapshot = try source.repo.materializeSnapshot(ref: source.ref)
                    leftSnapshot = snapshot
                    compareLeft = snapshot
                } else {
                    compareLeft = left
                }

                let compareRight: URL
                if let source = rightSnapshotSource {
                    let snapshot = try source.repo.materializeSnapshot(ref: source.ref)
                    rightSnapshot = snapshot
                    compareRight = snapshot
                } else {
                    compareRight = right
                }

                let comparison = try await FolderComparator().compare(
                    left: compareLeft, right: compareRight, options: options
                ) { progress in
                    Task { @MainActor [weak self] in
                        guard let self, self.phase.isRunning else { return }
                        switch progress {
                        case .scanning(let found):
                            self.phase = .scanning(found: found)
                        case .comparing(let done, let total):
                            self.phase = .comparing(done: done, total: total)
                        case .renames(let done, let total):
                            self.phase = .renaming(done: done, total: total)
                        }
                    }
                }
                let snapshots = [leftSnapshot, rightSnapshot].compactMap { $0 }
                snapshotsHandedOff = await MainActor.run { [weak self] in
                    guard let self else { return false }
                    self.activeSnapshots = snapshots
                    self.finish(with: comparison, error: nil)
                    return true
                }
            } catch let error as GitRepository.GitError {
                await MainActor.run { self?.finish(with: nil, error: error.localizedDescription) }
            } catch let error as FolderComparator.EntryLimitExceeded {
                await MainActor.run {
                    self?.finish(
                        with: nil,
                        error: error.localizedDescription,
                        limitAlert: .init(message: error.localizedDescription)
                    )
                }
            } catch is CancellationError {
                await MainActor.run { self?.finish(with: nil, error: nil) }
            } catch {
                await MainActor.run { self?.finish(with: nil, error: "比較に失敗しました: \(error.localizedDescription)") }
            }
        }
    }

    public func cancelCompare() {
        autoRefreshPending = false
        compareTask?.cancel()
    }

    public func openSettings() {
        NSApp.sendAction(Selector(("showSettingsWindow:")), to: nil, from: nil)
    }

    /// 「レポート」メニューから呼ばれる。結果があるときのみNSSavePanelを開いて保存する。
    /// 実際のファイル書き込みはユーザーが保存ボタンを押したときだけ行う。
    /// 既存ファイルの上書きはNSSavePanelの挙動に任せず明示的に確認し、
    /// 保存エラーは既存のerrorMessage経路で通知する。
    public func saveReport(as format: ReportFormat) {
        guard let result else { return }

        let panel = NSSavePanel()
        panel.title = "レポートを保存"
        panel.allowedContentTypes = [format.utType]
        panel.canCreateDirectories = true
        panel.isExtensionHidden = false
        panel.nameFieldStringValue = Self.defaultReportFileName(for: format)

        guard panel.runModal() == .OK, let url = panel.url else { return }

        // 上書き確認(NSSavePanel/ファイルシステムの安全な挙動に依存しない)
        if FileManager.default.fileExists(atPath: url.path) {
            let alert = NSAlert()
            alert.alertStyle = .warning
            alert.messageText = "上書きしますか？"
            alert.informativeText = "「\(url.lastPathComponent)」は既に存在します。置き換えますか？"
            alert.addButton(withTitle: "上書き")
            alert.addButton(withTitle: "キャンセル")
            guard alert.runModal() == .alertFirstButtonReturn else { return }
        }

        do {
            let report = ComparisonReport(
                result: result,
                generatedAt: Date(),
                leftGitRef: leftGitRef,
                rightGitRef: rightGitRef
            )
            try report.render(as: format).write(to: url, atomically: true, encoding: .utf8)
        } catch {
            errorMessage = "レポート保存に失敗しました: \(error.localizedDescription)"
        }
    }

    /// 既定のレポートファイル名(ロケール非依存・UTC)。
    /// 例: DiffLoupe-report-20260812-013000.html
    private static func defaultReportFileName(for format: ReportFormat) -> String {
        let formatter = DateFormatter()
        formatter.dateFormat = "yyyyMMdd-HHmmss"
        formatter.locale = Locale(identifier: "en_US_POSIX")
        formatter.timeZone = TimeZone(identifier: "UTC")
        return "DiffLoupe-report-\(formatter.string(from: Date())).\(format.fileExtension)"
    }

    private func finish(
        with comparison: FolderComparisonResult?,
        error: String?,
        limitAlert: EntryLimitAlert? = nil
    ) {
        result = comparison
        if let path = selectionToRestore, let comparison, comparison.entriesByPath[path] != nil {
            selection = path
        }
        selectionToRestore = nil
        errorMessage = error
        entryLimitAlert = limitAlert
        phase = .idle
        compareTask = nil
        // 比較完了で基準スナップショットを取り直し、表示結果との差分だけを次回検知する
        reconfigureWatcher()
        if comparison != nil, autoRefreshPending {
            // 比較中に検知した変更は、完了後に一度だけ再比較する(成功時のみ)
            autoRefreshPending = false
            autoRecompare()
        } else {
            autoRefreshPending = false
        }
    }
}
