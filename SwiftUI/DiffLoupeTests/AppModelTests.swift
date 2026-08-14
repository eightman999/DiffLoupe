// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: AppModelの左右交換(swapSides)と履歴復元(restoreComparison)を検証する。
// @MainActor上で実行する純粋ロジックのテスト(フォルダ選択UIは対象外)

import XCTest
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
        let model = AppModel()
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
        let model = AppModel()
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
        let model = AppModel()
        configurePair(model)
        model.phase = .scanning(found: 1)

        model.swapSides()

        XCTAssertEqual(model.leftFolder?.path, "/tmp/left")
        XCTAssertEqual(model.rightFolder?.path, "/tmp/right")
        XCTAssertEqual(model.leftGitRef, "main")
        XCTAssertEqual(model.rightGitRef, "HEAD")
    }

    func testSwapSidesSwapsNilRefsSafely() {
        let model = AppModel()
        configurePair(model)
        model.leftGitRef = nil
        model.rightGitRef = nil

        model.swapSides()

        XCTAssertNil(model.leftGitRef)
        XCTAssertNil(model.rightGitRef)
    }

    func testSwapSidesTwiceRestoresOriginalOrder() {
        let model = AppModel()
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
        let model = AppModel()
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
        let model = AppModel()
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
        let model = AppModel()
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
        let model = AppModel()
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

        let model = AppModel()
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

        let model = AppModel()
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

        let model = AppModel()
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
        let model = AppModel()
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

        let model = AppModel()
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
        let model = AppModel()
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
        let model = AppModel()
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
        let model = AppModel()
        model.result = Self.makeFilterResult()
        model.resultFilter = .leftOnly

        XCTAssertEqual(
            Self.flattenedIDs(model.displayNodes),
            ["src", "src/Added.swift", "empty-left"]
        )
    }

    func testResultSearchMatchesCaseInsensitiveRelativePath() {
        let model = AppModel()
        model.result = Self.makeFilterResult()
        model.resultSearchText = "DOCS/readme"

        XCTAssertEqual(
            Self.flattenedIDs(model.displayNodes),
            ["docs", "docs/README.md"]
        )
    }

    func testResultSearchCombinesWithStatusFilter() {
        let model = AppModel()
        model.result = Self.makeFilterResult()
        model.resultFilter = .rightOnly
        model.resultSearchText = " src "

        XCTAssertEqual(
            Self.flattenedIDs(model.displayNodes),
            ["src", "src/Removed.swift"]
        )
    }
}
