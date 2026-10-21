// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: CLI終了コードの判定(CLIExitCode.forSummary)の純粋テスト。
// 0=全一致 / 1=差異・片側のみ・エラー / 2=引数エラー / 3=処理エラーを検証する。

import XCTest
@testable import DiffLoupeCLI
@testable import DiffLoupeCore

final class ExitCodeTests: XCTestCase {

    func testConstants() {
        XCTAssertEqual(CLIExitCode.success, 0)
        XCTAssertEqual(CLIExitCode.differences, 1)
        XCTAssertEqual(CLIExitCode.usageError, 2)
        XCTAssertEqual(CLIExitCode.failure, 3)
    }

    func testAllIdenticalReturnsSuccess() {
        let summary = DiffLoupeCore.ComparisonResult.Summary(
            identicalFiles: 5, differentFiles: 0,
            leftOnlyFiles: 0, rightOnlyFiles: 0, errorFiles: 0
        )
        XCTAssertEqual(CLIExitCode.forSummary(summary), CLIExitCode.success)
    }

    func testEmptyTreeReturnsSuccess() {
        XCTAssertEqual(CLIExitCode.forSummary(DiffLoupeCore.ComparisonResult.Summary()), CLIExitCode.success)
    }

    func testDifferentFilesReturnsDifferences() {
        let summary = DiffLoupeCore.ComparisonResult.Summary(
            identicalFiles: 2, differentFiles: 1,
            leftOnlyFiles: 0, rightOnlyFiles: 0, errorFiles: 0
        )
        XCTAssertEqual(CLIExitCode.forSummary(summary), CLIExitCode.differences)
    }

    func testLeftOnlyReturnsDifferences() {
        let summary = DiffLoupeCore.ComparisonResult.Summary(
            identicalFiles: 0, differentFiles: 0,
            leftOnlyFiles: 1, rightOnlyFiles: 0, errorFiles: 0
        )
        XCTAssertEqual(CLIExitCode.forSummary(summary), CLIExitCode.differences)
    }

    func testRightOnlyReturnsDifferences() {
        let summary = DiffLoupeCore.ComparisonResult.Summary(
            identicalFiles: 0, differentFiles: 0,
            leftOnlyFiles: 0, rightOnlyFiles: 3, errorFiles: 0
        )
        XCTAssertEqual(CLIExitCode.forSummary(summary), CLIExitCode.differences)
    }

    func testErrorFilesReturnsDifferences() {
        let summary = DiffLoupeCore.ComparisonResult.Summary(
            identicalFiles: 0, differentFiles: 0,
            leftOnlyFiles: 0, rightOnlyFiles: 0, errorFiles: 2
        )
        XCTAssertEqual(CLIExitCode.forSummary(summary), CLIExitCode.differences)
    }

    func testMixedReturnsDifferences() {
        let summary = DiffLoupeCore.ComparisonResult.Summary(
            identicalFiles: 10, differentFiles: 1,
            leftOnlyFiles: 1, rightOnlyFiles: 1, errorFiles: 1
        )
        XCTAssertEqual(CLIExitCode.forSummary(summary), CLIExitCode.differences)
    }

    // MARK: - /enableexitcode のWinMerge互換写像(0=一致/1=差異/2=エラー)

    func testWinMergeCodeMapping() {
        XCTAssertEqual(CLIExitCode.winMergeCode(for: CLIExitCode.success), 0)
        XCTAssertEqual(CLIExitCode.winMergeCode(for: CLIExitCode.differences), 1)
        // diffloupe固有のエラー系(2=引数エラー/3=処理エラー)はWinMergeの2(エラー)へ縮約
        XCTAssertEqual(CLIExitCode.winMergeCode(for: CLIExitCode.usageError), 2)
        XCTAssertEqual(CLIExitCode.winMergeCode(for: CLIExitCode.failure), 2)
    }
}
