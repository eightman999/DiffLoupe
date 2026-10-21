// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: CLIテストの共通ヘルパ。DiffLoupeCoreの内部型を@testable importで組み立てる。

import Foundation
@testable import DiffLoupeCore

enum TestResultFactory {

    static func entry(
        path: String,
        status: DiffStatus,
        isDirectory: Bool = false,
        errorMessage: String? = nil
    ) -> ComparedEntry {
        ComparedEntry(
            relativePath: path,
            name: (path as NSString).lastPathComponent,
            isDirectory: isDirectory,
            status: status,
            leftURL: nil,
            rightURL: nil,
            leftSize: 0,
            rightSize: 0,
            errorMessage: errorMessage,
            leftModificationDate: nil,
            rightModificationDate: nil,
            leftPOSIX: nil,
            rightPOSIX: nil,
            renameCandidate: nil
        )
    }

    static func result(entries: [ComparedEntry]) -> DiffLoupeCore.ComparisonResult {
        var summary = DiffLoupeCore.ComparisonResult.Summary()
        for e in entries where !e.isDirectory {
            switch e.status {
            case .identical: summary.identicalFiles += 1
            case .different: summary.differentFiles += 1
            case .leftOnly: summary.leftOnlyFiles += 1
            case .rightOnly: summary.rightOnlyFiles += 1
            case .error: summary.errorFiles += 1
            }
        }
        return DiffLoupeCore.ComparisonResult(
            leftRoot: URL(fileURLWithPath: "/tmp/left", isDirectory: true),
            rightRoot: URL(fileURLWithPath: "/tmp/right", isDirectory: true),
            entries: entries,
            entriesByPath: Dictionary(uniqueKeysWithValues: entries.map { ($0.relativePath, $0) }),
            rootNode: DiffNode(id: "", name: "", isDirectory: true, status: .different, children: nil),
            summary: summary
        )
    }
}
