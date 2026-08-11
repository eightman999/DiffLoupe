// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: CLIOptionsParser(引数解析)の純粋テスト。
// 既定値・各オプション・形式推測・レポート出力先決定を検証する。

import XCTest
@testable import DiffLoupeCLI
@testable import DiffLoupeCore

final class CLIOptionsTests: XCTestCase {

    // MARK: - パース

    func testOnlyPathsUseDefaults() {
        let result = CLIOptionsParser.parse(["left/", "right/"])
        guard case .options(let options) = result else {
            return XCTFail("expected .options, got \(result)")
        }
        XCTAssertEqual(options.leftPath, "left/")
        XCTAssertEqual(options.rightPath, "right/")
        // WinMerge互換: 既定は非再帰(WinMergeは /r 指定時のみ再帰)
        XCTAssertFalse(options.recursive)
        XCTAssertTrue(options.useDefaultExcludes)
        XCTAssertTrue(options.userExcludePatterns.isEmpty)
        XCTAssertTrue(options.contentComparison.isStrict)
        XCTAssertFalse(options.detectRenames)
        XCTAssertNil(options.reportFormat)
        XCTAssertNil(options.outputPath)
        XCTAssertFalse(options.enableExitCode)
        XCTAssertFalse(options.quiet)
    }

    func testLongOptions() {
        let result = CLIOptionsParser.parse([
            "--no-default-excludes", "--detect-renames",
            "--ignore-line-endings", "--ignore-trailing-whitespace",
            "--normalize-whitespace", "--ignore-case",
            "--quiet", "a", "b",
        ])
        guard case .options(let options) = result else {
            return XCTFail("expected .options, got \(result)")
        }
        XCTAssertFalse(options.useDefaultExcludes)
        XCTAssertTrue(options.detectRenames)
        XCTAssertTrue(options.contentComparison.ignoreLineEndings)
        XCTAssertTrue(options.contentComparison.ignoreTrailingWhitespace)
        XCTAssertTrue(options.contentComparison.normalizeWhitespace)
        XCTAssertTrue(options.contentComparison.ignoreCase)
        XCTAssertFalse(options.contentComparison.isStrict)
        XCTAssertTrue(options.quiet)
    }

    func testShortOptionsAndInlineValues() {
        let result = CLIOptionsParser.parse([
            "-x", "-e", "*.tmp", "--exclude=build/", "-e", "*.log",
            "-f", "json", "-o", "/tmp/report.json", "-q", "a", "b",
        ])
        guard case .options(let options) = result else {
            return XCTFail("expected .options, got \(result)")
        }
        XCTAssertFalse(options.useDefaultExcludes)
        XCTAssertEqual(options.userExcludePatterns, ["*.tmp", "build/", "*.log"])
        XCTAssertEqual(options.reportFormat, .json)
        XCTAssertEqual(options.outputPath, "/tmp/report.json")
        XCTAssertTrue(options.quiet)
    }

    func testRecursiveShortAndLongAreAccepted() {
        guard case .options(let o) = CLIOptionsParser.parse(["-r", "a", "b"]) else {
            return XCTFail()
        }
        XCTAssertTrue(o.recursive)
        guard case .options(let o2) = CLIOptionsParser.parse(["--recursive", "a", "b"]) else {
            return XCTFail()
        }
        XCTAssertTrue(o2.recursive)
    }

    func testNoRecursiveDisablesRecursion() {
        // 既定は非再帰なので、まず有効化→無効化で最終的にfalseになること
        guard case .options(let o) = CLIOptionsParser.parse(["-r", "--no-recursive", "a", "b"]) else {
            return XCTFail()
        }
        XCTAssertFalse(o.recursive)
        // 非再帰既定の状態で明示指定してもfalseのまま
        guard case .options(let o2) = CLIOptionsParser.parse(["--no-recursive", "a", "b"]) else {
            return XCTFail()
        }
        XCTAssertFalse(o2.recursive)
    }

    // MARK: - WinMergeスラッシュスイッチ

    func testWinMergeSlashSwitches() {
        // /r は再帰有効化
        guard case .options(let o) = CLIOptionsParser.parse(["/r", "a", "b"]) else {
            return XCTFail()
        }
        XCTAssertTrue(o.recursive)
        // /r- は再帰無効化(既定の非再帰を維持)
        guard case .options(let o2) = CLIOptionsParser.parse(["/r-", "a", "b"]) else {
            return XCTFail()
        }
        XCTAssertFalse(o2.recursive)
        // 最後に指定した方が勝つ
        guard case .options(let o3) = CLIOptionsParser.parse(["/r", "/r-", "a", "b"]) else {
            return XCTFail()
        }
        XCTAssertFalse(o3.recursive)
    }

    func testWinMergeSlashSwitchCaseInsensitive() {
        guard case .options(let o) = CLIOptionsParser.parse(["/R", "a", "b"]) else {
            return XCTFail()
        }
        XCTAssertTrue(o.recursive)
        guard case .options(let o2) = CLIOptionsParser.parse(["/ENABLEEXITCODE", "a", "b"]) else {
            return XCTFail()
        }
        XCTAssertTrue(o2.enableExitCode)
    }

    func testWinMergeNoOpAndQuietSwitches() {
        // GUI専用・MRU系は受理のみ(無操作)でエラーにならない
        let noOps = ["/u", "/ul", "/um", "/ur", "/wl", "/wm", "/wr",
                     "/minimize", "/maximize", "/noninteractive"]
        for flag in noOps {
            let parsed = CLIOptionsParser.parse([flag, "a", "b"])
            guard case .options(let o) = parsed else {
                return XCTFail("\(flag) は受理されるべき: \(parsed)")
            }
            XCTAssertFalse(o.quiet, "\(flag) はquietを変更しないこと")
            XCTAssertFalse(o.recursive, "\(flag) はrecursiveを変更しないこと")
        }
        // /x /xq は -q 相当(進捗抑止)へマップ
        for flag in ["/x", "/xq"] {
            guard case .options(let o) = CLIOptionsParser.parse([flag, "a", "b"]) else {
                return XCTFail("\(flag) は受理されるべき")
            }
            XCTAssertTrue(o.quiet)
        }
    }

    func testWinMergeLabelSwitchesConsumeValue() {
        guard case .options(let o) = CLIOptionsParser.parse(["/dl", "左", "/dm", "中", "/dr", "右", "a", "b"]) else {
            return XCTFail()
        }
        XCTAssertEqual(o.leftPath, "a")
        XCTAssertEqual(o.rightPath, "b")
        // 値が無い場合はエラー
        XCTAssertEqual(CLIOptionsParser.parse(["/dl"]), .error(.missingValue("/dl")))
    }

    func testEnableExitCodeSwitch() {
        guard case .options(let o) = CLIOptionsParser.parse(["/enableexitcode", "a", "b"]) else {
            return XCTFail()
        }
        XCTAssertTrue(o.enableExitCode)
    }

    func testOrSwitchSetsOutputPathAndInference() {
        // /or は出力ファイルへ。拡張子から形式を推測
        guard case .options(let o) = CLIOptionsParser.parse(["/or", "/tmp/report.csv", "a", "b"]) else {
            return XCTFail()
        }
        XCTAssertEqual(o.outputPath, "/tmp/report.csv")
        XCTAssertTrue(o.outputFromWinMerge)
        XCTAssertEqual(
            try? CLIOptionsParser.reportTarget(options: o).get(),
            .file(path: "/tmp/report.csv", format: .csv)
        )

        // 拡張子不明のときはHTMLへフォールバック(既存 -o のエラー挙動とは区別)
        guard case .options(let o2) = CLIOptionsParser.parse(["/or", "/tmp/report.txt", "a", "b"]) else {
            return XCTFail()
        }
        XCTAssertEqual(
            try? CLIOptionsParser.reportTarget(options: o2).get(),
            .file(path: "/tmp/report.txt", format: .html)
        )
    }

    func testAbsolutePathsAreNotConfusedWithSlashSwitches() {
        // 既知スイッチ名と完全一致しない絶対POSIXパスは位置引数のまま
        guard case .options(let o) = CLIOptionsParser.parse(["/tmp/left", "/tmp/right"]) else {
            return XCTFail()
        }
        XCTAssertEqual(o.leftPath, "/tmp/left")
        XCTAssertEqual(o.rightPath, "/tmp/right")
        XCTAssertFalse(o.recursive)
    }

    func testUnknownSlashArgumentIsNotASwitch() {
        // 未知の/から始まる引数は位置引数(絶対パス等の誤認防止)
        let result = CLIOptionsParser.parse(["/somewhere", "/elsewhere"])
        guard case .options(let o) = result else {
            return XCTFail()
        }
        XCTAssertEqual(o.leftPath, "/somewhere")
        XCTAssertEqual(o.rightPath, "/elsewhere")
    }

    func testDoubleDashTerminatesOptions() {
        let result = CLIOptionsParser.parse(["--", "-weird", "dir"])
        guard case .options(let options) = result else {
            return XCTFail("expected .options, got \(result)")
        }
        XCTAssertEqual(options.leftPath, "-weird")
        XCTAssertEqual(options.rightPath, "dir")
    }

    // MARK: - エラー系

    func testMissingPathsIsError() {
        XCTAssertEqual(CLIOptionsParser.parse([]), .error(.missingPaths))
        XCTAssertEqual(CLIOptionsParser.parse(["only-left"]), .error(.missingPaths))
    }

    func testExtraArgumentsIsError() {
        guard case .error(let error) = CLIOptionsParser.parse(["a", "b", "c", "d"]) else {
            return XCTFail("expected .error")
        }
        XCTAssertEqual(error, .extraArguments(["c", "d"]))
    }

    func testUnknownOptionIsError() {
        XCTAssertEqual(CLIOptionsParser.parse(["--nope", "a", "b"]), .error(.unknownOption("--nope")))
        XCTAssertEqual(CLIOptionsParser.parse(["-z", "a", "b"]), .error(.unknownOption("-z")))
    }

    func testMissingValueIsError() {
        XCTAssertEqual(CLIOptionsParser.parse(["--exclude"]), .error(.missingValue("--exclude")))
        XCTAssertEqual(CLIOptionsParser.parse(["-e"]), .error(.missingValue("-e")))
        XCTAssertEqual(CLIOptionsParser.parse(["a", "b", "--format"]), .error(.missingValue("--format")))
        XCTAssertEqual(CLIOptionsParser.parse(["a", "b", "--structured"]), .error(.missingValue("--structured")))
    }

    func testInvalidFormatIsError() {
        XCTAssertEqual(CLIOptionsParser.parse(["-f", "yaml", "a", "b"]), .error(.invalidFormat("yaml")))
        XCTAssertEqual(CLIOptionsParser.parse(["--format=pdf", "a", "b"]), .error(.invalidFormat("pdf")))
    }

    // MARK: - 構造化比較モード

    func testStructuredModeDefaultDisabled() {
        guard case .options(let options) = CLIOptionsParser.parse(["a", "b"]) else {
            return XCTFail()
        }
        XCTAssertEqual(options.structuredMode, .disabled)
    }

    func testStructuredModeParsing() {
        for (flag, expected) in [
            ("off", StructuredComparisonMode.disabled),
            ("disabled", StructuredComparisonMode.disabled),
            ("auto", StructuredComparisonMode.automatic),
            ("automatic", StructuredComparisonMode.automatic),
            ("json", StructuredComparisonMode.json),
            ("xml", StructuredComparisonMode.xml),
            ("csv", StructuredComparisonMode.csv),
        ] {
            guard case .options(let o) = CLIOptionsParser.parse(["--structured", flag, "a", "b"]) else {
                return XCTFail("--structured \(flag) は受理されるべき")
            }
            XCTAssertEqual(o.structuredMode, expected, "--structured \(flag)")
        }
        // 大文字小文字を区別しない
        guard case .options(let o) = CLIOptionsParser.parse(["--structured", "JSON", "a", "b"]) else {
            return XCTFail()
        }
        XCTAssertEqual(o.structuredMode, .json)
        // --structured=value のインライン形式
        guard case .options(let o2) = CLIOptionsParser.parse(["--structured=auto", "a", "b"]) else {
            return XCTFail()
        }
        XCTAssertEqual(o2.structuredMode, .automatic)
    }

    func testStructuredModeAliases() {
        guard case .options(let json) = CLIOptionsParser.parse(["--normalize-json", "a", "b"]) else {
            return XCTFail()
        }
        XCTAssertEqual(json.structuredMode, .json)
        guard case .options(let xml) = CLIOptionsParser.parse(["--normalize-xml", "a", "b"]) else {
            return XCTFail()
        }
        XCTAssertEqual(xml.structuredMode, .xml)
        guard case .options(let csv) = CLIOptionsParser.parse(["--table-csv", "a", "b"]) else {
            return XCTFail()
        }
        XCTAssertEqual(csv.structuredMode, .csv)
    }

    func testInvalidStructuredModeIsError() {
        XCTAssertEqual(
            CLIOptionsParser.parse(["--structured", "yaml", "a", "b"]),
            .error(.invalidStructuredMode("yaml"))
        )
        XCTAssertEqual(
            CLIOptionsParser.parse(["--structured=bogus", "a", "b"]),
            .error(.invalidStructuredMode("bogus"))
        )
    }

    func testParseStructuredMode() {
        XCTAssertEqual(CLIOptionsParser.parseStructuredMode("off"), .disabled)
        XCTAssertEqual(CLIOptionsParser.parseStructuredMode("AUTO"), .automatic)
        XCTAssertEqual(CLIOptionsParser.parseStructuredMode("json"), .json)
        XCTAssertEqual(CLIOptionsParser.parseStructuredMode("Xml"), .xml)
        XCTAssertEqual(CLIOptionsParser.parseStructuredMode("csv"), .csv)
        XCTAssertNil(CLIOptionsParser.parseStructuredMode("yaml"))
    }

    func testHelpAndVersionPrecedePathValidation() {
        XCTAssertEqual(CLIOptionsParser.parse(["-h"]), .help)
        XCTAssertEqual(CLIOptionsParser.parse(["--help", "a", "b"]), .help)
        XCTAssertEqual(CLIOptionsParser.parse(["-V"]), .version)
        XCTAssertEqual(CLIOptionsParser.parse(["--version"]), .version)
    }

    // MARK: - 形式マッピング

    func testParseFormat() {
        XCTAssertEqual(CLIOptionsParser.parseFormat("json"), .json)
        XCTAssertEqual(CLIOptionsParser.parseFormat("JSON"), .json)
        XCTAssertEqual(CLIOptionsParser.parseFormat("csv"), .csv)
        XCTAssertEqual(CLIOptionsParser.parseFormat("md"), .markdown)
        XCTAssertEqual(CLIOptionsParser.parseFormat("markdown"), .markdown)
        XCTAssertEqual(CLIOptionsParser.parseFormat("html"), .html)
        XCTAssertNil(CLIOptionsParser.parseFormat("txt"))
    }

    func testFormatForOutputPath() {
        XCTAssertEqual(CLIOptionsParser.formatForOutputPath("r.json"), .json)
        XCTAssertEqual(CLIOptionsParser.formatForOutputPath("report.CSV"), .csv)
        XCTAssertEqual(CLIOptionsParser.formatForOutputPath("r.md"), .markdown)
        XCTAssertEqual(CLIOptionsParser.formatForOutputPath("r.markdown"), .markdown)
        XCTAssertEqual(CLIOptionsParser.formatForOutputPath("r.html"), .html)
        XCTAssertNil(CLIOptionsParser.formatForOutputPath("r.txt"))
    }

    // MARK: - レポート出力先決定(純粋)

    func testReportTargetNoneWhenUnspecified() {
        let options = CLIOptions(leftPath: "a", rightPath: "b")
        let target = try? CLIOptionsParser.reportTarget(options: options).get()
        XCTAssertEqual(target, CLIReportTarget.none)
    }

    func testReportTargetStdoutWhenFormatOnly() {
        var options = CLIOptions(leftPath: "a", rightPath: "b")
        options.reportFormat = .json
        XCTAssertEqual(try? CLIOptionsParser.reportTarget(options: options).get(), .stdout(.json))
    }

    func testReportTargetFileInferringFormatFromExtension() {
        var options = CLIOptions(leftPath: "a", rightPath: "b")
        options.outputPath = "/tmp/report.csv"
        XCTAssertEqual(try? CLIOptionsParser.reportTarget(options: options).get(), .file(path: "/tmp/report.csv", format: .csv))
    }

    func testReportTargetFileWithExplicitFormat() {
        var options = CLIOptions(leftPath: "a", rightPath: "b")
        options.outputPath = "/tmp/report.csv"
        options.reportFormat = .markdown
        XCTAssertEqual(try? CLIOptionsParser.reportTarget(options: options).get(), .file(path: "/tmp/report.csv", format: .markdown))
    }

    func testReportTargetRejectsUnknownExtension() {
        var options = CLIOptions(leftPath: "a", rightPath: "b")
        options.outputPath = "/tmp/report.txt"
        XCTAssertEqual(CLIOptionsParser.reportTarget(options: options), .failure(.outputExtensionUnknown("/tmp/report.txt")))
    }
}
