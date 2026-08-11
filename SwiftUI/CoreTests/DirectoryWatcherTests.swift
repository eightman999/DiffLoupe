// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: DirectoryWatcher/DirectorySnapshotのフォーカステスト。
// スナップショットの同値・追加/更新/削除検知は純粋なscan関数で安定検証し、
// タイマー駆動の検知とstop後の通知なしは短縮間隔+一時フォルダで最小限検証する。
// 一時ディレクトリはFileManager.default.temporaryDirectory配下で作成し、teardownで削除する。

import XCTest
@testable import DiffLoupeCore

final class DirectoryWatcherTests: XCTestCase {

    private func makeTempDir() throws -> URL {
        let dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("DirectoryWatcherTests-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        addTeardownBlock { try? FileManager.default.removeItem(at: dir) }
        return dir
    }

    private func write(_ text: String, to dir: URL, _ relative: String) throws {
        let url = dir.appendingPathComponent(relative)
        try FileManager.default.createDirectory(
            at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
        try text.data(using: .utf8)!.write(to: url)
    }

    // MARK: - スナップショット同値・種別

    func testScanSameTreeProducesEqualSnapshots() throws {
        let dir = try makeTempDir()
        try write("AAA", to: dir, "sub/nested.txt")
        try write("BBB", to: dir, "top.txt")

        let first = try DirectorySnapshot.scan(root: dir)
        let second = try DirectorySnapshot.scan(root: dir)

        XCTAssertEqual(first, second)
        XCTAssertEqual(first.entries.count, 3)
        XCTAssertTrue(first.entries.contains { $0.relativePath == "top.txt" && !$0.isDirectory })
        XCTAssertTrue(first.entries.contains { $0.relativePath == "sub" && $0.isDirectory })
        XCTAssertTrue(first.entries.contains { $0.relativePath == "sub/nested.txt" && !$0.isDirectory })
    }

    func testScanIsOrderIndependent() throws {
        let dir = try makeTempDir()
        try write("AAA", to: dir, "b.txt")
        try write("BBB", to: dir, "a.txt")

        // 走査順に依存しない(相対パス順ソート)ことを確認するため、2回のscanが同値であること。
        let first = try DirectorySnapshot.scan(root: dir)
        let second = try DirectorySnapshot.scan(root: dir)
        XCTAssertEqual(first.entries.map(\.relativePath), ["a.txt", "b.txt"])
        XCTAssertEqual(first, second)
    }

    // MARK: - 追加・削除・更新の検知(純粋関数)

    func testScanDetectsFileAddition() throws {
        let dir = try makeTempDir()
        try write("AAA", to: dir, "top.txt")
        let before = try DirectorySnapshot.scan(root: dir)

        try write("BBB", to: dir, "added.txt")
        let after = try DirectorySnapshot.scan(root: dir)

        XCTAssertNotEqual(before, after)
        XCTAssertTrue(after.entries.contains { $0.relativePath == "added.txt" })
    }

    func testScanDetectsFileRemoval() throws {
        let dir = try makeTempDir()
        try write("AAA", to: dir, "top.txt")
        try write("BBB", to: dir, "gone.txt")
        let before = try DirectorySnapshot.scan(root: dir)

        try FileManager.default.removeItem(at: dir.appendingPathComponent("gone.txt"))
        let after = try DirectorySnapshot.scan(root: dir)

        XCTAssertNotEqual(before, after)
        XCTAssertFalse(after.entries.contains { $0.relativePath == "gone.txt" })
    }

    func testScanDetectsSizeChange() throws {
        let dir = try makeTempDir()
        try write("AAA", to: dir, "top.txt")
        let before = try DirectorySnapshot.scan(root: dir)

        try write("AAAA", to: dir, "top.txt")
        let after = try DirectorySnapshot.scan(root: dir)

        XCTAssertNotEqual(before, after)
    }

    func testScanDetectsModificationDateChange() throws {
        let dir = try makeTempDir()
        try write("AAA", to: dir, "top.txt")
        let before = try DirectorySnapshot.scan(root: dir)

        // サイズを変えずにmtimeだけ書き換える(内容変更の検知を確実に検証)
        let future = Date(timeIntervalSinceNow: 60)
        try FileManager.default.setAttributes(
            [.modificationDate: future], ofItemAtPath: dir.appendingPathComponent("top.txt").path)
        let after = try DirectorySnapshot.scan(root: dir)

        XCTAssertNotEqual(before, after)
    }

    func testScanDetectsDirectoryAddition() throws {
        let dir = try makeTempDir()
        let before = try DirectorySnapshot.scan(root: dir)

        try write("x", to: dir, "newdir/inner.txt")
        let after = try DirectorySnapshot.scan(root: dir)

        XCTAssertNotEqual(before, after)
        XCTAssertTrue(after.entries.contains { $0.relativePath == "newdir" && $0.isDirectory })
        XCTAssertTrue(after.entries.contains { $0.relativePath == "newdir/inner.txt" })
    }

    // MARK: - シンボリックリンク

    func testScanDoesNotFollowDirectorySymlinks() throws {
        let dir = try makeTempDir()
        let target = try makeTempDir()
        try write("x", to: target, "inside.txt")
        try FileManager.default.createSymbolicLink(
            at: dir.appendingPathComponent("linkdir"), withDestinationURL: target)

        let snapshot = try DirectorySnapshot.scan(root: dir)

        XCTAssertTrue(snapshot.entries.contains { $0.relativePath == "linkdir" && $0.isSymbolicLink })
        XCTAssertFalse(snapshot.entries.contains { $0.relativePath == "linkdir/inside.txt" })
    }

    func testScanThrowsForNonDirectoryRoot() {
        let dir = try! makeTempDir()
        let file = dir.appendingPathComponent("f.txt")
        try! "x".data(using: .utf8)!.write(to: file)

        XCTAssertThrowsError(try DirectorySnapshot.scan(root: file)) { error in
            guard case DirectoryWatcherError.rootIsNotDirectory = error else {
                return XCTFail("期待しないエラー: \(error)")
            }
        }
    }

    // MARK: - 既定除外(excludedNames)

    func testScanExcludesNamedEntriesRecursively() throws {
        let dir = try makeTempDir()
        try write("x", to: dir, ".git/config")
        try write("x", to: dir, ".DS_Store")
        try write("x", to: dir, "node_modules/pkg/index.js")
        try write("x", to: dir, "keep.txt")
        try write("x", to: dir, "sub/.DS_Store")
        try write("x", to: dir, "sub/keep.txt")

        let snapshot = try DirectorySnapshot.scan(
            root: dir, excludedNames: [".git", ".DS_Store", "node_modules"])

        XCTAssertTrue(snapshot.entries.contains { $0.relativePath == "keep.txt" })
        XCTAssertTrue(snapshot.entries.contains { $0.relativePath == "sub" && $0.isDirectory })
        XCTAssertTrue(snapshot.entries.contains { $0.relativePath == "sub/keep.txt" })
        // 除外ディレクトリは自身も子孫も記録されない
        XCTAssertFalse(snapshot.entries.contains { $0.relativePath == ".git" })
        XCTAssertFalse(snapshot.entries.contains { $0.relativePath == ".git/config" })
        XCTAssertFalse(snapshot.entries.contains { $0.relativePath == "node_modules" })
        XCTAssertFalse(snapshot.entries.contains { $0.relativePath == "node_modules/pkg/index.js" })
        // 除外ファイルは記録されない(ネストしていても)
        XCTAssertFalse(snapshot.entries.contains { $0.relativePath == ".DS_Store" })
        XCTAssertFalse(snapshot.entries.contains { $0.relativePath == "sub/.DS_Store" })
    }

    func testWatcherIgnoresChangesInsideExcludedDirectory() throws {
        let left = try makeTempDir()
        let right = try makeTempDir()
        try write("x", to: left, ".git/config")
        let unexpected = expectation(description: "excluded change must not notify")
        unexpected.isInverted = true
        let watcher = DirectoryWatcher(
            roots: [left, right], interval: 0.05,
            excludedNames: [".git", ".DS_Store", "node_modules"]) {
                unexpected.fulfill()
            }
        watcher.start()
        defer { watcher.stop() }

        // baseline完了を待ってから、除外対象(.git)の内部を変更する
        Thread.sleep(forTimeInterval: 0.15)
        try write("y", to: left, ".git/config")

        wait(for: [unexpected], timeout: 1)
    }

    // MARK: - stop → start の再利用と非同期baselineの安全性

    func testStopThenStartReuseIsSafe() throws {
        let left = try makeTempDir()
        let right = try makeTempDir()
        let notification = expectation(description: "change after restart")
        let watcher = DirectoryWatcher(roots: [left, right], interval: 0.05) {
            notification.fulfill()
        }
        // baseline完了前にstopしても安全(古いrunのbaselineは破棄される)
        watcher.start()
        watcher.stop()
        // 再利用: 新しいrunとしてbaselineを取り直して監視を再開する
        watcher.start()
        defer { watcher.stop() }

        Thread.sleep(forTimeInterval: 0.15)
        try write("x", to: left, "added.txt")

        wait(for: [notification], timeout: 3)
    }

    func testStopDuringBaselineIsSafeAndNeverNotifies() throws {
        let left = try makeTempDir()
        let right = try makeTempDir()
        let unexpected = expectation(description: "no notify after stop during baseline")
        unexpected.isInverted = true
        let watcher = DirectoryWatcher(roots: [left, right], interval: 0.05) {
            unexpected.fulfill()
        }
        // start直後にstop。進行中の非同期baselineは新しいrunへ書き込まない
        watcher.start()
        watcher.stop()
        try write("x", to: left, "added.txt")

        wait(for: [unexpected], timeout: 1)
    }

    // MARK: - watcherのタイマー駆動検知

    func testWatcherDetectsChangeAndNotifies() throws {
        let left = try makeTempDir()
        let right = try makeTempDir()
        let notification = expectation(description: "change notification")
        let watcher = DirectoryWatcher(roots: [left, right], interval: 0.05) {
            notification.fulfill()
        }
        watcher.start()
        defer { watcher.stop() }

        // 基準スナップショットを取る時間を確保してから変更を加える
        Thread.sleep(forTimeInterval: 0.15)
        try write("x", to: left, "added.txt")

        wait(for: [notification], timeout: 3)
    }

    func testWatcherNotifiesAfterStop() throws {
        let left = try makeTempDir()
        let right = try makeTempDir()
        let noNotification = expectation(description: "should not notify after stop")
        noNotification.isInverted = true
        let watcher = DirectoryWatcher(roots: [left, right], interval: 0.05) {
            noNotification.fulfill()
        }
        watcher.start()

        Thread.sleep(forTimeInterval: 0.1)
        watcher.stop()
        try write("x", to: left, "after_stop.txt")

        // stop後に1秒待っても通知が来なければパス(inverted expectation)
        wait(for: [noNotification], timeout: 1)
    }
}
