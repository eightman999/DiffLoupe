// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: ArchiveSnapshot(zipの一時展開ファサード)のフォーカステスト。
// /usr/bin/zip でテスト内にzipを作り、実体化 → 中身一致 → cleanupで消えること、
// zip同士をFolderComparatorで比較して相違・片側のみが出ること、壊れたzipのエラーを検証する。

import XCTest
@testable import DiffLoupeCore

final class ArchiveSnapshotTests: XCTestCase {

    private var base: URL!

    override func setUpWithError() throws {
        base = FileManager.default.temporaryDirectory
            .appendingPathComponent("DiffLoupeArchiveTests-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: base, withIntermediateDirectories: true)
    }

    override func tearDownWithError() throws {
        try? FileManager.default.removeItem(at: base)
    }

    /// files(相対パス → 内容)から zip を作って URL を返す。
    private func makeZip(named name: String, files: [String: String]) throws -> URL {
        let source = base.appendingPathComponent("src-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: source, withIntermediateDirectories: true)
        for (relative, text) in files {
            let url = source.appendingPathComponent(relative)
            try FileManager.default.createDirectory(
                at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
            try Data(text.utf8).write(to: url)
        }
        let zipURL = base.appendingPathComponent(name)
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/zip")
        process.currentDirectoryURL = source
        process.arguments = ["-q", "-r", "-X", zipURL.path, "."]
        try process.run()
        process.waitUntilExit()
        XCTAssertEqual(process.terminationStatus, 0, "zip の作成に失敗")
        return zipURL
    }

    func testIsArchiveByExtension() {
        XCTAssertTrue(ArchiveSnapshot.isArchive(url: URL(fileURLWithPath: "/tmp/a.zip")))
        XCTAssertTrue(ArchiveSnapshot.isArchive(url: URL(fileURLWithPath: "/tmp/a.ZIP")))
        XCTAssertFalse(ArchiveSnapshot.isArchive(url: URL(fileURLWithPath: "/tmp/a.txt")))
        XCTAssertFalse(ArchiveSnapshot.isArchive(url: URL(fileURLWithPath: "/tmp/zip")))
    }

    func testMaterializeMatchesContentAndCleanupRemoves() throws {
        let zip = try makeZip(named: "a.zip", files: [
            "hello.txt": "こんにちは\n",
            "dir/nested.txt": "nested",
        ])
        let root = try ArchiveSnapshot.materialize(url: zip)
        defer { ArchiveSnapshot.cleanup(root) }

        XCTAssertEqual(try String(contentsOf: root.appendingPathComponent("hello.txt")), "こんにちは\n")
        XCTAssertEqual(try String(contentsOf: root.appendingPathComponent("dir/nested.txt")), "nested")
        // 元のzipは変更されない
        XCTAssertTrue(FileManager.default.fileExists(atPath: zip.path))

        ArchiveSnapshot.cleanup(root)
        XCTAssertFalse(FileManager.default.fileExists(atPath: root.path))
    }

    func testComparingTwoArchivesReportsDifferentAndOneSided() async throws {
        let left = try makeZip(named: "left.zip", files: ["same.txt": "same", "a.txt": "AAA", "only-left.txt": "L"])
        let right = try makeZip(named: "right.zip", files: ["same.txt": "same", "a.txt": "BBB", "only-right.txt": "R"])
        let leftRoot = try ArchiveSnapshot.materialize(url: left)
        let rightRoot = try ArchiveSnapshot.materialize(url: right)
        defer {
            ArchiveSnapshot.cleanup(leftRoot)
            ArchiveSnapshot.cleanup(rightRoot)
        }

        let result = try await FolderComparator().compare(left: leftRoot, right: rightRoot)
        XCTAssertEqual(result.entriesByPath["same.txt"]?.status, .identical)
        XCTAssertEqual(result.entriesByPath["a.txt"]?.status, .different)
        XCTAssertEqual(result.entriesByPath["only-left.txt"]?.status, .leftOnly)
        XCTAssertEqual(result.entriesByPath["only-right.txt"]?.status, .rightOnly)
    }

    func testBrokenArchiveThrowsAndLeavesNoTempDirectory() throws {
        let broken = base.appendingPathComponent("broken.zip")
        try Data("これはzipではありません".utf8).write(to: broken)
        let before = Self.tempArchiveDirectoryCount()
        XCTAssertThrowsError(try ArchiveSnapshot.materialize(url: broken)) { error in
            XCTAssertTrue(error is ArchiveSnapshot.ArchiveError)
            XCTAssertFalse((error as? ArchiveSnapshot.ArchiveError)?.message.isEmpty ?? true)
        }
        XCTAssertEqual(Self.tempArchiveDirectoryCount(), before)
    }

    /// 一時ディレクトリ直下の展開先(DiffLoupe-zip-*)の数。失敗時に残骸がないことの確認用。
    private static func tempArchiveDirectoryCount() -> Int {
        let names = (try? FileManager.default.contentsOfDirectory(
            atPath: FileManager.default.temporaryDirectory.path)) ?? []
        return names.filter { $0.hasPrefix("DiffLoupe-zip-") }.count
    }
}
