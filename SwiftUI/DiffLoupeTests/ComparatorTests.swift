// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: FolderComparatorの分類結果をTestFixturesと一時ディレクトリで検証する

import XCTest

final class ComparatorTests: XCTestCase {

    /// リポジトリ同梱の TestFixtures/(SwiftUI/TestFixtures)
    private static var fixturesURL: URL {
        URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()    // DiffLoupeTests/
            .deletingLastPathComponent()    // SwiftUI/
            .appendingPathComponent("TestFixtures")
    }

    private func compareFixtures(
        options: FolderComparator.Options = .init()
    ) async throws -> ComparisonResult {
        try await FolderComparator().compare(
            left: Self.fixturesURL.appendingPathComponent("left"),
            right: Self.fixturesURL.appendingPathComponent("right"),
            options: options
        )
    }

    // MARK: - 同梱フィクスチャによる分類検証(受け入れ条件)

    func testFixtureClassification() async throws {
        let result = try await compareFixtures()
        let byPath = result.entriesByPath

        // 同一
        XCTAssertEqual(byPath["same.txt"]?.status, .identical)
        XCTAssertEqual(byPath["binary_same.bin"]?.status, .identical)
        XCTAssertEqual(byPath["sub/nested_same.txt"]?.status, .identical)

        // 内容相違(同サイズ → ハッシュ比較で検出されること)
        XCTAssertEqual(byPath["changed_same_size.txt"]?.status, .different)
        XCTAssertEqual(byPath["binary_diff.bin"]?.status, .different)
        // 内容相違(サイズ不一致 → ハッシュ省略で確定)
        XCTAssertEqual(byPath["changed_size.txt"]?.status, .different)
        XCTAssertEqual(byPath["sub/nested_changed.txt"]?.status, .different)

        // 片側のみ
        XCTAssertEqual(byPath["only_left.txt"]?.status, .leftOnly)
        XCTAssertEqual(byPath["only_right.txt"]?.status, .rightOnly)
        XCTAssertEqual(byPath["sub/deep/only_left_deep.txt"]?.status, .leftOnly)
        XCTAssertEqual(byPath["sub/deep2/only_right_deep.txt"]?.status, .rightOnly)

        // ディレクトリの集約: 差分を含むsubはdifferent、片側のみのディレクトリはleftOnly/rightOnly
        XCTAssertEqual(byPath["sub"]?.status, .different)
        XCTAssertEqual(byPath["sub/deep"]?.status, .leftOnly)
        XCTAssertEqual(byPath["sub/deep2"]?.status, .rightOnly)

        // 種別不一致(左ファイル・右ディレクトリ)
        XCTAssertEqual(byPath["typemix"]?.status, .different)
        XCTAssertEqual(byPath["typemix/inner.txt"]?.status, .rightOnly)

        // 隠しファイルは既定で比較対象(SPEC.md §2.2)
        XCTAssertEqual(byPath[".hidden.txt"]?.status, .different)

        // サマリ(ファイルのみ集計)
        XCTAssertEqual(result.summary.identicalFiles, 3)
        XCTAssertEqual(result.summary.differentFiles, 5)
        XCTAssertEqual(result.summary.leftOnlyFiles, 2)
        XCTAssertEqual(result.summary.rightOnlyFiles, 3)
        XCTAssertEqual(result.summary.errorFiles, 0)
    }

    func testTreeStructure() async throws {
        let result = try await compareFixtures()
        let root = result.rootNode

        XCTAssertEqual(root.status, .different)
        let children = try XCTUnwrap(root.children)

        // ディレクトリ優先・名前昇順
        let subIndex = try XCTUnwrap(children.firstIndex { $0.name == "sub" })
        let firstFileIndex = try XCTUnwrap(children.firstIndex { $0.children == nil })
        XCTAssertLessThan(subIndex, firstFileIndex)

        let sub = children[subIndex]
        XCTAssertEqual(sub.status, .different)
        let deep = try XCTUnwrap(sub.children?.first { $0.name == "deep" })
        XCTAssertEqual(deep.status, .leftOnly)
        XCTAssertEqual(deep.children?.count, 1)
    }

    // MARK: - 一時ディレクトリによる検証

    private func makeTempPair() throws -> (left: URL, right: URL, cleanup: () -> Void) {
        let base = FileManager.default.temporaryDirectory
            .appendingPathComponent("DiffLoupeTests-\(UUID().uuidString)")
        let left = base.appendingPathComponent("left")
        let right = base.appendingPathComponent("right")
        try FileManager.default.createDirectory(at: left, withIntermediateDirectories: true)
        try FileManager.default.createDirectory(at: right, withIntermediateDirectories: true)
        return (left, right, { try? FileManager.default.removeItem(at: base) })
    }

    private func write(_ text: String, to dir: URL, _ relative: String) throws {
        let url = dir.appendingPathComponent(relative)
        try FileManager.default.createDirectory(
            at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
        try text.data(using: .utf8)!.write(to: url)
    }

    func testDefaultExcludes() async throws {
        let (left, right, cleanup) = try makeTempPair()
        defer { cleanup() }

        try write("ref", to: left, ".git/config")
        try write("junk", to: left, ".DS_Store")
        try write("module", to: left, "node_modules/pkg/index.js")
        try write("keep", to: left, "keep.txt")

        // 既定: 除外オン
        let excluded = try await FolderComparator().compare(left: left, right: right)
        XCTAssertEqual(excluded.entries.map(\.relativePath), ["keep.txt"])
        XCTAssertEqual(excluded.entriesByPath["keep.txt"]?.status, .leftOnly)

        // トグルオフ: すべて対象になる
        let included = try await FolderComparator().compare(
            left: left, right: right,
            options: .init(useDefaultExcludes: false))
        XCTAssertNotNil(included.entriesByPath[".git"])
        XCTAssertNotNil(included.entriesByPath[".git/config"])
        XCTAssertNotNil(included.entriesByPath[".DS_Store"])
        XCTAssertNotNil(included.entriesByPath["node_modules/pkg/index.js"])
    }

    func testEmptyDirectories() async throws {
        let (left, right, cleanup) = try makeTempPair()
        defer { cleanup() }

        let fm = FileManager.default
        try fm.createDirectory(at: left.appendingPathComponent("both_empty"), withIntermediateDirectories: true)
        try fm.createDirectory(at: right.appendingPathComponent("both_empty"), withIntermediateDirectories: true)
        try fm.createDirectory(at: left.appendingPathComponent("left_only_dir"), withIntermediateDirectories: true)

        let result = try await FolderComparator().compare(left: left, right: right)
        XCTAssertEqual(result.entriesByPath["both_empty"]?.status, .identical)
        XCTAssertEqual(result.entriesByPath["left_only_dir"]?.status, .leftOnly)
    }

    func testSymlinkHandling() async throws {
        let (left, right, cleanup) = try makeTempPair()
        defer { cleanup() }
        let fm = FileManager.default

        // ディレクトリへのシンボリックリンク: 辿らず記録もしない
        try write("real content", to: left, "real_dir/file.txt")
        try fm.createSymbolicLink(
            at: left.appendingPathComponent("dir_link"),
            withDestinationURL: left.appendingPathComponent("real_dir"))

        // ファイルへのシンボリックリンク: リンク先の内容で比較
        try write("link target", to: left, "target.txt")
        try fm.createSymbolicLink(
            at: left.appendingPathComponent("file_link.txt"),
            withDestinationURL: left.appendingPathComponent("target.txt"))
        try write("link target", to: right, "file_link.txt")

        // リンク切れ(両側): 読み込み不可 → error
        try fm.createSymbolicLink(
            at: left.appendingPathComponent("broken.txt"),
            withDestinationURL: left.appendingPathComponent("nonexistent"))
        try fm.createSymbolicLink(
            at: right.appendingPathComponent("broken.txt"),
            withDestinationURL: right.appendingPathComponent("nonexistent"))

        let result = try await FolderComparator().compare(left: left, right: right)
        XCTAssertNil(result.entriesByPath["dir_link"])
        XCTAssertNil(result.entriesByPath["dir_link/file.txt"])
        XCTAssertEqual(result.entriesByPath["file_link.txt"]?.status, .identical)
        XCTAssertEqual(result.entriesByPath["broken.txt"]?.status, .error)
        XCTAssertEqual(result.summary.errorFiles, 1)
    }

    func testCancellation() async throws {
        let (left, right, cleanup) = try makeTempPair()
        defer { cleanup() }

        // 同サイズのファイルを多数用意してハッシュ段階まで進む比較を作る
        let payload = String(repeating: "x", count: 4096)
        for index in 0..<200 {
            try write(payload, to: left, "file\(index).txt")
            try write(payload, to: right, "file\(index).txt")
        }

        let task = Task {
            try await FolderComparator().compare(left: left, right: right)
        }
        task.cancel()

        do {
            _ = try await task.value
            XCTFail("キャンセルされるべき")
        } catch is CancellationError {
            // 期待どおり
        }
    }

    func testProgressReporting() async throws {
        let (left, right, cleanup) = try makeTempPair()
        defer { cleanup() }

        for index in 0..<50 {
            try write("content \(index)", to: left, "f\(index).txt")
            try write("content \(index)", to: right, "f\(index).txt")
        }

        final class Box: @unchecked Sendable {
            let lock = NSLock()
            var scanned = false
            var comparedTotal = 0
            var lastDone = 0
        }
        let box = Box()

        _ = try await FolderComparator().compare(left: left, right: right) { progress in
            box.lock.lock()
            defer { box.lock.unlock() }
            switch progress {
            case .scanning: box.scanned = true
            case .comparing(let done, let total):
                box.comparedTotal = total
                box.lastDone = max(box.lastDone, done)
            }
        }

        XCTAssertTrue(box.scanned)
        XCTAssertEqual(box.comparedTotal, 50)
        XCTAssertEqual(box.lastDone, 50)
    }
}
