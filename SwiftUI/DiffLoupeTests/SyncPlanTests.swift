// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: Xcodeアプリターゲットから見たSyncPlanの振る舞いを検証する。
// 実装はDiffLoupeCore(CoreTests/SyncPlanTests.swift)と同一の純粋ロジックのため、
// ここでは方向判定とサマリの最低限を確認する。

import XCTest
@testable import DiffLoupe

final class SyncPlanTests: XCTestCase {

    private func makeResult() -> ComparisonResult {
        let left = URL(fileURLWithPath: "/tmp/left", isDirectory: true)
        let right = URL(fileURLWithPath: "/tmp/right", isDirectory: true)

        func entry(
            _ path: String, _ status: DiffStatus, isDirectory: Bool = false,
            leftSize: Int64 = 0, rightSize: Int64 = 0, errorMessage: String? = nil
        ) -> ComparedEntry {
            ComparedEntry(
                relativePath: path,
                name: (path as NSString).lastPathComponent,
                isDirectory: isDirectory,
                status: status,
                leftURL: left.appendingPathComponent(path),
                rightURL: right.appendingPathComponent(path),
                leftSize: leftSize,
                rightSize: rightSize,
                errorMessage: errorMessage,
                leftModificationDate: nil,
                rightModificationDate: nil,
                leftPOSIX: nil,
                rightPOSIX: nil,
                renameCandidate: nil
            )
        }

        let entries = [
            entry("same.txt", .identical),
            entry("changed.txt", .different, leftSize: 100, rightSize: 100),
            entry("left_only.txt", .leftOnly, leftSize: 42),
            entry("right_only.txt", .rightOnly, rightSize: 7),
            entry("unreadable.txt", .error, errorMessage: "読み込み不可"),
        ]
        return ComparisonResult(
            leftRoot: left,
            rightRoot: right,
            entries: entries,
            entriesByPath: Dictionary(uniqueKeysWithValues: entries.map { ($0.relativePath, $0) }),
            rootNode: DiffNode(id: "", name: "", isDirectory: true, status: .different, children: nil),
            summary: ComparisonResult.Summary(
                identicalFiles: 1, differentFiles: 1,
                leftOnlyFiles: 1, rightOnlyFiles: 1, errorFiles: 1)
        )
    }

    func testLeftToRightCopiesLeftOnlyAndPreservesRightOnly() {
        let plan = SyncPlan.make(from: makeResult(), direction: .leftToRight)
        let byPath = Dictionary(uniqueKeysWithValues: plan.items.map { ($0.relativePath, $0) })

        XCTAssertEqual(byPath["left_only.txt"]?.action, .copyFile)
        XCTAssertEqual(byPath["right_only.txt"]?.action, .preservedDestination)
        XCTAssertEqual(byPath["changed.txt"]?.action, .conflict)
        XCTAssertEqual(byPath["unreadable.txt"]?.action, .blocked)
        XCTAssertNil(byPath["same.txt"])
    }

    func testRightToLeftSwapsSourceAndDestination() {
        let plan = SyncPlan.make(from: makeResult(), direction: .rightToLeft)

        XCTAssertEqual(plan.sourceRoot.path, "/tmp/right")
        XCTAssertEqual(plan.destinationRoot.path, "/tmp/left")

        let byPath = Dictionary(uniqueKeysWithValues: plan.items.map { ($0.relativePath, $0) })
        XCTAssertEqual(byPath["right_only.txt"]?.action, .copyFile)
        XCTAssertEqual(byPath["left_only.txt"]?.action, .preservedDestination)
        XCTAssertEqual(byPath["changed.txt"]?.source?.path, "/tmp/right/changed.txt")
        XCTAssertEqual(byPath["changed.txt"]?.destination?.path, "/tmp/left/changed.txt")
    }

    func testSummaryCountsByAction() {
        let summary = SyncPlan.make(from: makeResult(), direction: .leftToRight).summary
        XCTAssertEqual(summary.copyFiles, 1)
        XCTAssertEqual(summary.conflicts, 1)
        XCTAssertEqual(summary.preservedDestinations, 1)
        XCTAssertEqual(summary.blocked, 1)
        XCTAssertEqual(summary.totalBytes, 42)
    }
}
