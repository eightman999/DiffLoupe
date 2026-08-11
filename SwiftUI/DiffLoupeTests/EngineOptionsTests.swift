// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: 比較オプション拡張(ユーザー除外glob / 内容比較モード / リネーム検出 / メタデータ)の検証。
// 既存挙動(既定除外・厳密比較・DiffStatus)を壊していないことも併せて確認する

import XCTest
@testable import DiffLoupe

final class EngineOptionsTests: XCTestCase {

    // MARK: - 一時ディレクトリヘルパ

    private func makeTempPair() throws -> (left: URL, right: URL, cleanup: () -> Void) {
        let base = FileManager.default.temporaryDirectory
            .appendingPathComponent("EngineOptionsTests-\(UUID().uuidString)")
        let left = base.appendingPathComponent("left")
        let right = base.appendingPathComponent("right")
        try FileManager.default.createDirectory(at: left, withIntermediateDirectories: true)
        try FileManager.default.createDirectory(at: right, withIntermediateDirectories: true)
        return (left, right, { try? FileManager.default.removeItem(at: base) })
    }

    private func write(_ data: Data, to dir: URL, _ relative: String) throws {
        let url = dir.appendingPathComponent(relative)
        try FileManager.default.createDirectory(
            at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
        try data.write(to: url)
    }

    private func write(_ text: String, to dir: URL, _ relative: String) throws {
        try write(Data(text.utf8), to: dir, relative)
    }

    private func compare(
        left: URL, right: URL,
        options: FolderComparator.Options = .init(),
        progress: FolderComparator.ProgressHandler? = nil
    ) async throws -> ComparisonResult {
        try await FolderComparator().compare(left: left, right: right, options: options, progress: progress)
    }

    // MARK: - ユーザー定義除外glob

    func testGlobPatternMatching() {
        XCTAssertTrue(GlobPattern("*.bak").matches(name: "a.bak", relativePath: "sub/a.bak"))
        XCTAssertFalse(GlobPattern("*.bak").matches(name: "a.txt", relativePath: "sub/a.txt"))
        XCTAssertTrue(GlobPattern("build/").matches(name: "build", relativePath: "build"))
        XCTAssertFalse(GlobPattern("build/").matches(name: "build", relativePath: "build/x.txt"))
        XCTAssertTrue(GlobPattern("build/*").matches(name: "x.txt", relativePath: "build/x.txt"))
        XCTAssertFalse(GlobPattern("build/*").matches(name: "x.txt", relativePath: "build/sub/x.txt"))
        XCTAssertTrue(GlobPattern("**/tmp/*.log").matches(name: "a.log", relativePath: "src/tmp/a.log"))
        XCTAssertTrue(GlobPattern("**/tmp/*.log").matches(name: "a.log", relativePath: "tmp/a.log"))
        XCTAssertFalse(GlobPattern("**/tmp/*.log").matches(name: "a.log", relativePath: "src/tmp/keep.txt"))
        XCTAssertTrue(GlobPattern("a?c").matches(name: "abc", relativePath: "abc"))
        XCTAssertFalse(GlobPattern("a?c").matches(name: "ac", relativePath: "ac"))
        XCTAssertTrue(GlobPattern("ab*").matches(name: "ab", relativePath: "ab"))
    }

    func testUserExcludeGlobBasename() async throws {
        let (left, right, cleanup) = try makeTempPair()
        defer { cleanup() }
        try write("a", to: left, "a.txt")
        try write("b", to: left, "b.bak")
        try write("c", to: left, "sub/c.bak")
        try write("keep", to: left, "sub/keep.txt")

        let result = try await compare(
            left: left, right: right,
            options: .init(userExcludePatterns: ["*.bak"]))
        let paths = Set(result.entries.map(\.relativePath))
        XCTAssertTrue(paths.contains("a.txt"))
        XCTAssertTrue(paths.contains("sub/keep.txt"))
        XCTAssertFalse(paths.contains("b.bak"))
        XCTAssertFalse(paths.contains("sub/c.bak"))
    }

    func testUserExcludeGlobRelativePathPrunesSubtree() async throws {
        let (left, right, cleanup) = try makeTempPair()
        defer { cleanup() }
        try write("x", to: left, "build/x.txt")
        try write("y", to: left, "build/sub/y.txt")
        try write("z", to: left, "src/z.txt")

        let result = try await compare(
            left: left, right: right,
            options: .init(userExcludePatterns: ["build/"]))
        let paths = Set(result.entries.map(\.relativePath))
        XCTAssertFalse(paths.contains("build"))
        XCTAssertFalse(paths.contains("build/x.txt"))
        XCTAssertFalse(paths.contains("build/sub/y.txt"))
        XCTAssertTrue(paths.contains("src/z.txt"))
    }

    func testUserExcludeGlobDoubleStar() async throws {
        let (left, right, cleanup) = try makeTempPair()
        defer { cleanup() }
        try write("log1", to: left, "tmp/a.log")
        try write("log2", to: left, "src/tmp/b.log")
        try write("keep", to: left, "src/tmp/keep.txt")

        let result = try await compare(
            left: left, right: right,
            options: .init(userExcludePatterns: ["**/tmp/*.log"]))
        let paths = Set(result.entries.map(\.relativePath))
        XCTAssertFalse(paths.contains("tmp/a.log"))
        XCTAssertFalse(paths.contains("src/tmp/b.log"))
        XCTAssertTrue(paths.contains("src/tmp/keep.txt"))
    }

    func testUserExcludeGlobDirectoryNamePrunesWholeTree() async throws {
        let (left, right, cleanup) = try makeTempPair()
        defer { cleanup() }
        try write("index", to: left, "vendor/package/index.js")
        try write("app", to: left, "app.txt")

        let result = try await compare(
            left: left, right: right,
            options: .init(userExcludePatterns: ["vendor"]))
        let paths = Set(result.entries.map(\.relativePath))
        XCTAssertFalse(paths.contains("vendor"))
        XCTAssertFalse(paths.contains("vendor/package/index.js"))
        XCTAssertTrue(paths.contains("app.txt"))
    }

    func testUserExcludeGlobKeepsDefaultExcludes() async throws {
        let (left, right, cleanup) = try makeTempPair()
        defer { cleanup() }
        try write("cfg", to: left, ".git/config")
        try write("junk", to: left, "junk.tmp")
        try write("keep", to: left, "keep.txt")

        let result = try await compare(
            left: left, right: right,
            options: .init(userExcludePatterns: ["*.tmp"]))
        let paths = Set(result.entries.map(\.relativePath))
        XCTAssertEqual(paths, ["keep.txt"])
    }

    // MARK: - 内容比較モード

    func testStrictModeIsDefaultForLineEndings() async throws {
        // 既定(厳密): 改行コードのみ異なるファイルは「相違」(既存挙動を維持)
        let (left, right, cleanup) = try makeTempPair()
        defer { cleanup() }
        try write(Data("a\nb\n".utf8), to: left, "f.txt")
        try write(Data("a\r\nb\r\n".utf8), to: right, "f.txt")
        let result = try await compare(left: left, right: right)
        XCTAssertEqual(result.entriesByPath["f.txt"]?.status, .different)
    }

    func testIgnoreLineEndings() async throws {
        let (left, right, cleanup) = try makeTempPair()
        defer { cleanup() }
        try write(Data("a\nb\n".utf8), to: left, "f.txt")
        try write(Data("a\r\nb\r\n".utf8), to: right, "f.txt")

        // サイズが異なる(CRLF分だけ右が大きい)ため、正規化比較経路で一致になること
        let result = try await compare(
            left: left, right: right,
            options: .init(contentComparison: .init(ignoreLineEndings: true)))
        XCTAssertEqual(result.entriesByPath["f.txt"]?.status, .identical)
    }

    func testIgnoreTrailingWhitespace() async throws {
        let (left, right, cleanup) = try makeTempPair()
        defer { cleanup() }
        try write(Data("a \nb\t\n".utf8), to: left, "f.txt")
        try write(Data("a\nb\n".utf8), to: right, "f.txt")

        let result = try await compare(
            left: left, right: right,
            options: .init(contentComparison: .init(ignoreTrailingWhitespace: true)))
        XCTAssertEqual(result.entriesByPath["f.txt"]?.status, .identical)
    }

    func testNormalizeWhitespace() async throws {
        let (left, right, cleanup) = try makeTempPair()
        defer { cleanup() }
        try write(Data("a  b\n".utf8), to: left, "f.txt")
        try write(Data("a b\n".utf8), to: right, "f.txt")

        let result = try await compare(
            left: left, right: right,
            options: .init(contentComparison: .init(normalizeWhitespace: true)))
        XCTAssertEqual(result.entriesByPath["f.txt"]?.status, .identical)
    }

    func testIgnoreCase() async throws {
        let (left, right, cleanup) = try makeTempPair()
        defer { cleanup() }
        try write(Data("Hello World\n".utf8), to: left, "f.txt")
        try write(Data("hello world\n".utf8), to: right, "f.txt")

        let result = try await compare(
            left: left, right: right,
            options: .init(contentComparison: .init(ignoreCase: true)))
        XCTAssertEqual(result.entriesByPath["f.txt"]?.status, .identical)
    }

    func testCombinedNormalization() async throws {
        let (left, right, cleanup) = try makeTempPair()
        defer { cleanup() }
        try write(Data("HELLO  WORLD\r\n".utf8), to: left, "f.txt")
        try write(Data("hello world\n".utf8), to: right, "f.txt")

        var options = FolderComparator.Options()
        options.contentComparison = ContentComparison(
            ignoreLineEndings: true, normalizeWhitespace: true, ignoreCase: true)
        let result = try await compare(left: left, right: right, options: options)
        XCTAssertEqual(result.entriesByPath["f.txt"]?.status, .identical)
    }

    func testNormalizationDoesNotMissRealDifferences() async throws {
        let (left, right, cleanup) = try makeTempPair()
        defer { cleanup() }
        // 大文字小文字無視でも実際の文字列違いは相違のまま
        try write(Data("hello\n".utf8), to: left, "f.txt")
        try write(Data("world\n".utf8), to: right, "f.txt")

        let result = try await compare(
            left: left, right: right,
            options: .init(contentComparison: .init(ignoreCase: true)))
        XCTAssertEqual(result.entriesByPath["f.txt"]?.status, .different)
    }

    func testBinaryFilesUseByteFallbackUnderNormalization() async throws {
        let (left, right, cleanup) = try makeTempPair()
        defer { cleanup() }
        // NULバイトを含むバイナリは正規化せずバイト厳密比較にフォールバック
        try write(Data([0x00, 0x01, 0x41]), to: left, "bin.dat")
        try write(Data([0x00, 0x01, 0x41]), to: right, "bin.dat")

        let same = try await compare(
            left: left, right: right,
            options: .init(contentComparison: .init(ignoreCase: true)))
        XCTAssertEqual(same.entriesByPath["bin.dat"]?.status, .identical)

        // サイズ不一致のバイナリは正規化対象外 → バイト厳密で「相違」
        try write(Data([0x00, 0x01, 0x42, 0x43]), to: right, "bin.dat")
        let different = try await compare(
            left: left, right: right,
            options: .init(contentComparison: .init(ignoreCase: true)))
        XCTAssertEqual(different.entriesByPath["bin.dat"]?.status, .different)
    }

    func testLineDiffUsesSharedNormalization() throws {
        // 詳細差分(LineDiff)でも同じContentComparisonが効くことを確認
        let rows = LineDiff.rows(
            leftText: "Hello World\n", rightText: "hello world\n",
            contentComparison: ContentComparison(ignoreCase: true))
        XCTAssertEqual(rows?.allSatisfy { $0.kind == .context }, true)

        let strictRows = LineDiff.rows(
            leftText: "Hello World\n", rightText: "hello world\n")
        XCTAssertEqual(strictRows?.allSatisfy { $0.kind == .context }, false)
    }

    // MARK: - リネーム/移動候補の検出

    func testRenameDetectionUniqueMove() async throws {
        let (left, right, cleanup) = try makeTempPair()
        defer { cleanup() }
        try write("shared content", to: left, "old.txt")
        try write("shared content", to: right, "new.txt")

        let result = try await compare(
            left: left, right: right,
            options: .init(detectRenames: true))

        // DiffStatusの意味は壊さない
        XCTAssertEqual(result.entriesByPath["old.txt"]?.status, .leftOnly)
        XCTAssertEqual(result.entriesByPath["new.txt"]?.status, .rightOnly)

        let leftCandidate = try XCTUnwrap(result.entriesByPath["old.txt"]?.renameCandidate)
        XCTAssertEqual(leftCandidate.counterpartPath, "new.txt")
        XCTAssertTrue(leftCandidate.isUniqueMatch)
        XCTAssertEqual(leftCandidate.totalMatches, 1)

        let rightCandidate = try XCTUnwrap(result.entriesByPath["new.txt"]?.renameCandidate)
        XCTAssertEqual(rightCandidate.counterpartPath, "old.txt")
        XCTAssertTrue(rightCandidate.isUniqueMatch)
        XCTAssertEqual(rightCandidate.totalMatches, 1)
    }

    func testRenameDetectionDisabledByDefault() async throws {
        let (left, right, cleanup) = try makeTempPair()
        defer { cleanup() }
        try write("shared content", to: left, "old.txt")
        try write("shared content", to: right, "new.txt")

        let result = try await compare(left: left, right: right)
        XCTAssertNil(result.entriesByPath["old.txt"]?.renameCandidate)
        XCTAssertNil(result.entriesByPath["new.txt"]?.renameCandidate)
    }

    func testRenameDetectionDuplicateHandling() async throws {
        let (left, right, cleanup) = try makeTempPair()
        defer { cleanup() }
        try write("dup content", to: left, "a1.txt")
        try write("dup content", to: left, "a2.txt")
        try write("dup content", to: right, "b1.txt")

        let result = try await compare(
            left: left, right: right,
            options: .init(detectRenames: true))

        // 左側は2件・右側は1件 → 1対1対応ではない
        for path in ["a1.txt", "a2.txt"] {
            let candidate = try XCTUnwrap(result.entriesByPath[path]?.renameCandidate)
            XCTAssertFalse(candidate.isUniqueMatch)
            XCTAssertEqual(candidate.counterpartPath, "b1.txt")
            XCTAssertEqual(candidate.totalMatches, 1)
        }
        let rightCandidate = try XCTUnwrap(result.entriesByPath["b1.txt"]?.renameCandidate)
        XCTAssertFalse(rightCandidate.isUniqueMatch)
        XCTAssertEqual(rightCandidate.totalMatches, 2)
    }

    func testRenameDetectionIgnoresDirectoriesAndDifferentContent() async throws {
        let (left, right, cleanup) = try makeTempPair()
        defer { cleanup() }
        try FileManager.default.createDirectory(
            at: left.appendingPathComponent("dir_only"), withIntermediateDirectories: true)
        try FileManager.default.createDirectory(
            at: right.appendingPathComponent("moved_dir"), withIntermediateDirectories: true)
        try write("AAA", to: left, "only_left.txt")
        try write("BBB", to: right, "only_right.txt")

        let result = try await compare(
            left: left, right: right,
            options: .init(detectRenames: true))

        // ディレクトリは候補にならない
        XCTAssertNil(result.entriesByPath["dir_only"]?.renameCandidate)
        XCTAssertNil(result.entriesByPath["moved_dir"]?.renameCandidate)
        // 内容が異なる片側のみファイルは候補にならない
        XCTAssertNil(result.entriesByPath["only_left.txt"]?.renameCandidate)
        XCTAssertNil(result.entriesByPath["only_right.txt"]?.renameCandidate)
    }

    // MARK: - リネーム検出の進捗とキャンセル

    func testRenameDetectionReportsProgress() async throws {
        let (left, right, cleanup) = try makeTempPair()
        defer { cleanup() }
        for index in 0..<30 {
            try write("content \(index)", to: left, "old\(index).txt")
            try write("content \(index)", to: right, "new\(index).txt")
        }

        final class Box: @unchecked Sendable {
            let lock = NSLock()
            var renamesTotal = 0
            var renamesDone = 0
        }
        let box = Box()

        let result = try await compare(
            left: left, right: right,
            options: .init(detectRenames: true)) { progress in
            box.lock.lock()
            defer { box.lock.unlock() }
            if case .renames(let done, let total) = progress {
                box.renamesTotal = total
                box.renamesDone = max(box.renamesDone, done)
            }
        }

        // 左30 + 右30 = 60件をハッシュ計算して進捗を通知する
        XCTAssertEqual(box.renamesTotal, 60)
        XCTAssertEqual(box.renamesDone, 60)
        XCTAssertNotNil(result.entriesByPath["old0.txt"]?.renameCandidate)
        XCTAssertNotNil(result.entriesByPath["new0.txt"]?.renameCandidate)
    }

    func testRenameDetectionCancellationPropagates() async throws {
        // リネーム検出中のキャンセルは、読み込みエラーとして隠蔽せずCancellationErrorを再送出する
        let (left, right, cleanup) = try makeTempPair()
        defer { cleanup() }
        for index in 0..<800 {
            try write(String(repeating: "x", count: 4096), to: left, "old\(index).txt")
            try write(String(repeating: "x", count: 4096), to: right, "new\(index).txt")
        }

        final class Box: @unchecked Sendable {
            let lock = NSLock()
            var sawRenames = false
        }
        let box = Box()
        let task = Task {
            try await FolderComparator().compare(
                left: left, right: right,
                options: .init(detectRenames: true)) { progress in
                box.lock.lock()
                defer { box.lock.unlock() }
                if case .renames = progress { box.sawRenames = true }
            }
        }

        // リネーム検出フェーズに入ったことを確認してからキャンセルする
        while true {
            try await Task.sleep(for: .milliseconds(5))
            box.lock.lock()
            let saw = box.sawRenames
            box.lock.unlock()
            if saw {
                task.cancel()
                break
            }
        }

        do {
            _ = try await task.value
            XCTFail("リネーム検出中にキャンセルされるとCancellationErrorが投げられるべき")
        } catch is CancellationError {
            // 期待どおり
        }
    }

    // MARK: - 巨大ファイル保護(正規化比較の一括読込上限)

    func testHugeFilesFallBackToByteCompareWithoutOOM() async throws {
        let (left, right, cleanup) = try makeTempPair()
        defer { cleanup() }

        // 上限を16バイトに引き下げ、通常サイズのファイルを「巨大扱い」にする
        let smallLimit: Int64 = 16
        let text = String(repeating: "a", count: 32)

        // サイズ不一致: 一括読込の正規化比較をせず、バイト厳密(サイズ)で相違
        try write(Data(text.utf8), to: left, "f.txt")
        try write(Data((text + text).utf8), to: right, "f.txt")
        let diff = try await compare(
            left: left, right: right,
            options: .init(
                contentComparison: .init(ignoreLineEndings: true),
                maxNormalizationCompareBytes: smallLimit))
        XCTAssertEqual(diff.entriesByPath["f.txt"]?.status, .different)

        // サイズ一致・同一内容: ストリーミングハッシュ経路で一致
        try write(Data(text.utf8), to: right, "f.txt")
        let same = try await compare(
            left: left, right: right,
            options: .init(
                contentComparison: .init(ignoreLineEndings: true),
                maxNormalizationCompareBytes: smallLimit))
        XCTAssertEqual(same.entriesByPath["f.txt"]?.status, .identical)

        // 上限内のファイルは従来どおり正規化比較が働く(CRLF差でも一致)
        try write(Data("a\r\nb\r\n".utf8), to: left, "g.txt")
        try write(Data("a\nb\n".utf8), to: right, "g.txt")
        let inLimit = try await compare(
            left: left, right: right,
            options: .init(contentComparison: .init(ignoreLineEndings: true)))
        XCTAssertEqual(inLimit.entriesByPath["g.txt"]?.status, .identical)
    }

    // MARK: - globの空パターン・Unicode

    func testEmptyGlobPatternMatchesNothing() {
        XCTAssertFalse(GlobPattern("").matches(name: "a.txt", relativePath: "a.txt"))
        XCTAssertFalse(GlobPattern("").matches(name: "a.txt", relativePath: "sub/a.txt"))
        XCTAssertFalse(GlobPattern("  ").matches(name: "a.txt", relativePath: "a.txt"))
    }

    func testGlobPatternWithUnicodeAndNFC() {
        // NFC正規化済みの相対パスに日本語名がマッチする
        XCTAssertTrue(GlobPattern("*.txt").matches(name: "ファイル.txt", relativePath: "ディレクトリ/ファイル.txt"))
        XCTAssertTrue(GlobPattern("**/サブ/*.txt").matches(name: "a.txt", relativePath: "ディレクトリ/サブ/a.txt"))
        XCTAssertTrue(GlobPattern("写真?.jpg").matches(name: "写真A.jpg", relativePath: "写真A.jpg"))
        XCTAssertFalse(GlobPattern("写真?.jpg").matches(name: "写真AB.jpg", relativePath: "写真AB.jpg"))

        // 分解形(NFD)の文字列を正規化(NFC)すると合成形に一致する
        let nfd = "が".decomposedStringWithCanonicalMapping   // が を分解した形
        let nfc = nfd.precomposedStringWithCanonicalMapping
        XCTAssertEqual(nfc, "が")
        XCTAssertTrue(GlobPattern("*.txt").matches(name: "file.txt", relativePath: nfc + "/file.txt"))
    }

    // MARK: - UI設定の読み込み(純粋関数)

    private func makeUserDefaultsSuite() -> (UserDefaults, () -> Void) {
        let name = "EngineOptionsTests-\(UUID().uuidString)"
        let defaults = UserDefaults(suiteName: name)!
        defaults.register(defaults: AppModel.defaultSettings)
        return (defaults, { defaults.removePersistentDomain(forName: name) })
    }

    func testNormalizeExcludePatterns() {
        XCTAssertEqual(AppModel.normalizeExcludePatterns(""), [])
        XCTAssertEqual(AppModel.normalizeExcludePatterns("\n  \n\n"), [])
        XCTAssertEqual(
            AppModel.normalizeExcludePatterns("  *.bak  \n\n*.tmp\n*.bak\n"),
            ["*.bak", "*.tmp"])
        XCTAssertEqual(
            AppModel.normalizeExcludePatterns("build/ \n build/ \nsub/**\n"),
            ["build/", "sub/**"])
    }

    func testDefaultSettingsYieldStrictComparison() {
        let (defaults, cleanup) = makeUserDefaultsSuite()
        defer { cleanup() }

        // 初期値は厳密比較・リネーム検出オフ・既定除外オン(後方互換)
        let options = AppModel.makeOptions(defaults: defaults, maxEntries: 7)
        XCTAssertEqual(options.useDefaultExcludes, true)
        XCTAssertEqual(options.maxEntries, 7)
        XCTAssertTrue(options.contentComparison.isStrict)
        XCTAssertEqual(options.contentComparison.settingsSignature, "0000")
        XCTAssertEqual(options.detectRenames, false)
        XCTAssertEqual(options.userExcludePatterns, [])
    }

    func testMakeOptionsReadsAllUserDefaults() {
        let (defaults, cleanup) = makeUserDefaultsSuite()
        defer { cleanup() }

        defaults.set(false, forKey: AppModel.useDefaultExcludesKey)
        defaults.set("*.bak\n\n sub/tmp* \n*.bak", forKey: AppModel.userExcludePatternsKey)
        defaults.set(true, forKey: AppModel.ignoreLineEndingsKey)
        defaults.set(true, forKey: AppModel.ignoreTrailingWhitespaceKey)
        defaults.set(true, forKey: AppModel.normalizeWhitespaceKey)
        defaults.set(true, forKey: AppModel.ignoreCaseKey)
        defaults.set(true, forKey: AppModel.detectRenamesKey)

        let options = AppModel.makeOptions(defaults: defaults, maxEntries: nil)
        XCTAssertEqual(options.useDefaultExcludes, false)
        XCTAssertNil(options.maxEntries)
        XCTAssertEqual(options.userExcludePatterns, ["*.bak", "sub/tmp*"])
        XCTAssertEqual(options.contentComparison.ignoreLineEndings, true)
        XCTAssertEqual(options.contentComparison.ignoreTrailingWhitespace, true)
        XCTAssertEqual(options.contentComparison.normalizeWhitespace, true)
        XCTAssertEqual(options.contentComparison.ignoreCase, true)
        XCTAssertFalse(options.contentComparison.isStrict)
        XCTAssertEqual(options.detectRenames, true)
    }

    func testContentComparisonSignatureReflectsSettings() {
        var comparison = ContentComparison()
        XCTAssertEqual(comparison.settingsSignature, "0000")
        comparison.ignoreLineEndings = true
        XCTAssertEqual(comparison.settingsSignature, "1000")
        comparison.ignoreTrailingWhitespace = true
        XCTAssertEqual(comparison.settingsSignature, "1100")
        comparison.normalizeWhitespace = true
        XCTAssertEqual(comparison.settingsSignature, "1110")
        comparison.ignoreCase = true
        XCTAssertEqual(comparison.settingsSignature, "1111")
    }

    // MARK: - 正規化時の元表示の保持(LineDiff)

    func testIgnoreCaseKeepsOriginalDisplayText() throws {
        let rows = try XCTUnwrap(LineDiff.rows(
            leftText: "Hello World\nSecond Line\n",
            rightText: "hello world\nsecond line\n",
            contentComparison: ContentComparison(ignoreCase: true)))
        XCTAssertEqual(rows.allSatisfy { $0.kind == .context }, true)
        let contextRows = rows.filter { $0.kind == .context }
        XCTAssertEqual(contextRows[0].leftText, "Hello World")
        XCTAssertEqual(contextRows[0].rightText, "hello world")
        XCTAssertEqual(contextRows[1].leftText, "Second Line")
        XCTAssertEqual(contextRows[1].rightText, "second line")
    }

    func testNormalizeWhitespaceKeepsOriginalDisplayText() throws {
        let rows = try XCTUnwrap(LineDiff.rows(
            leftText: "a  b\n",
            rightText: "a b\n",
            contentComparison: ContentComparison(normalizeWhitespace: true)))
        XCTAssertEqual(rows.allSatisfy { $0.kind == .context }, true)
        let row = try XCTUnwrap(rows.first { $0.kind == .context })
        XCTAssertEqual(row.leftText, "a  b")
        XCTAssertEqual(row.rightText, "a b")
    }

    func testIgnoreLineEndingsKeepsOriginalCRInDisplay() throws {
        // 差分計算上は一致するが、表示には元の行末CRが残る(改行差を確認できる)
        let rows = try XCTUnwrap(LineDiff.rows(
            leftText: "a\r\nb\r\n",
            rightText: "a\nb\n",
            contentComparison: ContentComparison(ignoreLineEndings: true)))
        XCTAssertEqual(rows.allSatisfy { $0.kind == .context }, true)
        XCTAssertEqual(rows[0].leftText, "a\r")
        XCTAssertEqual(rows[0].rightText, "a")
    }

    // MARK: - メタデータ

    func testMetadataIsExposed() async throws {
        let (left, right, cleanup) = try makeTempPair()
        defer { cleanup() }
        try write("same", to: left, "f.txt")
        try write("same", to: right, "f.txt")

        let result = try await compare(left: left, right: right)
        let entry = try XCTUnwrap(result.entriesByPath["f.txt"])
        XCTAssertEqual(entry.leftSize, 4)
        XCTAssertEqual(entry.rightSize, 4)
        XCTAssertNotNil(entry.leftModificationDate)
        XCTAssertNotNil(entry.rightModificationDate)
        XCTAssertNotNil(entry.leftPOSIX?.permissions)
        XCTAssertNotNil(entry.rightPOSIX?.permissions)
    }

    // MARK: - インクルードglob(フォルダ比較)

    func testUserIncludeGlobFiltersFilesByBasename() async throws {
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
            options: .init(userIncludePatterns: ["*.swift"]))
        XCTAssertEqual(Set(result.entries.map(\.relativePath)), ["a.swift"])
    }

    func testUserIncludeGlobReachesNestedFiles() async throws {
        // ディレクトリ名が一致しなくても降下してネストした一致ファイルに到達する
        let (left, right, cleanup) = try makeTempPair()
        defer { cleanup() }
        try write("AAA", to: left, "src/Engine/Foo.swift")
        try write("BBB", to: right, "src/Engine/Foo.swift")
        try write("readme", to: left, "src/README.md")
        try write("top", to: left, "top.txt")

        let result = try await compare(
            left: left, right: right,
            options: .init(userIncludePatterns: ["*.swift"]))
        let paths = Set(result.entries.map(\.relativePath))
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
            options: .init(userExcludePatterns: ["*.bak"], userIncludePatterns: []))
        XCTAssertEqual(Set(result.entries.map(\.relativePath)), ["a.txt", "keep.txt"])
    }

    func testIncludeExcludePrecedence() async throws {
        let (left, right, cleanup) = try makeTempPair()
        defer { cleanup() }
        try write("a", to: left, "Foo.swift")
        try write("b", to: left, "Generated.swift")
        try write("c", to: left, "notes.txt")

        let result = try await compare(
            left: left, right: right,
            options: .init(
                userExcludePatterns: ["Generated.swift"],
                userIncludePatterns: ["*.swift"]))
        XCTAssertEqual(Set(result.entries.map(\.relativePath)), ["Foo.swift"])
    }

    func testExcludedDirectoryPrunesSubtreeUnderInclude() async throws {
        let (left, right, cleanup) = try makeTempPair()
        defer { cleanup() }
        try write("a", to: left, "src/app.swift")
        try write("b", to: left, "build/tmp.swift")
        try write("c", to: left, "build/sub/deep.swift")

        let result = try await compare(
            left: left, right: right,
            options: .init(
                userExcludePatterns: ["build"],
                userIncludePatterns: ["**"]))
        XCTAssertEqual(Set(result.entries.map(\.relativePath)), ["src", "src/app.swift"])
    }

    // MARK: - インクルードglob・プリセット(UI設定の純粋関数)

    func testNormalizeIncludePatterns() {
        XCTAssertEqual(AppModel.normalizeIncludePatterns(""), [])
        XCTAssertEqual(AppModel.normalizeIncludePatterns("\n  \n\n"), [])
        XCTAssertEqual(
            AppModel.normalizeIncludePatterns("  *.swift  \n\nsrc/**\n*.swift\n"),
            ["*.swift", "src/**"])
    }

    func testMakeOptionsReadsIncludePatterns() {
        let (defaults, cleanup) = makeUserDefaultsSuite()
        defer { cleanup() }

        defaults.set("*.swift\n\n src/** \n*.swift", forKey: AppModel.userIncludePatternsKey)
        let options = AppModel.makeOptions(defaults: defaults, maxEntries: nil)
        XCTAssertEqual(options.userIncludePatterns, ["*.swift", "src/**"])
    }

    func testMakeOptionsExpandsSelectedPresets() {
        let (defaults, cleanup) = makeUserDefaultsSuite()
        defer { cleanup() }

        defaults.set("*.tmp", forKey: AppModel.userExcludePatternsKey)
        defaults.set("source-control,web-nodejs", forKey: AppModel.filterPresetsKey)

        let options = AppModel.makeOptions(defaults: defaults, maxEntries: nil)
        XCTAssertTrue(options.userExcludePatterns.contains("*.tmp"))
        XCTAssertTrue(options.userExcludePatterns.contains(".git"))
        XCTAssertTrue(options.userExcludePatterns.contains("node_modules"))
        XCTAssertTrue(options.userIncludePatterns.isEmpty)
    }

    func testMakeOptionsWithoutPresetsDoesNotExpand() {
        let (defaults, cleanup) = makeUserDefaultsSuite()
        defer { cleanup() }

        let options = AppModel.makeOptions(defaults: defaults, maxEntries: nil)
        XCTAssertEqual(options.userExcludePatterns, [])
        XCTAssertEqual(options.userIncludePatterns, [])
    }

    func testSelectedFilterPresetIDsDropsUnknownAndBlank() {
        let (defaults, cleanup) = makeUserDefaultsSuite()
        defer { cleanup() }

        defaults.set("source-control, bogus ,, web-nodejs", forKey: AppModel.filterPresetsKey)
        XCTAssertEqual(
            AppModel.selectedFilterPresetIDs(defaults: defaults),
            ["source-control", "web-nodejs"])
        XCTAssertEqual(
            AppModel.selectedFilterPresets(defaults: defaults),
            [.sourceControl, .webNodeJS])
    }

    func testFilterSetUnitSemantics() {
        let filter = FilterSet(includePatterns: ["*.swift"], excludePatterns: ["**/Generated.swift"])
        XCTAssertTrue(filter.allowsFile(name: "Foo.swift", relativePath: "src/Foo.swift"))
        XCTAssertFalse(filter.allowsFile(name: "Generated.swift", relativePath: "src/Generated.swift"))
        XCTAssertFalse(filter.allowsFile(name: "README.md", relativePath: "README.md"))
        // インクルードはディレクトリを刈らない
        XCTAssertFalse(filter.isExcluded(name: "src", relativePath: "src"))
        // 空インクルードは全許可(従来挙動)
        XCTAssertTrue(FilterSet().allowsFile(name: "anything", relativePath: "anything"))
        // プリセットはopt-inで、既定の比較へ自動追加されない
        XCTAssertEqual(FilterPreset.all.map(\.id), ["source-control", "build-artifacts", "web-nodejs", "macos-metadata"])
    }

    func testFilterSetDropsBlankPatternsSoEmptyFilterAllowsEverything() {
        // 空白のみのインクルード/エクスクルードは初期化時に除去される。
        // 除去しないとインクルード非空と誤判定され「全ファイル除外」になる(回帰防止)。
        let blankIncludes = FilterSet(includePatterns: ["", "  "])
        XCTAssertEqual(blankIncludes.includePatterns, [])
        XCTAssertTrue(blankIncludes.allowsFile(name: "a.txt", relativePath: "a.txt"))
        XCTAssertTrue(blankIncludes.allowsFile(name: "a.txt", relativePath: "sub/a.txt"))

        let blankExcludes = FilterSet(excludePatterns: ["", "  "])
        XCTAssertEqual(blankExcludes.excludePatterns, [])
        XCTAssertTrue(blankExcludes.allowsFile(name: "a.txt", relativePath: "a.txt"))
    }

    func testFilterSetTrimsAndDeduplicatesPatternsPreservingOrder() {
        let filter = FilterSet(
            includePatterns: [" *.swift ", "*.swift", "src/**", "", "src/**"],
            excludePatterns: ["build/", " build/ "])
        XCTAssertEqual(filter.includePatterns, ["*.swift", "src/**"])
        XCTAssertEqual(filter.excludePatterns, ["build/"])

        // UI(SettingViewのTextEditor)は行区切り入力のため、空行・重複は上記で吸収される
        let (defaults, cleanup) = makeUserDefaultsSuite()
        defer { cleanup() }
        defaults.set("*.swift\n\n src/** \n*.swift", forKey: AppModel.userIncludePatternsKey)
        let viaAppModel = AppModel.makeOptions(defaults: defaults, maxEntries: nil)
        XCTAssertEqual(viaAppModel.userIncludePatterns, ["*.swift", "src/**"])
    }

    func testFilterSetUnicodeMatchingIsNormalizationInsensitive() {
        // SwiftのCharacter比較は正規化等価のため、NFD入力でもNFCパターンに一致する。
        // FilterSet自体は正規化せず、NFCへの統一はFolderComparator側の責務。
        let composed = "が.txt"
        let decomposed = composed.precomposedStringWithCanonicalMapping
            .decomposedStringWithCanonicalMapping
        guard !decomposed.unicodeScalars.elementsEqual(composed.unicodeScalars) else { return }

        let filter = FilterSet(includePatterns: ["*.txt", composed])
        XCTAssertTrue(filter.allowsFile(name: composed, relativePath: composed))
        XCTAssertTrue(filter.allowsFile(name: decomposed, relativePath: decomposed))
        XCTAssertTrue(filter.allowsFile(name: decomposed, relativePath: "sub/" + decomposed))
    }

    // MARK: - 後方互換(既定オプションは既存挙動と同一)

    func testDefaultOptionsPreserveStrictClassification() async throws {
        let (left, right, cleanup) = try makeTempPair()
        defer { cleanup() }
        try write("same", to: left, "f.txt")
        try write("same", to: right, "f.txt")
        try write("a", to: left, "g.txt")
        try write("b", to: right, "g.txt")
        try write("only", to: left, "only_left.txt")

        let result = try await compare(left: left, right: right)
        XCTAssertEqual(result.entriesByPath["f.txt"]?.status, .identical)
        XCTAssertEqual(result.entriesByPath["g.txt"]?.status, .different)
        XCTAssertEqual(result.entriesByPath["only_left.txt"]?.status, .leftOnly)
        XCTAssertNil(result.entriesByPath["only_left.txt"]?.renameCandidate)
    }
}
