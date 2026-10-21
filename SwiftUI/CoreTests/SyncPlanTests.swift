// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: SyncPlan.make(from:direction:)の純粋ロジックを検証する。
// ファイルIO・コピー・削除・上書きは行わない(プレビュー専用であることの確認込み)。

import XCTest
@testable import DiffLoupeCore

final class SyncPlanTests: XCTestCase {

    /// 各状態を1つずつ含む最小のComparisonResultを構築する。
    /// leftOnlyFile/dirOnlyは「コピー元のみ」、rightOnlyFileは「コピー先のみ」に置く。
    private func makeResult() -> DiffLoupeCore.ComparisonResult {
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
            entry("left_only_dir", .leftOnly, isDirectory: true),
            entry("right_only.txt", .rightOnly, rightSize: 7),
            entry("unreadable.txt", .error, errorMessage: "読み込み不可"),
        ]
        return DiffLoupeCore.ComparisonResult(
            leftRoot: left,
            rightRoot: right,
            entries: entries,
            entriesByPath: Dictionary(uniqueKeysWithValues: entries.map { ($0.relativePath, $0) }),
            rootNode: DiffNode(id: "", name: "", isDirectory: true, status: .different, children: nil),
            summary: DiffLoupeCore.ComparisonResult.Summary(
                identicalFiles: 1, differentFiles: 1,
                leftOnlyFiles: 2, rightOnlyFiles: 1, errorFiles: 1)
        )
    }

    // MARK: - leftToRight

    func testLeftToRightMapsEachStatus() {
        let plan = SyncPlan.make(from: makeResult(), direction: .leftToRight)

        XCTAssertEqual(plan.direction, .leftToRight)
        XCTAssertEqual(plan.sourceRoot.path, "/tmp/left")
        XCTAssertEqual(plan.destinationRoot.path, "/tmp/right")

        let byPath = Dictionary(uniqueKeysWithValues: plan.items.map { ($0.relativePath, $0) })

        // identicalは対象外
        XCTAssertNil(byPath["same.txt"])

        // コピー元のみファイル → copyFile
        let copy = byPath["left_only.txt"]
        XCTAssertEqual(copy?.action, .copyFile)
        XCTAssertEqual(copy?.source?.path, "/tmp/left/left_only.txt")
        XCTAssertEqual(copy?.destination?.path, "/tmp/right/left_only.txt")
        XCTAssertEqual(copy?.bytes, 42)

        // コピー元のみディレクトリ → createDirectory
        let dir = byPath["left_only_dir"]
        XCTAssertEqual(dir?.action, .createDirectory)
        XCTAssertEqual(dir?.source?.path, "/tmp/left/left_only_dir")
        XCTAssertEqual(dir?.destination?.path, "/tmp/right/left_only_dir")

        // コピー先のみ → preservedDestination(source nil・削除しない)
        let preserved = byPath["right_only.txt"]
        XCTAssertEqual(preserved?.action, .preservedDestination)
        XCTAssertNil(preserved?.source)
        XCTAssertEqual(preserved?.destination?.path, "/tmp/right/right_only.txt")

        // 差分ファイル → conflict
        let conflict = byPath["changed.txt"]
        XCTAssertEqual(conflict?.action, .conflict)
        XCTAssertEqual(conflict?.source?.path, "/tmp/left/changed.txt")
        XCTAssertEqual(conflict?.destination?.path, "/tmp/right/changed.txt")

        // 比較エラー → blocked(reasonはエラーメッセージ)
        let blocked = byPath["unreadable.txt"]
        XCTAssertEqual(blocked?.action, .blocked)
        XCTAssertEqual(blocked?.reason, "読み込み不可")
    }

    func testLeftToRightSummary() {
        let summary = SyncPlan.make(from: makeResult(), direction: .leftToRight).summary
        XCTAssertEqual(summary.copyFiles, 1)
        XCTAssertEqual(summary.createDirectories, 1)
        XCTAssertEqual(summary.conflicts, 1)
        XCTAssertEqual(summary.preservedDestinations, 1)
        XCTAssertEqual(summary.blocked, 1)
        XCTAssertEqual(summary.totalBytes, 42)
        XCTAssertEqual(summary.totalItems, 5)
    }

    // MARK: - rightToLeft(方向反転)

    func testRightToLeftMapsEachStatus() {
        let plan = SyncPlan.make(from: makeResult(), direction: .rightToLeft)

        XCTAssertEqual(plan.direction, .rightToLeft)
        XCTAssertEqual(plan.sourceRoot.path, "/tmp/right")
        XCTAssertEqual(plan.destinationRoot.path, "/tmp/left")

        let byPath = Dictionary(uniqueKeysWithValues: plan.items.map { ($0.relativePath, $0) })

        // 右のみ(コピー元のみ扱いになる) → copyFile
        let copy = byPath["right_only.txt"]
        XCTAssertEqual(copy?.action, .copyFile)
        XCTAssertEqual(copy?.source?.path, "/tmp/right/right_only.txt")
        XCTAssertEqual(copy?.destination?.path, "/tmp/left/right_only.txt")
        XCTAssertEqual(copy?.bytes, 7)

        // 左のみ → preservedDestination
        XCTAssertEqual(byPath["left_only.txt"]?.action, .preservedDestination)
        XCTAssertNil(byPath["left_only.txt"]?.source)

        // 差分 → conflict(source/destinationが反転)
        let conflict = byPath["changed.txt"]
        XCTAssertEqual(conflict?.action, .conflict)
        XCTAssertEqual(conflict?.source?.path, "/tmp/right/changed.txt")
        XCTAssertEqual(conflict?.destination?.path, "/tmp/left/changed.txt")

        // エラー → blocked
        XCTAssertEqual(byPath["unreadable.txt"]?.action, .blocked)
    }

    func testRightToLeftSummaryBytes() {
        let summary = SyncPlan.make(from: makeResult(), direction: .rightToLeft).summary
        XCTAssertEqual(summary.copyFiles, 1)
        XCTAssertEqual(summary.preservedDestinations, 2)
        XCTAssertEqual(summary.totalBytes, 7)
    }

    // MARK: - Identifiable/Equatable/Hashable

    func testItemsAreIdentifiableAndDirectionAware() {
        let plan = SyncPlan.make(from: makeResult(), direction: .leftToRight)
        let ids = plan.items.map(\.id)
        XCTAssertEqual(Set(ids).count, ids.count, "idはdirection込みで一意であるべき")

        // 同一パスでも方向が異なれば別id(左右切替時のSwiftUI衝突防止)
        let other = SyncPlan.make(from: makeResult(), direction: .rightToLeft)
        let rightIds = Set(other.items.map(\.id))
        XCTAssertTrue(ids.allSatisfy { !rightIds.contains($0) })
    }

    func testSyncItemIsEquatableAndHashable() {
        let a = SyncItem(
            direction: .leftToRight, relativePath: "x.txt", action: .copyFile,
            source: URL(fileURLWithPath: "/a/x.txt"),
            destination: URL(fileURLWithPath: "/b/x.txt"), bytes: 1, reason: "")
        let b = SyncItem(
            direction: .leftToRight, relativePath: "x.txt", action: .copyFile,
            source: URL(fileURLWithPath: "/a/x.txt"),
            destination: URL(fileURLWithPath: "/b/x.txt"), bytes: 1, reason: "")
        XCTAssertEqual(a, b)
        XCTAssertEqual(Set([a, b]).count, 1)
    }
}
