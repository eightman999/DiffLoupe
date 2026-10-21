// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: フォルダ比較のインクルード/エクスクルードフィルタ(FilterSet)と組み込みプリセット
// (FilterPreset)のフォーカステスト。空インクルード互換・ネスト透過・優先順位・
// 不正/空白パターン・プリセット展開を一時ディレクトリ対と純粋な型の両方で検証する。

import XCTest
@testable import DiffLoupeCore

final class FilterSetTests: XCTestCase {

    // MARK: - FilterSet(純粋な型)

    func testEmptyIncludeAllowsEverything() {
        let filter = FilterSet()
        XCTAssertTrue(filter.allowsFile(name: "a.txt", relativePath: "a.txt"))
        XCTAssertTrue(filter.allowsFile(name: "a.txt", relativePath: "sub/a.txt"))
        XCTAssertTrue(filter.allowsFile(name: "build", relativePath: "build"))
        XCTAssertTrue(filter.allowsFile(name: "x", relativePath: "src/x"))
    }

    func testEmptyIncludeStillHonorsExcludes() {
        let filter = FilterSet(excludePatterns: ["*.bak", "build/"])
        XCTAssertFalse(filter.allowsFile(name: "a.bak", relativePath: "a.bak"))
        XCTAssertFalse(filter.allowsFile(name: "build", relativePath: "build"))
        XCTAssertTrue(filter.allowsFile(name: "a.txt", relativePath: "a.txt"))
    }

    func testNonEmptyIncludeRequiresMatch() {
        let filter = FilterSet(includePatterns: ["*.swift"])
        XCTAssertTrue(filter.allowsFile(name: "Foo.swift", relativePath: "src/Engine/Foo.swift"))
        XCTAssertTrue(filter.allowsFile(name: "Foo.swift", relativePath: "build/Foo.swift"))
        XCTAssertFalse(filter.allowsFile(name: "README.md", relativePath: "README.md"))
        XCTAssertFalse(filter.allowsFile(name: "notes.txt", relativePath: "build/notes.txt"))
    }

    func testIncludeMatchesRelativePathWithSeparator() {
        let filter = FilterSet(includePatterns: ["src/Engine/*"])
        XCTAssertTrue(filter.allowsFile(name: "Foo.swift", relativePath: "src/Engine/Foo.swift"))
        XCTAssertFalse(filter.allowsFile(name: "Foo.swift", relativePath: "other/Foo.swift"))
        XCTAssertFalse(filter.allowsFile(name: "Foo.swift", relativePath: "src/Other/Foo.swift"))
    }

    func testExcludeWinsOverInclude() {
        let filter = FilterSet(includePatterns: ["*.swift"], excludePatterns: ["**/Generated.swift"])
        XCTAssertFalse(filter.allowsFile(name: "Generated.swift", relativePath: "src/Generated.swift"))
        XCTAssertTrue(filter.allowsFile(name: "Foo.swift", relativePath: "src/Foo.swift"))
    }

    func testIncludeDoesNotPruneDirectories() {
        let filter = FilterSet(includePatterns: ["*.swift"])
        // ディレクトリはインクルード一致の有無に関係なく除外されない(降下する)
        XCTAssertFalse(filter.isExcluded(name: "src", relativePath: "src"))
        XCTAssertFalse(filter.isExcluded(name: "Engine", relativePath: "src/Engine"))
        XCTAssertFalse(filter.isExcluded(name: "build", relativePath: "src/build"))
    }

    func testExcludedDirectoryPrunesSubtree() {
        let filter = FilterSet(includePatterns: ["**"], excludePatterns: ["build"])
        XCTAssertTrue(filter.isExcluded(name: "build", relativePath: "build"))
        XCTAssertFalse(filter.isExcluded(name: "src", relativePath: "src"))
    }

    func testBlankAndEmptyPatternsAreDroppedFromFilter() {
        // 空白・空パターンは初期化時に除去され、空フィルタ(全ファイル対象)と同一の挙動になる。
        // 除去しないとインクルード非空と誤判定され「全ファイル除外」になるため、回帰防止の要。
        XCTAssertTrue(FilterSet().allowsFile(name: "a.txt", relativePath: "a.txt"))
        let blankExcludes = FilterSet(excludePatterns: ["", "  "])
        XCTAssertEqual(blankExcludes.excludePatterns, [])
        XCTAssertTrue(blankExcludes.allowsFile(name: "a.txt", relativePath: "a.txt"))
        // 空インクルードパターンも除去され、空配列と同じく全ファイル対象
        let onlyBlankIncludes = FilterSet(includePatterns: ["", "  "])
        XCTAssertEqual(onlyBlankIncludes.includePatterns, [])
        XCTAssertTrue(onlyBlankIncludes.allowsFile(name: "a.txt", relativePath: "a.txt"))
        XCTAssertTrue(onlyBlankIncludes.allowsFile(name: "a.txt", relativePath: "sub/a.txt"))
    }

    func testPatternsAreTrimmedAndDeduplicatedPreservingOrder() {
        let filter = FilterSet(
            includePatterns: [" *.swift ", "*.swift", "", "  ", "src/**"],
            excludePatterns: ["build/", " build/ ", "node_modules"])
        // 前後空白はtrimされ、重複は最初の出現位置を保って除去され、空は落ちる
        XCTAssertEqual(filter.includePatterns, ["*.swift", "src/**"])
        XCTAssertEqual(filter.excludePatterns, ["build/", "node_modules"])
    }

    func testUnicodeFilterMatchingIsNormalizationInsensitive() {
        // SwiftのCharacter比較は正規化等価(NFC/NFDを区別しない)ため、NFDで渡しても一致する。
        // FilterSet自体は正規化せず呼び出し側(FolderComparator)がNFCへ揃える契約。
        let composed = "が.txt"
        let decomposed = composed.precomposedStringWithCanonicalMapping
            .decomposedStringWithCanonicalMapping
        // NFC/NFDが同一スカラ列になる環境ではこのテストは無意味なのでスキップ
        guard !decomposed.unicodeScalars.elementsEqual(composed.unicodeScalars) else { return }

        let filter = FilterSet(includePatterns: ["*.txt"])
        XCTAssertTrue(filter.allowsFile(name: composed, relativePath: composed))
        XCTAssertTrue(filter.allowsFile(name: decomposed, relativePath: decomposed))
        XCTAssertTrue(filter.allowsFile(name: decomposed, relativePath: "sub/" + decomposed))
    }

    func testRulesFlattening() {
        let filter = FilterSet(includePatterns: ["*.swift"], excludePatterns: ["build/"])
        let rules = filter.rules
        XCTAssertEqual(rules.count, 2)
        XCTAssertEqual(rules[0], FilterRule(kind: .include, pattern: "*.swift"))
        XCTAssertEqual(rules[1], FilterRule(kind: .exclude, pattern: "build/"))
    }

    // MARK: - FilterRule

    func testFilterRuleMatching() {
        let include = FilterRule(kind: .include, pattern: "*.swift")
        XCTAssertEqual(include.kind, .include)
        XCTAssertTrue(include.matches(name: "Foo.swift", relativePath: "src/Foo.swift"))
        XCTAssertFalse(include.matches(name: "Foo.txt", relativePath: "src/Foo.txt"))
    }

    // MARK: - FilterPreset

    func testAllPresetsHaveNonEmptySafeExcludes() {
        XCTAssertEqual(FilterPreset.all.count, 4)
        for preset in FilterPreset.all {
            XCTAssertFalse(preset.id.isEmpty, "\(preset.displayName) のIDが空")
            XCTAssertFalse(preset.displayName.isEmpty)
            XCTAssertFalse(preset.excludePatterns.isEmpty, "\(preset.displayName) の除外パターンが空")
        }
    }

    func testPresetIDsAreUnique() {
        let ids = FilterPreset.all.map(\.id)
        XCTAssertEqual(Set(ids).count, ids.count)
    }

    func testPresetLookupById() {
        XCTAssertEqual(FilterPreset.preset(id: "source-control"), .sourceControl)
        XCTAssertEqual(FilterPreset.preset(id: "build-artifacts"), .buildArtifacts)
        XCTAssertEqual(FilterPreset.preset(id: "web-nodejs"), .webNodeJS)
        XCTAssertEqual(FilterPreset.preset(id: "macos-metadata"), .macMetadata)
        XCTAssertNil(FilterPreset.preset(id: "unknown"))
        XCTAssertNil(FilterPreset.preset(id: ""))
    }

    func testPresetPatternsAreSane() {
        XCTAssertTrue(FilterPreset.sourceControl.excludePatterns.contains(".git"))
        XCTAssertTrue(FilterPreset.buildArtifacts.excludePatterns.contains("build"))
        XCTAssertTrue(FilterPreset.webNodeJS.excludePatterns.contains("node_modules"))
        XCTAssertTrue(FilterPreset.macMetadata.excludePatterns.contains(".DS_Store"))
    }

    // MARK: - FolderComparatorとの統合(一時ディレクトリ対)

    private func makeTempPair() throws -> (left: URL, right: URL, cleanup: () -> Void) {
        let base = FileManager.default.temporaryDirectory
            .appendingPathComponent("FilterSetTests-\(UUID().uuidString)")
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

    private func compare(
        left: URL, right: URL,
        options: FolderComparator.Options = .init()
    ) async throws -> DiffLoupeCore.ComparisonResult {
        try await FolderComparator().compare(left: left, right: right, options: options)
    }

    func testIncludeGlobFiltersFilesByBasename() async throws {
        let (left, right, cleanup) = try makeTempPair()
        defer { cleanup() }
        for text in ["a", "b"] {
            try write(text, to: left, "a.swift")
            try write(text, to: left, "b.txt")
            try write(text, to: right, "a.swift")
            try write(text, to: right, "b.txt")
        }

        let result = try await compare(
            left: left, right: right,
            options: FolderComparator.Options(userIncludePatterns: ["*.swift"]))
        XCTAssertEqual(Set(result.entries.map(\.relativePath)), ["a.swift"])
    }

    func testIncludeReachesNestedFilesThroughNonMatchingDirectories() async throws {
        let (left, right, cleanup) = try makeTempPair()
        defer { cleanup() }
        try write("AAA", to: left, "src/Engine/Foo.swift")
        try write("BBB", to: right, "src/Engine/Foo.swift")
        try write("readme", to: left, "src/README.md")
        try write("top", to: left, "top.txt")

        let result = try await compare(
            left: left, right: right,
            options: FolderComparator.Options(userIncludePatterns: ["*.swift"]))
        let paths = Set(result.entries.map(\.relativePath))
        // src / src/Engine は名前不一致でも降下して残る
        XCTAssertTrue(paths.contains("src"))
        XCTAssertTrue(paths.contains("src/Engine"))
        XCTAssertTrue(paths.contains("src/Engine/Foo.swift"))
        XCTAssertFalse(paths.contains("src/README.md"))
        XCTAssertFalse(paths.contains("top.txt"))
        XCTAssertEqual(result.entriesByPath["src/Engine/Foo.swift"]?.status, .different)
    }

    func testEmptyIncludeKeepsExistingExcludeBehavior() async throws {
        let (left, right, cleanup) = try makeTempPair()
        defer { cleanup() }
        try write("a", to: left, "a.txt")
        try write("b", to: left, "b.bak")
        try write("keep", to: left, "keep.txt")

        let result = try await compare(
            left: left, right: right,
            options: FolderComparator.Options(userExcludePatterns: ["*.bak"], userIncludePatterns: []))
        XCTAssertEqual(Set(result.entries.map(\.relativePath)), ["a.txt", "keep.txt"])
    }

    func testEmptyIncludeWithNoExcludesKeepsAllFiles() async throws {
        let (left, right, cleanup) = try makeTempPair()
        defer { cleanup() }
        try write("a", to: left, "a.txt")
        try write("b", to: left, "sub/b.txt")

        let result = try await compare(
            left: left, right: right,
            options: FolderComparator.Options(userIncludePatterns: []))
        XCTAssertEqual(
            Set(result.entries.map(\.relativePath)),
            ["a.txt", "sub", "sub/b.txt"])
    }

    func testIncludeExcludePrecedenceInComparator() async throws {
        let (left, right, cleanup) = try makeTempPair()
        defer { cleanup() }
        try write("a", to: left, "Foo.swift")
        try write("b", to: left, "Generated.swift")
        try write("c", to: left, "notes.txt")

        let result = try await compare(
            left: left, right: right,
            options: FolderComparator.Options(
                userExcludePatterns: ["Generated.swift"],
                userIncludePatterns: ["*.swift"]))
        XCTAssertEqual(Set(result.entries.map(\.relativePath)), ["Foo.swift"])
    }

    func testExcludedDirectoryPrunesSubtreeUnderIncludes() async throws {
        let (left, right, cleanup) = try makeTempPair()
        defer { cleanup() }
        try write("a", to: left, "src/app.swift")
        try write("b", to: left, "build/tmp.swift")
        try write("c", to: left, "build/sub/deep.swift")

        let result = try await compare(
            left: left, right: right,
            options: FolderComparator.Options(
                userExcludePatterns: ["build"],
                userIncludePatterns: ["**"]))
        XCTAssertEqual(Set(result.entries.map(\.relativePath)), ["src", "src/app.swift"])
    }

    func testNFCNormalizedResultKeysForUnicodeNames() async throws {
        // APFS/HFS+はファイル名をNFDで返すため、scanTreeはフィルタ照合と結果キーをNFCへ揃える。
        // 結果キーがNFC形式(単一コードポイント)であることをスカラ列で直接検証する。
        let (left, right, cleanup) = try makeTempPair()
        defer { cleanup() }
        let composed = "が.txt"
        let decomposed = composed.precomposedStringWithCanonicalMapping
            .decomposedStringWithCanonicalMapping
        guard !decomposed.unicodeScalars.elementsEqual(composed.unicodeScalars) else { return }

        try write("a", to: left, composed)
        try write("b", to: left, "other.txt")
        let result = try await compare(left: left, right: right)
        let key = result.entries
            .first { $0.relativePath.precomposedStringWithCanonicalMapping == composed }
        XCTAssertNotNil(key)
        XCTAssertEqual(
            key?.relativePath.unicodeScalars.map { $0.value },
            [0x304C, 0x2E, 0x74, 0x78, 0x74],
            "結果キーはNFC(が.txt)であるべき")
    }
}
