// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: diffloupe CLIの終了コード。WinMergeのCLI/CI運用に相当する値と、
// 比較サマリからの純粋な判定関数を定義する。

import DiffLoupeCore

enum CLIExitCode {
    /// 全ファイル一致
    static let success: Int32 = 0
    /// 差異・片側のみ・読み込みエラーあり
    static let differences: Int32 = 1
    /// 引数・パス・オプションのエラー
    static let usageError: Int32 = 2
    /// 比較処理エラーまたはキャンセル
    static let failure: Int32 = 3

    /// 比較サマリから終了コードを判定する(純粋関数)。
    /// 相違・片側のみ・エラーのいずれかが1件でもあれば1、なければ0。
    static func forSummary(_ summary: ComparisonResult.Summary) -> Int32 {
        if summary.differentFiles == 0,
           summary.leftOnlyFiles == 0,
           summary.rightOnlyFiles == 0,
           summary.errorFiles == 0 {
            return success
        }
        return differences
    }

    /// /enableexitcode(WinMerge互換)の終了コード写像(純粋関数)。
    /// WinMergeは 0=一致 / 1=差異 / 2=エラー の3値で、diffloupeの4値(0/1/2/3)から
    /// エラー系(usageError=2・failure=3)をまとめて 2 へ落とす。
    /// 注意: 完全なドロップイン互換ではない。WinMergeは設定・比較不能の細別を
    /// すべて 2 で返すが、本実装はdiffloupeの引数エラー/処理エラーの区別を残さない
    /// 3値への縮約写像に留める(既存の0/1/2/3 APIは /enableexitcode 未指定時は不変)。
    static func winMergeCode(for code: Int32) -> Int32 {
        switch code {
        case success: return 0
        case differences: return 1
        default: return 2
        }
    }
}
