// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: レポートの文字列化とファイル書き込み。既存のComparisonReportを再利用し、
// ファイル書き込みは「親ディレクトリ存在」と「既存ファイルの上書き拒否」を保証する(CLIポリシー)。

import Foundation
import DiffLoupeCore

enum CLIOutputError: LocalizedError, Equatable {
    /// 既存ファイルへの上書きはCLIでは明示的に拒否する
    case fileExists(String)
    /// 出力先の親ディレクトリが存在しない
    case parentDirectoryMissing(String)

    var errorDescription: String? {
        switch self {
        case .fileExists(let path):
            return "出力先が既に存在します(上書きは拒否されます): \(path)"
        case .parentDirectoryMissing(let path):
            return "出力先の親ディレクトリが存在しません: \(path)"
        }
    }
}

enum CLIReporter {

    /// 比較結果を指定形式の文字列に整形する(既存の純粋フォーマッタを再利用)。
    static func reportText(result: DiffLoupeCore.ComparisonResult, format: ReportFormat) -> String {
        ComparisonReport(result: result).render(as: format)
    }

    /// 出力先の事前検証。親ディレクトリの存在と既存ファイルの有無を確認する。
    static func validateOutput(path: String) throws {
        let url = URL(fileURLWithPath: path)
        let parent = url.deletingLastPathComponent()
        var isDirectory: ObjCBool = false
        guard FileManager.default.fileExists(atPath: parent.path, isDirectory: &isDirectory),
              isDirectory.boolValue else {
            throw CLIOutputError.parentDirectoryMissing(path)
        }
        guard !FileManager.default.fileExists(atPath: url.path) else {
            throw CLIOutputError.fileExists(path)
        }
    }

    /// レポートをファイルへ書き込む。既存ファイルの上書きは拒否し、
    /// 親ディレクトリが存在しなければ安全にエラーを投げる。
    static func write(_ text: String, to path: String) throws {
        try validateOutput(path: path)
        let url = URL(fileURLWithPath: path)
        try text.write(to: url, atomically: true, encoding: .utf8)
    }
}
