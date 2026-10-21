// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: diffloupe CLIの実行制御。引数解析→パス/出力検証→FolderComparatorで比較→
// レポート/サマリ出力→終了コード決定を担う。Foundation.exitを呼ばず終了コードを返すため、
// テスト・swift runの実行環境を壊さない(mainのみがexitする)。

import Foundation
import DiffLoupeCore

public enum CLIRunner {

    /// コマンドを実行し終了コードを返す。
    /// - Parameters:
    ///   - arguments: CommandLine.argumentsから実行ファイル名を除いたもの
    ///   - cancellation: シグナル等から比較タスクをキャンセルするためのトークン
    public static func run(arguments: [String], cancellation: CancellationToken) async -> Int32 {
        switch CLIOptionsParser.parse(arguments) {
        case .help:
            CLIO.standardOutput(CLIOptionsParser.usageText)
            return CLIExitCode.success
        case .version:
            CLIO.standardOutput(CLIVersion.text)
            return CLIExitCode.success
        case .error(let parseError):
            CLIO.standardError("diffloupe: \(CLIError.message(for: parseError))")
            CLIO.standardError("`diffloupe --help` で使い方を確認してください。")
            return CLIExitCode.usageError
        case .options(let options):
            let code = await runComparison(options: options, cancellation: cancellation)
            // /enableexitcode 指定時はWinMerge互換の3値(0/1/2)へ写像する。
            // 完全なドロップイン互換ではなく、エラー系(2/3)をまとめて 2 へ縮約する写像
            // (CLIExitCode.winMergeCode のドキュメント参照)。
            if options.enableExitCode {
                return CLIExitCode.winMergeCode(for: code)
            }
            return code
        }
    }

    // MARK: - 比較本体

    /// CLIOptionsからFolderComparator.Optionsを組み立てる(純粋・テスト可能)。
    /// 選択済みプリセット(--preset)の除外パターンをユーザー除外へ展開して追加する(opt-inのみ)。
    static func comparatorOptions(from options: CLIOptions) -> FolderComparator.Options {
        var excludePatterns = options.userExcludePatterns
        for presetID in options.filterPresets {
            if let preset = FilterPreset.preset(id: presetID) {
                excludePatterns.append(contentsOf: preset.excludePatterns)
            }
        }
        var contentComparison = options.contentComparison
        contentComparison.structuredMode = options.structuredMode
        return FolderComparator.Options(
            recursive: options.recursive,
            useDefaultExcludes: options.useDefaultExcludes,
            maxEntries: nil,
            userExcludePatterns: excludePatterns,
            userIncludePatterns: options.userIncludePatterns,
            contentComparison: contentComparison,
            detectRenames: options.detectRenames
        )
    }

    private static func runComparison(options: CLIOptions, cancellation: CancellationToken) async -> Int32 {
        // 1. パス検証(左右ともディレクトリであること)
        guard let leftPath = options.leftPath, let rightPath = options.rightPath else {
            CLIO.standardError("diffloupe: 左右のフォルダパスが指定されていません。")
            return CLIExitCode.usageError
        }
        guard isDirectory(leftPath) else {
            CLIO.standardError("diffloupe: 左パスはディレクトリではありません: \(leftPath)")
            return CLIExitCode.usageError
        }
        guard isDirectory(rightPath) else {
            CLIO.standardError("diffloupe: 右パスはディレクトリではありません: \(rightPath)")
            return CLIExitCode.usageError
        }

        // 2. レポート出力先の決定と事前検証
        let target: CLIReportTarget
        switch CLIOptionsParser.reportTarget(options: options) {
        case .success(let resolved):
            target = resolved
        case .failure(let error):
            CLIO.standardError("diffloupe: \(CLIError.message(for: error))")
            return CLIExitCode.usageError
        }
        if case .file(let path, _) = target {
            do {
                try CLIReporter.validateOutput(path: path)
            } catch {
                CLIO.standardError("diffloupe: \(error.localizedDescription)")
                return CLIExitCode.usageError
            }
        }

        // 3. 比較(既存エンジンを再利用。UI/AppKit非依存)
        let comparatorOptions = comparatorOptions(from: options)
        let quiet = options.quiet
        let compareTask = Task<DiffLoupeCore.ComparisonResult, Error> {
            try await FolderComparator().compare(
                left: URL(fileURLWithPath: leftPath, isDirectory: true),
                right: URL(fileURLWithPath: rightPath, isDirectory: true),
                options: comparatorOptions
            ) { progress in
                if !quiet { CLIProgress.print(progress) }
            }
        }
        cancellation.set { compareTask.cancel() }
        defer { cancellation.clear() }

        let result: DiffLoupeCore.ComparisonResult
        do {
            result = try await compareTask.value
        } catch is CancellationError {
            CLIO.standardError("diffloupe: 比較がキャンセルされました。")
            return CLIExitCode.failure
        } catch {
            CLIO.standardError("diffloupe: 比較に失敗しました: \(error.localizedDescription)")
            return CLIExitCode.failure
        }

        // 4. 結果出力と終了コード
        return emit(result: result, target: target)
    }

    // MARK: - 出力

    /// 人間向けサマリとレポートの出力をルーティングし、終了コードを返す。
    /// - stdoutはレポート専用(レポートをstdoutへ出す場合はサマリをstderrへ回す)
    /// - ファイル出力時は上書きを事前検証済み
    private static func emit(result: DiffLoupeCore.ComparisonResult, target: CLIReportTarget) -> Int32 {
        let exitCode = CLIExitCode.forSummary(result.summary)
        let summary = HumanSummary.render(result: result)

        switch target {
        case .none:
            CLIO.standardOutput(summary)
        case .stdout(let format):
            CLIO.standardError(summary)
            CLIO.standardOutput(CLIReporter.reportText(result: result, format: format))
        case .file(let path, let format):
            do {
                try CLIReporter.write(CLIReporter.reportText(result: result, format: format), to: path)
                CLIO.standardOutput(summary)
                CLIO.standardOutput("レポートを書き込みました: \(path)")
            } catch {
                CLIO.standardError("diffloupe: レポートの書き込みに失敗しました: \(error.localizedDescription)")
                return CLIExitCode.failure
            }
        }
        return exitCode
    }

    private static func isDirectory(_ path: String) -> Bool {
        var isDirectory: ObjCBool = false
        return FileManager.default.fileExists(atPath: path, isDirectory: &isDirectory)
            && isDirectory.boolValue
    }
}
