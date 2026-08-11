// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: インクルードglob(-i/--include)と組み込みプリセット(--preset)の引数解析と、
// CLIRunner.comparatorOptions によるFolderComparator.Optionsへの伝播を検証する。
// 既存のWinMergeスラッシュスイッチ/絶対パス解析を壊していないことも併せて確認する。

import XCTest
@testable import DiffLoupeCLI
@testable import DiffLoupeCore

final class FilterCLITests: XCTestCase {

    // MARK: - パース

    func testDefaultsAreEmpty() {
        guard case .options(let options) = CLIOptionsParser.parse(["a", "b"]) else {
            return XCTFail("expected .options")
        }
        XCTAssertTrue(options.userIncludePatterns.isEmpty)
        XCTAssertTrue(options.filterPresets.isEmpty)
    }

    func testShortIncludeIsRepeatable() {
        guard case .options(let options) = CLIOptionsParser.parse([
            "-i", "*.swift", "-i", "src/**", "a", "b",
        ]) else {
            return XCTFail()
        }
        XCTAssertEqual(options.userIncludePatterns, ["*.swift", "src/**"])
        XCTAssertEqual(options.leftPath, "a")
        XCTAssertEqual(options.rightPath, "b")
    }

    func testLongIncludeAcceptsInlineAndSeparateValues() {
        guard case .options(let options) = CLIOptionsParser.parse([
            "--include=*.swift", "--include", "src/**", "a", "b",
        ]) else {
            return XCTFail()
        }
        XCTAssertEqual(options.userIncludePatterns, ["*.swift", "src/**"])
    }

    func testPresetOptionIsOptInAndRepeatable() {
        guard case .options(let options) = CLIOptionsParser.parse([
            "--preset", "build-artifacts", "--preset=web-nodejs", "a", "b",
        ]) else {
            return XCTFail()
        }
        XCTAssertEqual(options.filterPresets, ["build-artifacts", "web-nodejs"])
    }

    func testUnknownPresetIsError() {
        XCTAssertEqual(
            CLIOptionsParser.parse(["--preset", "bogus", "a", "b"]),
            .error(.unknownPreset("bogus")))
    }

    func testMissingIncludeValueIsError() {
        XCTAssertEqual(CLIOptionsParser.parse(["-i"]), .error(.missingValue("-i")))
        XCTAssertEqual(CLIOptionsParser.parse(["a", "b", "--include"]), .error(.missingValue("--include")))
        XCTAssertEqual(CLIOptionsParser.parse(["a", "b", "--preset"]), .error(.missingValue("--preset")))
    }

    func testIncludeWithSlashSwitchAndAbsolutePathsStillParse() {
        guard case .options(let options) = CLIOptionsParser.parse([
            "/r", "-i", "*.swift", "/tmp/left", "/tmp/right",
        ]) else {
            return XCTFail("スラッシュスイッチ/絶対パスと併用できるべき")
        }
        XCTAssertTrue(options.recursive)
        XCTAssertEqual(options.userIncludePatterns, ["*.swift"])
        XCTAssertEqual(options.leftPath, "/tmp/left")
        XCTAssertEqual(options.rightPath, "/tmp/right")
    }

    // MARK: - FolderComparator.Optionsへの伝播(純粋関数)

    func testComparatorOptionsPropagatesIncludes() {
        var options = CLIOptions(leftPath: "a", rightPath: "b")
        options.userIncludePatterns = ["*.swift"]
        let comparatorOptions = CLIRunner.comparatorOptions(from: options)
        XCTAssertEqual(comparatorOptions.userIncludePatterns, ["*.swift"])
        XCTAssertTrue(comparatorOptions.userExcludePatterns.isEmpty)
    }

    func testComparatorOptionsExpandsPresetsIntoExcludes() {
        var options = CLIOptions(leftPath: "a", rightPath: "b")
        options.filterPresets = ["source-control", "macos-metadata"]
        let comparatorOptions = CLIRunner.comparatorOptions(from: options)
        XCTAssertTrue(comparatorOptions.userExcludePatterns.contains(".git"))
        XCTAssertTrue(comparatorOptions.userExcludePatterns.contains(".DS_Store"))
        XCTAssertTrue(comparatorOptions.userIncludePatterns.isEmpty)
    }

    func testComparatorOptionsKeepsDefaultExcludesFlag() {
        var options = CLIOptions(leftPath: "a", rightPath: "b")
        options.useDefaultExcludes = false
        options.recursive = true
        let comparatorOptions = CLIRunner.comparatorOptions(from: options)
        XCTAssertFalse(comparatorOptions.useDefaultExcludes)
        XCTAssertTrue(comparatorOptions.recursive)
        XCTAssertEqual(comparatorOptions.maxEntries, nil)
    }

    func testComparatorOptionsPresetExcludesMergeWithUserExcludes() {
        var options = CLIOptions(leftPath: "a", rightPath: "b")
        options.userExcludePatterns = ["*.tmp"]
        options.filterPresets = ["build-artifacts"]
        let comparatorOptions = CLIRunner.comparatorOptions(from: options)
        XCTAssertTrue(comparatorOptions.userExcludePatterns.contains("*.tmp"))
        XCTAssertTrue(comparatorOptions.userExcludePatterns.contains("build"))
    }

    func testComparatorOptionsUnselectedPresetDoesNothing() {
        let options = CLIOptions(leftPath: "a", rightPath: "b")
        let comparatorOptions = CLIRunner.comparatorOptions(from: options)
        XCTAssertTrue(comparatorOptions.userExcludePatterns.isEmpty)
        XCTAssertTrue(comparatorOptions.userIncludePatterns.isEmpty)
    }
}
