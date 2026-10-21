// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: FolderComparator.Options.recursive(再帰比較)のフォーカステスト。
// 既定(true)はネストしたファイルも比較し、false はトップレベルのディレクトリを
// エントリとして残しつつ降下しないことを一時ディレクトリ対で検証する。

import XCTest
@testable import DiffLoupeCore

final class RecursiveComparisonTests: XCTestCase {

    private func makeTempPair() throws -> (left: URL, right: URL, cleanup: () -> Void) {
        let base = FileManager.default.temporaryDirectory
            .appendingPathComponent("DiffLoupeCoreTests-\(UUID().uuidString)")
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

    /// 既定(recursive=true)はネストしたファイルも比較対象に含む(SwiftUIアプリの既存挙動)。
    func testRecursiveDefaultIncludesNestedFiles() async throws {
        let (left, right, cleanup) = try makeTempPair()
        defer { cleanup() }

        try write("AAA", to: left, "sub/nested.txt")
        try write("BBB", to: right, "sub/nested.txt")

        let result = try await FolderComparator().compare(left: left, right: right)
        XCTAssertEqual(result.entriesByPath["sub"]?.isDirectory, true)
        XCTAssertEqual(result.entriesByPath["sub/nested.txt"]?.status, .different)
        XCTAssertEqual(result.summary.differentFiles, 1)
    }

    /// recursive=false は各ルートの直下エントリのみを対象とし、ディレクトリは
    /// エントリとして残すが中へは降下しない。
    func testNonRecursiveKeepsTopLevelDirectoriesWithoutDescending() async throws {
        let (left, right, cleanup) = try makeTempPair()
        defer { cleanup() }

        try write("AAA", to: left, "sub/nested.txt")
        try write("BBB", to: right, "sub/nested.txt")

        let result = try await FolderComparator().compare(
            left: left, right: right,
            options: FolderComparator.Options(recursive: false)
        )

        // トップレベルのディレクトリはエントリとして残る(中身未読のためidentical)
        XCTAssertEqual(result.entriesByPath["sub"]?.isDirectory, true)
        XCTAssertEqual(result.entriesByPath["sub"]?.status, .identical)
        // 中へは降下しない → ネストしたファイルは対象に含まれない
        XCTAssertNil(result.entriesByPath["sub/nested.txt"])
        XCTAssertEqual(result.summary.totalFiles, 0)
    }

    /// recursive=false でもトップレベルのファイル差分は検出される。
    func testNonRecursiveDetectsTopLevelDifferences() async throws {
        let (left, right, cleanup) = try makeTempPair()
        defer { cleanup() }

        try write("AAA", to: left, "top.txt")
        try write("BBB", to: right, "top.txt")
        try write("left only", to: left, "only_left.txt")

        let result = try await FolderComparator().compare(
            left: left, right: right,
            options: FolderComparator.Options(recursive: false)
        )
        XCTAssertEqual(result.entriesByPath["top.txt"]?.status, .different)
        XCTAssertEqual(result.entriesByPath["only_left.txt"]?.status, .leftOnly)
        XCTAssertEqual(result.summary.differentFiles, 1)
        XCTAssertEqual(result.summary.leftOnlyFiles, 1)
    }

    /// 両側に同名ディレクトリだけがある場合、recursive=false では identical(中身未読)。
    func testNonRecursiveSameNamedDirectoryIsIdentical() async throws {
        let (left, right, cleanup) = try makeTempPair()
        defer { cleanup() }

        try write("x", to: left, "d/inner.txt")
        try write("y", to: right, "d/inner.txt")

        let result = try await FolderComparator().compare(
            left: left, right: right,
            options: FolderComparator.Options(recursive: false)
        )
        XCTAssertEqual(result.entriesByPath["d"]?.isDirectory, true)
        XCTAssertEqual(result.entriesByPath["d"]?.status, .identical)
        XCTAssertNil(result.entriesByPath["d/inner.txt"])
    }

    /// recursive=false でも、トップレベル直下のディレクトリ名の片側のみは検出される。
    func testNonRecursiveDetectsTopLevelDirectoryOnly() async throws {
        let (left, right, cleanup) = try makeTempPair()
        defer { cleanup() }

        try write("x", to: left, "left_only_dir/inner.txt")

        let result = try await FolderComparator().compare(
            left: left, right: right,
            options: FolderComparator.Options(recursive: false)
        )
        XCTAssertEqual(result.entriesByPath["left_only_dir"]?.isDirectory, true)
        XCTAssertEqual(result.entriesByPath["left_only_dir"]?.status, .leftOnly)
    }
}
