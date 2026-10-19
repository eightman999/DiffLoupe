// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: AppModelの左右交換(swapSides)と履歴復元(restoreComparison)を検証する。
// @MainActor上で実行する純粋ロジックのテスト(フォルダ選択UIは対象外)

import XCTest
import AppKit
@testable import DiffLoupe

@MainActor
final class AppModelTests: XCTestCase {

    private static func makeResult() -> ComparisonResult {
        ComparisonResult(
            leftRoot: URL(fileURLWithPath: "/tmp/left", isDirectory: true),
            rightRoot: URL(fileURLWithPath: "/tmp/right", isDirectory: true),
            entries: [],
            entriesByPath: [:],
            rootNode: DiffNode(id: "", name: "", isDirectory: true, status: .different, children: nil),
            summary: ComparisonResult.Summary()
        )
    }

    private static func makeFilterResult() -> ComparisonResult {
        let root = DiffNode(
            id: "",
            name: "",
            isDirectory: true,
            status: .different,
            children: [
                DiffNode(id: "docs", name: "docs", isDirectory: true, status: .different, children: [
                    DiffNode(id: "docs/README.md", name: "README.md", isDirectory: false, status: .identical, children: nil),
                    DiffNode(id: "docs/Guide.md", name: "Guide.md", isDirectory: false, status: .different, children: nil)
                ]),
                DiffNode(id: "src", name: "src", isDirectory: true, status: .different, children: [
                    DiffNode(id: "src/Added.swift", name: "Added.swift", isDirectory: false, status: .leftOnly, children: nil),
                    DiffNode(id: "src/Removed.swift", name: "Removed.swift", isDirectory: false, status: .rightOnly, children: nil)
                ]),
                DiffNode(id: "errors.log", name: "errors.log", isDirectory: false, status: .error, children: nil),
                DiffNode(id: "same.txt", name: "same.txt", isDirectory: false, status: .identical, children: nil),
                DiffNode(id: "empty-left", name: "empty-left", isDirectory: true, status: .leftOnly, children: [])
            ]
        )
        return ComparisonResult(
            leftRoot: URL(fileURLWithPath: "/tmp/left", isDirectory: true),
            rightRoot: URL(fileURLWithPath: "/tmp/right", isDirectory: true),
            entries: [],
            entriesByPath: [:],
            rootNode: root,
            summary: ComparisonResult.Summary()
        )
    }

    private static func flattenedIDs(_ nodes: [DiffNode]) -> [String] {
        nodes.flatMap { [$0.id] + flattenedIDs($0.children ?? []) }
    }

    private static func makeActionEntry(leftURL: URL?, rightURL: URL?) -> ComparedEntry {
        ComparedEntry(
            relativePath: "folder/file.txt",
            name: "file.txt",
            isDirectory: false,
            status: rightURL == nil ? .leftOnly : .different,
            leftURL: leftURL,
            rightURL: rightURL,
            leftSize: 1,
            rightSize: rightURL == nil ? -1 : 1,
            errorMessage: nil,
            leftModificationDate: nil,
            rightModificationDate: nil,
            leftPOSIX: nil,
            rightPOSIX: nil,
            renameCandidate: nil
        )
    }

    private func configurePair(_ model: AppModel) {
        let leftURL = URL(fileURLWithPath: "/tmp/left", isDirectory: true)
        let rightURL = URL(fileURLWithPath: "/tmp/right", isDirectory: true)
        model.leftFolder = leftURL
        model.rightFolder = rightURL
        model.leftGitRepo = GitRepository(toplevel: leftURL)
        model.rightGitRepo = GitRepository(toplevel: rightURL)
        model.leftGitRef = "main"
        model.rightGitRef = "HEAD"
    }

    // MARK: - swapSides

    func testSwapSidesExchangesFoldersReposAndRefs() {
        let model = AppModel(edition: FreeEdition())
        configurePair(model)

        model.swapSides()

        XCTAssertEqual(model.leftFolder?.path, "/tmp/right")
        XCTAssertEqual(model.rightFolder?.path, "/tmp/left")
        XCTAssertEqual(model.leftGitRepo?.toplevel.path, "/tmp/right")
        XCTAssertEqual(model.rightGitRepo?.toplevel.path, "/tmp/left")
        XCTAssertEqual(model.leftGitRef, "HEAD")
        XCTAssertEqual(model.rightGitRef, "main")
    }

    func testSwapSidesClearsResultSelectionAndError() {
        let model = AppModel(edition: FreeEdition())
        configurePair(model)
        model.result = Self.makeResult()
        model.selection = "some/path"
        model.errorMessage = "前回のエラー"

        model.swapSides()

        XCTAssertNil(model.result)
        XCTAssertNil(model.selection)
        XCTAssertNil(model.errorMessage)
    }

    func testSwapSidesIsNoOpWhileRunning() {
        let model = AppModel(edition: FreeEdition())
        configurePair(model)
        model.phase = .scanning(found: 1)

        model.swapSides()

        XCTAssertEqual(model.leftFolder?.path, "/tmp/left")
        XCTAssertEqual(model.rightFolder?.path, "/tmp/right")
        XCTAssertEqual(model.leftGitRef, "main")
        XCTAssertEqual(model.rightGitRef, "HEAD")
    }

    func testSwapSidesSwapsNilRefsSafely() {
        let model = AppModel(edition: FreeEdition())
        configurePair(model)
        model.leftGitRef = nil
        model.rightGitRef = nil

        model.swapSides()

        XCTAssertNil(model.leftGitRef)
        XCTAssertNil(model.rightGitRef)
    }

    func testSwapSidesTwiceRestoresOriginalOrder() {
        let model = AppModel(edition: FreeEdition())
        configurePair(model)

        model.swapSides()
        model.swapSides()

        XCTAssertEqual(model.leftFolder?.path, "/tmp/left")
        XCTAssertEqual(model.rightFolder?.path, "/tmp/right")
        XCTAssertEqual(model.leftGitRef, "main")
        XCTAssertEqual(model.rightGitRef, "HEAD")
    }

    // MARK: - restoreComparison

    func testRestoreComparisonIgnoresMissingPaths() {
        let model = AppModel(edition: FreeEdition())
        configurePair(model)
        let item = RecentComparison(
            leftPath: "/definitely/not/exists/left",
            rightPath: "/definitely/not/exists/right",
            leftGitRef: "main",
            rightGitRef: nil,
            date: Date()
        )

        model.restoreComparison(item)

        XCTAssertEqual(model.leftFolder?.path, "/tmp/left")
        XCTAssertEqual(model.rightFolder?.path, "/tmp/right")
        XCTAssertEqual(model.leftGitRef, "main")
        XCTAssertEqual(model.rightGitRef, "HEAD")
    }

    func testRestoreComparisonIgnoresNonDirectoryPaths() {
        let model = AppModel(edition: FreeEdition())
        configurePair(model)
        let fileURL = FileManager.default.temporaryDirectory
            .appendingPathComponent("AppModelTests-\(UUID().uuidString).txt")
        try? "x".write(to: fileURL, atomically: true, encoding: .utf8)
        defer { try? FileManager.default.removeItem(at: fileURL) }
        let item = RecentComparison(
            leftPath: fileURL.path,
            rightPath: "/tmp/right",
            leftGitRef: nil,
            rightGitRef: nil,
            date: Date()
        )

        model.restoreComparison(item)

        XCTAssertEqual(model.leftFolder?.path, "/tmp/left")
        XCTAssertEqual(model.rightFolder?.path, "/tmp/right")
    }

    func testRestoreComparisonSetsBothSidesWithRefs() throws {
        let model = AppModel(edition: FreeEdition())
        configurePair(model)
        let base = FileManager.default.temporaryDirectory
            .appendingPathComponent("AppModelTests-restore-\(UUID().uuidString)")
        let left = base.appendingPathComponent("left")
        let right = base.appendingPathComponent("right")
        try FileManager.default.createDirectory(at: left, withIntermediateDirectories: true)
        try FileManager.default.createDirectory(at: right, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: base) }

        let item = RecentComparison(
            leftPath: left.path,
            rightPath: right.path,
            leftGitRef: "feature",
            rightGitRef: nil,
            date: Date()
        )

        model.restoreComparison(item)

        XCTAssertEqual(model.leftFolder?.path, left.path)
        XCTAssertEqual(model.rightFolder?.path, right.path)
        XCTAssertEqual(model.leftGitRef, "feature")
        XCTAssertNil(model.rightGitRef)
    }

    // MARK: - 自動再比較(opt-in)

    private func resetAutoRefreshDefaults() {
        UserDefaults.standard.removeObject(forKey: AppModel.autoRefreshKey)
    }

    func testAutoRefreshDefaultsToFalse() {
        resetAutoRefreshDefaults()
        XCTAssertEqual(AppModel.defaultSettings[AppModel.autoRefreshKey] as? Bool, false)
        let model = AppModel(edition: FreeEdition())
        XCTAssertFalse(model.autoRefreshEnabled)
        XCTAssertFalse(model.watcherActive)
    }

    func testWatcherActivatesOnlyWhenEnabledAndFoldersSet() throws {
        resetAutoRefreshDefaults()
        let base = FileManager.default.temporaryDirectory
            .appendingPathComponent("AppModelTests-watcher-\(UUID().uuidString)")
        let left = base.appendingPathComponent("left")
        let right = base.appendingPathComponent("right")
        try FileManager.default.createDirectory(at: left, withIntermediateDirectories: true)
        try FileManager.default.createDirectory(at: right, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: base) }

        let model = AppModel(edition: FreeEdition())
        model.leftFolder = left
        model.rightFolder = right
        // 設定がオフなら左右フォルダが揃っていても監視しない
        XCTAssertFalse(model.watcherActive)

        model.setAutoRefreshEnabled(true)
        XCTAssertTrue(model.watcherActive)

        model.setAutoRefreshEnabled(false)
        XCTAssertFalse(model.watcherActive)
    }

    func testWatcherDetectsFolderChangeIntegration() throws {
        let base = FileManager.default.temporaryDirectory
            .appendingPathComponent("AppModelTests-watcher-\(UUID().uuidString)")
        let left = base.appendingPathComponent("left")
        let right = base.appendingPathComponent("right")
        try FileManager.default.createDirectory(at: left, withIntermediateDirectories: true)
        try FileManager.default.createDirectory(at: right, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: base) }

        let notification = expectation(description: "watcher change notification")
        let watcher = DirectoryWatcher(roots: [left, right], interval: 0.05) {
            notification.fulfill()
        }
        watcher.start()
        defer { watcher.stop() }

        // 基準スナップショットを取る時間を確保してから変更を加える
        Thread.sleep(forTimeInterval: 0.15)
        try "x".data(using: .utf8)!.write(to: left.appendingPathComponent("new.txt"))

        wait(for: [notification], timeout: 3)
    }

    // MARK: - Git ref選択時の監視対象

    func testWatcherStopsWhenBothSidesUseGitRefs() throws {
        resetAutoRefreshDefaults()
        let base = FileManager.default.temporaryDirectory
            .appendingPathComponent("AppModelTests-gitref-\(UUID().uuidString)")
        let left = base.appendingPathComponent("left")
        let right = base.appendingPathComponent("right")
        try FileManager.default.createDirectory(at: left, withIntermediateDirectories: true)
        try FileManager.default.createDirectory(at: right, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: base) }

        let model = AppModel(edition: FreeEdition())
        model.leftFolder = left
        model.rightFolder = right
        model.leftGitRepo = GitRepository(toplevel: left)
        model.rightGitRepo = GitRepository(toplevel: right)
        model.setAutoRefreshEnabled(true)
        XCTAssertTrue(model.watcherActive)

        // 両側をrefへ → 作業ツリー監視対象なし → watcher停止
        model.selectGitRef("main", side: .left)
        model.selectGitRef("HEAD", side: .right)
        XCTAssertFalse(model.watcherActive)

        // 片側だけ作業ツリーへ戻す → 作業ツリー側のみ監視
        model.selectGitRef(nil, side: .left)
        XCTAssertTrue(model.watcherActive)
        // もう片側も戻す → 両方監視
        model.selectGitRef(nil, side: .right)
        XCTAssertTrue(model.watcherActive)

        model.setAutoRefreshEnabled(false)
    }

    func testSelectGitRefReconfiguresWatcherImmediately() throws {
        resetAutoRefreshDefaults()
        let base = FileManager.default.temporaryDirectory
            .appendingPathComponent("AppModelTests-gitref2-\(UUID().uuidString)")
        let left = base.appendingPathComponent("left")
        let right = base.appendingPathComponent("right")
        try FileManager.default.createDirectory(at: left, withIntermediateDirectories: true)
        try FileManager.default.createDirectory(at: right, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: base) }

        let model = AppModel(edition: FreeEdition())
        model.leftFolder = left
        model.rightFolder = right
        model.leftGitRepo = GitRepository(toplevel: left)
        model.setAutoRefreshEnabled(true)
        XCTAssertTrue(model.watcherActive)

        // 左をrefへ → 右(作業ツリー)のみ監視が続く
        model.selectGitRef("main", side: .left)
        XCTAssertTrue(model.watcherActive)

        // 右もrefへ → 監視停止(両側ref比較のため)
        model.rightGitRepo = GitRepository(toplevel: right)
        model.selectGitRef("HEAD", side: .right)
        XCTAssertFalse(model.watcherActive)

        // 右を作業ツリーへ戻す → 右のみ監視
        model.selectGitRef(nil, side: .right)
        XCTAssertTrue(model.watcherActive)

        model.setAutoRefreshEnabled(false)
    }

    // MARK: - 自動再比較の連続上限と一時停止

    func testAutoRefreshPausePolicy() {
        // 上限以下は一時停止しない。上限を超えたら一時停止する(純粋ロジック)
        XCTAssertFalse(
            AppModel.shouldPauseAutoRefresh(afterConsecutiveRuns: AppModel.maxAutoRefreshRuns))
        XCTAssertTrue(
            AppModel.shouldPauseAutoRefresh(afterConsecutiveRuns: AppModel.maxAutoRefreshRuns + 1))
    }

    func testReenableClearsPauseState() {
        resetAutoRefreshDefaults()
        let model = AppModel(edition: FreeEdition())
        // 上限超過で設定される一時停止状態を模擬
        model.autoRefreshPausedReason = "テスト用一時停止"
        model.setAutoRefreshEnabled(true)
        XCTAssertNil(model.autoRefreshPausedReason)
    }

    func testPausedStateStopsWatcherAndStaysOff() throws {
        resetAutoRefreshDefaults()
        let base = FileManager.default.temporaryDirectory
            .appendingPathComponent("AppModelTests-pause-\(UUID().uuidString)")
        let left = base.appendingPathComponent("left")
        let right = base.appendingPathComponent("right")
        try FileManager.default.createDirectory(at: left, withIntermediateDirectories: true)
        try FileManager.default.createDirectory(at: right, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: base) }

        let model = AppModel(edition: FreeEdition())
        model.leftFolder = left
        model.rightFolder = right
        model.setAutoRefreshEnabled(true)
        XCTAssertTrue(model.watcherActive)

        // 一時停止状態ではwatcherが止まり、フォルダ再選択(reconfigureWatcher経路)でも再開しない
        model.autoRefreshPausedReason = "テスト用一時停止"
        model.acceptDroppedFolder(url: left, side: .left)
        XCTAssertFalse(model.watcherActive)
        XCTAssertNotNil(model.autoRefreshPausedReason)

        // トグル再ONで一時停止が解除されwatcherが再開する
        model.setAutoRefreshEnabled(true)
        XCTAssertNil(model.autoRefreshPausedReason)
        XCTAssertTrue(model.watcherActive)

        model.setAutoRefreshEnabled(false)
    }

    func testResultFilterDefaultsToAllEntries() {
        let model = AppModel(edition: FreeEdition())
        model.result = Self.makeFilterResult()

        XCTAssertEqual(
            Self.flattenedIDs(model.displayNodes),
            [
                "docs", "docs/README.md", "docs/Guide.md",
                "src", "src/Added.swift", "src/Removed.swift",
                "errors.log", "same.txt", "empty-left"
            ]
        )
    }

    func testDifferencesFilterRetainsAncestorsAndDropsIdenticalEntries() {
        let model = AppModel(edition: FreeEdition())
        model.result = Self.makeFilterResult()
        model.resultFilter = .differences

        XCTAssertEqual(
            Self.flattenedIDs(model.displayNodes),
            [
                "docs", "docs/Guide.md",
                "src", "src/Added.swift", "src/Removed.swift",
                "errors.log", "empty-left"
            ]
        )
    }

    func testStatusFilterKeepsMatchingEmptyDirectoryAndAncestors() {
        let model = AppModel(edition: FreeEdition())
        model.result = Self.makeFilterResult()
        model.resultFilter = .leftOnly

        XCTAssertEqual(
            Self.flattenedIDs(model.displayNodes),
            ["src", "src/Added.swift", "empty-left"]
        )
    }

    func testResultSearchMatchesCaseInsensitiveRelativePath() {
        let model = AppModel(edition: FreeEdition())
        model.result = Self.makeFilterResult()
        model.resultSearchText = "DOCS/readme"

        XCTAssertEqual(
            Self.flattenedIDs(model.displayNodes),
            ["docs", "docs/README.md"]
        )
    }

    func testResultSearchCombinesWithStatusFilter() {
        let model = AppModel(edition: FreeEdition())
        model.result = Self.makeFilterResult()
        model.resultFilter = .rightOnly
        model.resultSearchText = " src "

        XCTAssertEqual(
            Self.flattenedIDs(model.displayNodes),
            ["src", "src/Removed.swift"]
        )
    }

    func testEntryURLUsesRequestedSide() {
        let model = AppModel(edition: FreeEdition())
        let left = URL(fileURLWithPath: "/tmp/left/file.txt")
        let right = URL(fileURLWithPath: "/tmp/right/file.txt")
        let entry = Self.makeActionEntry(leftURL: left, rightURL: right)

        XCTAssertEqual(model.entryURL(for: entry, side: .left), left)
        XCTAssertEqual(model.entryURL(for: entry, side: .right), right)
    }

    func testCopyEntryPathUsesProvidedPasteboardAndPreservesItForMissingSide() {
        let model = AppModel(edition: FreeEdition())
        let pasteboard = NSPasteboard.withUniqueName()
        defer { pasteboard.releaseGlobally() }
        let left = URL(fileURLWithPath: "/tmp/left/folder/file.txt")
        let entry = Self.makeActionEntry(leftURL: left, rightURL: nil)

        XCTAssertTrue(model.copyEntryPath(entry, side: .left, to: pasteboard))
        XCTAssertEqual(pasteboard.string(forType: .string), left.path)

        XCTAssertFalse(model.copyEntryPath(entry, side: .right, to: pasteboard))
        XCTAssertEqual(pasteboard.string(forType: .string), left.path)
    }

    // MARK: - エディション

    /// 拡張点を一切実装しない最小のエディション(既定実装の確認用)
    private final class MinimalEdition: DiffLoupeEdition {
        let name = "Minimal"
        let maxFolderEntries: Int? = nil
    }

    func testFreeEditionLimitsFolderEntriesTo2000() {
        let edition = FreeEdition()
        XCTAssertEqual(edition.name, "Free")
        XCTAssertEqual(edition.maxFolderEntries, 2000)
        XCTAssertEqual(edition.maxFolderEntries, FolderComparator.Options.freeMaxEntries)
    }

    func testEditionExtensionPointsDefaultToNil() {
        let model = AppModel(edition: FreeEdition())
        let entry = Self.makeActionEntry(
            leftURL: URL(fileURLWithPath: "/tmp/left/file.txt"),
            rightURL: URL(fileURLWithPath: "/tmp/right/file.txt")
        )
        let plan = SyncPlan.make(from: Self.makeResult(), direction: .leftToRight)

        for edition: DiffLoupeEdition in [FreeEdition(), MinimalEdition()] {
            XCTAssertNil(edition.textDetailView(model: model, entry: entry), edition.name)
            XCTAssertNil(edition.syncPlanActions(model: model, plan: plan), edition.name)
            XCTAssertNil(edition.threeWayView(model: model), edition.name)
            XCTAssertNil(edition.settingsSection(model: model), edition.name)
        }
    }

    func testAppModelKeepsGivenEdition() {
        let edition = MinimalEdition()
        let model = AppModel(edition: edition)
        XCTAssertTrue(model.edition === edition)
        XCTAssertNil(model.edition.maxFolderEntries)
    }
    // MARK: - 未保存の編集の確認フック

    /// 確認フックの呼び出し回数と応答を制御できるテスト用エディション
    private final class GuardEdition: DiffLoupeEdition {
        let name = "Guard"
        let maxFolderEntries: Int? = nil
        var allow = true
        var unsaved = false
        var confirmCalls = 0
        var hasUnsavedChanges: Bool { unsaved }
        func confirmDiscardUnsavedChanges() -> Bool {
            confirmCalls += 1
            return allow
        }
    }

    private func makeFolders(prefix: String) throws -> (base: URL, left: URL, right: URL) {
        let base = FileManager.default.temporaryDirectory
            .appendingPathComponent("AppModelTests-\(prefix)-\(UUID().uuidString)")
        let left = base.appendingPathComponent("left")
        let right = base.appendingPathComponent("right")
        try FileManager.default.createDirectory(at: left, withIntermediateDirectories: true)
        try FileManager.default.createDirectory(at: right, withIntermediateDirectories: true)
        return (base, left, right)
    }

    private func waitUntil(timeout: TimeInterval = 5, _ condition: () -> Bool) async -> Bool {
        let deadline = Date().addingTimeInterval(timeout)
        while Date() < deadline {
            if condition() { return true }
            try? await Task.sleep(nanoseconds: 50_000_000)
        }
        return condition()
    }

    func testFreeEditionNeverBlocksAndHasNoUnsavedChanges() {
        let edition = FreeEdition()
        XCTAssertTrue(edition.confirmDiscardUnsavedChanges())
        XCTAssertFalse(edition.hasUnsavedChanges)
    }

    func testSwapSidesCallsConfirmHookAndAbortsWhenDenied() {
        let edition = GuardEdition()
        let model = AppModel(edition: edition)
        configurePair(model)
        let originalLeft = model.leftFolder

        edition.allow = false
        model.swapSides()
        XCTAssertEqual(edition.confirmCalls, 1)
        XCTAssertEqual(model.leftFolder, originalLeft)

        edition.allow = true
        model.swapSides()
        XCTAssertEqual(edition.confirmCalls, 2)
        XCTAssertNotEqual(model.leftFolder, originalLeft)
    }

    func testRestoreComparisonCallsConfirmHookAndAbortsWhenDenied() throws {
        let edition = GuardEdition()
        let model = AppModel(edition: edition)
        configurePair(model)
        let dirs = try makeFolders(prefix: "guard-restore")
        defer { try? FileManager.default.removeItem(at: dirs.base) }
        let item = RecentComparison(leftPath: dirs.left.path, rightPath: dirs.right.path,
                                    leftGitRef: nil, rightGitRef: nil, date: Date())
        let originalLeft = model.leftFolder

        edition.allow = false
        model.restoreComparison(item)
        XCTAssertEqual(edition.confirmCalls, 1)
        XCTAssertEqual(model.leftFolder, originalLeft)

        edition.allow = true
        model.restoreComparison(item)
        XCTAssertEqual(edition.confirmCalls, 2)
        XCTAssertEqual(model.leftFolder?.path, dirs.left.path)
    }

    func testAcceptDroppedFolderCallsConfirmHookAndAbortsWhenDenied() throws {
        let edition = GuardEdition()
        let model = AppModel(edition: edition)
        let dirs = try makeFolders(prefix: "guard-drop")
        defer { try? FileManager.default.removeItem(at: dirs.base) }

        edition.allow = false
        model.acceptDroppedFolder(url: dirs.left, side: .left)
        XCTAssertEqual(edition.confirmCalls, 1)
        XCTAssertNil(model.leftFolder)

        edition.allow = true
        model.acceptDroppedFolder(url: dirs.left, side: .left)
        XCTAssertEqual(edition.confirmCalls, 2)
        XCTAssertNotNil(model.leftFolder)
    }

    func testStartCompareCallsConfirmHookAndAbortsWhenDenied() async throws {
        let edition = GuardEdition()
        let model = AppModel(edition: edition)
        let dirs = try makeFolders(prefix: "guard-compare")
        defer { try? FileManager.default.removeItem(at: dirs.base) }
        model.acceptDroppedFolder(url: dirs.left, side: .left)
        model.acceptDroppedFolder(url: dirs.right, side: .right)
        edition.confirmCalls = 0

        edition.allow = false
        model.startCompare()
        XCTAssertEqual(edition.confirmCalls, 1)
        XCTAssertFalse(model.phase.isRunning)
        XCTAssertNil(model.result)

        edition.allow = true
        model.startCompare()
        XCTAssertEqual(edition.confirmCalls, 2)
        let finished = await waitUntil { model.result != nil }
        XCTAssertTrue(finished)
    }

    func testRecompareRestoresSelectionWhenPathStillExists() async throws {
        let model = AppModel(edition: FreeEdition())
        let dirs = try makeFolders(prefix: "recompare")
        defer { try? FileManager.default.removeItem(at: dirs.base) }
        try "a".write(to: dirs.left.appendingPathComponent("a.txt"), atomically: true, encoding: .utf8)
        try "b".write(to: dirs.right.appendingPathComponent("a.txt"), atomically: true, encoding: .utf8)
        model.acceptDroppedFolder(url: dirs.left, side: .left)
        model.acceptDroppedFolder(url: dirs.right, side: .right)
        model.startCompare()
        let first = await waitUntil { model.result != nil }
        XCTAssertTrue(first)

        model.selection = "a.txt"
        model.recompare(preservingSelection: true)
        let second = await waitUntil { model.result != nil && !model.phase.isRunning }
        XCTAssertTrue(second)
        XCTAssertEqual(model.selection, "a.txt")

        // 存在しなくなったパスは復元しない
        model.selection = "a.txt"
        try FileManager.default.removeItem(at: dirs.left.appendingPathComponent("a.txt"))
        try FileManager.default.removeItem(at: dirs.right.appendingPathComponent("a.txt"))
        model.recompare(preservingSelection: true)
        let third = await waitUntil { model.result != nil && !model.phase.isRunning }
        XCTAssertTrue(third)
        XCTAssertNil(model.selection)

        // preservingSelection: false なら従来どおり選択はクリアされる
        model.selection = "x"
        model.recompare(preservingSelection: false)
        XCTAssertNil(model.selection)
    }

    func testAutoRecompareIsDeferredWhileUnsavedAndResumesAfterwards() async throws {
        resetAutoRefreshDefaults()
        defer { resetAutoRefreshDefaults() }
        let edition = GuardEdition()
        let model = AppModel(edition: edition)
        let dirs = try makeFolders(prefix: "guard-auto")
        defer { try? FileManager.default.removeItem(at: dirs.base) }
        model.acceptDroppedFolder(url: dirs.left, side: .left)
        model.acceptDroppedFolder(url: dirs.right, side: .right)
        edition.confirmCalls = 0
        edition.unsaved = true
        model.setAutoRefreshEnabled(true)
        defer { model.setAutoRefreshEnabled(false) }

        try await Task.sleep(nanoseconds: 300_000_000)
        try "x".write(to: dirs.left.appendingPathComponent("new.txt"), atomically: true, encoding: .utf8)

        let deferred = await waitUntil(timeout: 8) { model.autoRefreshDeferred }
        XCTAssertTrue(deferred)
        XCTAssertNil(model.result)
        XCTAssertFalse(model.phase.isRunning)

        edition.unsaved = false
        model.resumeDeferredAutoRefresh()
        let resumed = await waitUntil { model.result != nil }
        XCTAssertTrue(resumed)
        XCTAssertFalse(model.autoRefreshDeferred)
        // 自動再比較は確認ダイアログを出さない
        XCTAssertEqual(edition.confirmCalls, 0)
    }
}
