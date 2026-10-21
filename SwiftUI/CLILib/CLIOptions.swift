// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: diffloupe CLIの引数解析。ファイルI/O・実行に依存しない純粋な型とパーサを定義する。
// パース結果はCLIOptions(バリュータイプ)としてCLIRunnerへ渡し、mainから分離してテスト可能にする。

import Foundation
import DiffLoupeCore

/// 引数解析エラー。終了コード2(引数・パス・オプションエラー)に対応する。
enum CLIParseError: Error, Equatable {
    case unknownOption(String)
    case missingValue(String)
    case missingPaths
    case extraArguments([String])
    case invalidFormat(String)
    case invalidStructuredMode(String)
    case outputExtensionUnknown(String)
    case unknownPreset(String)
}

/// 引数解析の結果。help/versionはパス指定なしで確定する。
enum CLIParseResult: Equatable {
    case options(CLIOptions)
    case help
    case version
    case error(CLIParseError)
}

/// レポートの出力先。CLIのstdout/ファイルポリシー(レポート専用stdout・既存ファイルの上書き拒否)を型で表す。
enum CLIReportTarget: Equatable {
    /// レポートなし。人間向けサマリのみをstdoutへ
    case none
    /// レポートをstdoutへ(バイナリでなくUTF-8文字列)。人間向けサマリはstderrへ
    case stdout(ReportFormat)
    /// レポートをファイルへ書き込む。既存ファイルの上書きは拒否する
    case file(path: String, format: ReportFormat)
}

/// diffloupe CLIの解析済みオプション。未指定のものは既定値を持つ。
struct CLIOptions: Equatable {
    var leftPath: String?
    var rightPath: String?

    /// 再帰比較(WinMerge /r 互換)。WinMergeは既定で非再帰のため、既定は false。
    /// -r / --recursive / /r で有効化、--no-recursive / /r- で無効化する。
    var recursive = false
    /// 既定除外(.git / .DS_Store / node_modules)を適用するか
    var useDefaultExcludes = true
    /// ユーザー定義の除外glob(複数指定可)
    var userExcludePatterns: [String] = []
    /// ユーザー定義のインクルードglob(複数指定可)。非空なら一致するファイルのみ対象
    var userIncludePatterns: [String] = []
    /// 組み込みプリセットのID(opt-in)。選択したプリセットの除外パターンが比較に追加される
    var filterPresets: [String] = []
    /// 内容比較モード(全てオフ=厳密比較)
    var contentComparison = ContentComparison()
    /// 構造化比較モード(JSON/XML/CSV。既定 disabled)
    var structuredMode: StructuredComparisonMode = .disabled
    /// リネーム/移動候補の検出
    var detectRenames = false
    /// レポート形式(未指定なら出力ファイルの拡張子から推測する)
    var reportFormat: ReportFormat?
    /// レポートの出力ファイルパス(未指定ならレポートをstdoutへ)
    var outputPath: String?
    /// /enableexitcode が指定されたか。終了コードをWinMerge互換(0/1/2)へ写像する
    var enableExitCode = false
    /// /or で出力先を指定したか(WinMerge互換)。拡張子不明時は形式をhtmlへフォールバックする
    var outputFromWinMerge = false
    /// 進捗(stderr)を抑止する
    var quiet = false
    var showHelp = false
    var showVersion = false
}

enum CLIOptionsParser {

    static let executableName = "diffloupe"

    static let usageText = """
    使い方: \(executableName) [オプション] <左フォルダ> <右フォルダ>

    左右のフォルダを比較し、結果を標準出力またはファイルへ出力します。
    既定は非再帰比較です(WinMergeで /r を指定しない場合と同じ)。
    WinMergeのCLI/CI運用に相当する終了コードを返します:
      0 = 全ファイル一致
      1 = 差異・片側のみ・読み込みエラーあり
      2 = 引数・パス・オプションのエラー
      3 = 比較処理エラーまたはキャンセル

    オプション:
      -r, --recursive                   再帰比較を有効化(既定は非再帰)
          --no-recursive                再帰比較を明示的に無効化
      -x, --no-default-excludes         既定除外(.git/.DS_Store/node_modules)を無効化
      -e, --exclude <glob>              除外glob(複数指定可。区切りなし=ファイル名、あり=相対パス)
      -i, --include <glob>              インクルードglob(複数指定可。非空時は一致するファイルのみ対象)
          --preset <name>               組み込みプリセットの除外を追加
                                        (source-control / build-artifacts / web-nodejs / macos-metadata。opt-in)
          --ignore-line-endings         改行コード(CRLF/CR/LF)を無視
          --ignore-trailing-whitespace  行末の空白を無視
          --normalize-whitespace        連続空白を単一スペースに正規化
          --ignore-case                 大文字小文字を無視
          --structured <off|auto|json|xml|csv>
                                        JSON/XML/CSV・TSVの構造化比較
                                        (キー順・属性順・引用・区切りなどの表記揺れを正規化。既定 off)
          --normalize-json              --structured json のエイリアス
          --normalize-xml               --structured xml のエイリアス
          --table-csv                   --structured csv のエイリアス
          --detect-renames              片側のみファイルのリネーム/移動候補を検出
      -f, --format <json|csv|md|html>   レポート形式(-o省略時はstdoutへ出力)
      -o, --output <パス>               レポートをファイルへ書き込む(既存ファイルの上書きは拒否)
      -q, --quiet                       進捗(stderr)を出力しない
      -h, --help                        このヘルプを表示
      -V, --version                     バージョンを表示

    WinMerge互換スラッシュスイッチ(絶対POSIXパスと混同しないよう、既知のスイッチ名にのみ反応):
      /r /r-                          再帰比較の有効化(/r)/無効化(/r-)
      /u /ul /um /ur                  MRU履歴の抑制(GUI専用のため受理のみ・無操作)
      /wl /wm /wr                     ペインの読み取り専用(GUI専用のため受理のみ・無操作)
      /minimize /maximize /noninteractive  GUI用スイッチ(受理のみ・無操作)
      /x /xq                          比較後に終了(GUI用。CLIでは -q 相当へマップ)
      /enableexitcode                 WinMerge互換の終了コード(0=一致/1=差異/2=エラー)へ写像
      /dl <ラベル> /dm <ラベル> /dr <ラベル>  表示ラベル(GUI専用のため値は無視)
      /or <パス>                      レポートをファイルへ出力(拡張子不明時はHTML)

    例:
      \(executableName) left/ right/
      \(executableName) -r left/ right/
      \(executableName) --format json left/ right/ > report.json
      \(executableName) /r /or report.html left/ right/
    """

    static func parse(_ args: [String]) -> CLIParseResult {
        var options = CLIOptions()
        var positional: [String] = []
        var optionsEnded = false
        var iterator = args.makeIterator()

        while let arg = iterator.next() {
            if optionsEnded {
                positional.append(arg)
                continue
            }
            if arg == "--" {
                optionsEnded = true
                continue
            }

            if arg.hasPrefix("--") {
                if let error = applyLongOption(
                    body: String(arg.dropFirst(2)),
                    options: &options,
                    iterator: &iterator
                ) {
                    return .error(error)
                }
            } else if arg.hasPrefix("-"), arg != "-" {
                if let error = applyShortOption(
                    flag: String(arg.dropFirst()),
                    options: &options,
                    iterator: &iterator
                ) {
                    return .error(error)
                }
            } else if arg.hasPrefix("/"), isSlashSwitch(arg) {
                if let error = applySlashOption(
                    arg: arg,
                    options: &options,
                    iterator: &iterator
                ) {
                    return .error(error)
                }
            } else {
                positional.append(arg)
            }
        }

        if options.showHelp { return .help }
        if options.showVersion { return .version }

        guard positional.count >= 2 else { return .error(.missingPaths) }
        guard positional.count == 2 else {
            return .error(.extraArguments(Array(positional.dropFirst(2))))
        }
        options.leftPath = positional[0]
        options.rightPath = positional[1]
        return .options(options)
    }

    // MARK: - ロングオプション(--name または --name=value)

    private static func applyLongOption(
        body: String,
        options: inout CLIOptions,
        iterator: inout IndexingIterator<[String]>
    ) -> CLIParseError? {
        let parts = body.split(separator: "=", maxSplits: 1, omittingEmptySubsequences: false)
        let name = String(parts[0])
        let inlineValue = parts.count > 1 ? String(parts[1]) : nil

        switch name {
        case "help":
            options.showHelp = true
        case "version":
            options.showVersion = true
        case "recursive":
            options.recursive = true
        case "no-recursive":
            options.recursive = false
        case "no-default-excludes":
            options.useDefaultExcludes = false
        case "exclude":
            guard let value = inlineValue ?? iterator.next() else { return .missingValue("--exclude") }
            options.userExcludePatterns.append(value)
        case "include":
            guard let value = inlineValue ?? iterator.next() else { return .missingValue("--include") }
            options.userIncludePatterns.append(value)
        case "preset":
            guard let value = inlineValue ?? iterator.next() else { return .missingValue("--preset") }
            guard FilterPreset.preset(id: value) != nil else { return .unknownPreset(value) }
            options.filterPresets.append(value)
        case "ignore-line-endings":
            options.contentComparison.ignoreLineEndings = true
        case "ignore-trailing-whitespace":
            options.contentComparison.ignoreTrailingWhitespace = true
        case "normalize-whitespace":
            options.contentComparison.normalizeWhitespace = true
        case "ignore-case":
            options.contentComparison.ignoreCase = true
        case "structured":
            guard let value = inlineValue ?? iterator.next() else { return .missingValue("--structured") }
            guard let mode = parseStructuredMode(value) else { return .invalidStructuredMode(value) }
            options.structuredMode = mode
        case "normalize-json":
            options.structuredMode = .json
        case "normalize-xml":
            options.structuredMode = .xml
        case "table-csv":
            options.structuredMode = .csv
        case "detect-renames":
            options.detectRenames = true
        case "format":
            guard let value = inlineValue ?? iterator.next() else { return .missingValue("--format") }
            guard let format = parseFormat(value) else { return .invalidFormat(value) }
            options.reportFormat = format
        case "output":
            guard let value = inlineValue ?? iterator.next() else { return .missingValue("--output") }
            options.outputPath = value
        case "quiet":
            options.quiet = true
        default:
            return .unknownOption("--\(name)")
        }
        return nil
    }

    // MARK: - ショートオプション(-x)。複合フラグ(-xq)は非対応。

    private static func applyShortOption(
        flag: String,
        options: inout CLIOptions,
        iterator: inout IndexingIterator<[String]>
    ) -> CLIParseError? {
        guard flag.count == 1 else { return .unknownOption("-\(flag)") }
        switch flag {
        case "h":
            options.showHelp = true
        case "V":
            options.showVersion = true
        case "r":
            options.recursive = true
        case "x":
            options.useDefaultExcludes = false
        case "e":
            guard let value = iterator.next() else { return .missingValue("-e") }
            options.userExcludePatterns.append(value)
        case "i":
            guard let value = iterator.next() else { return .missingValue("-i") }
            options.userIncludePatterns.append(value)
        case "f":
            guard let value = iterator.next() else { return .missingValue("-f") }
            guard let format = parseFormat(value) else { return .invalidFormat(value) }
            options.reportFormat = format
        case "o":
            guard let value = iterator.next() else { return .missingValue("-o") }
            options.outputPath = value
        case "q":
            options.quiet = true
        default:
            return .unknownOption("-\(flag)")
        }
        return nil
    }

    // MARK: - WinMergeスラッシュスイッチ(/r 等)

    /// 引数がWinMerge互換のスラッシュスイッチかを判定する。
    /// 絶対POSIXパス(/tmp/foo等)をスイッチと誤認しないよう、既知のスイッチ名と
    /// 完全一致した場合のみ true を返す。WinMergeはWindows由来でスイッチが
    /// 大文字小文字を区別しないため、小文字化して照合する。
    static func isSlashSwitch(_ arg: String) -> Bool {
        switch arg.lowercased() {
        case "/r", "/r-", "/u", "/ul", "/um", "/ur",
             "/wl", "/wm", "/wr",
             "/minimize", "/maximize", "/noninteractive",
             "/x", "/xq", "/enableexitcode",
             "/dl", "/dm", "/dr", "/or":
            return true
        default:
            return false
        }
    }

    /// WinMergeのスラッシュスイッチを適用する。GUI専用・MRU履歴系は受理のみ(無操作)、
    /// /or はレポート出力ファイルへ、/enableexitcode は終了コード写像へマップする。
    private static func applySlashOption(
        arg: String,
        options: inout CLIOptions,
        iterator: inout IndexingIterator<[String]>
    ) -> CLIParseError? {
        switch arg.lowercased() {
        case "/r":
            options.recursive = true
        case "/r-":
            options.recursive = false
        case "/u", "/ul", "/um", "/ur":
            // MRU履歴の抑制(GUI専用)。CLIには履歴が無いため無操作
            break
        case "/wl", "/wm", "/wr":
            // ペインの読み取り専用表示(GUI専用)。無操作
            break
        case "/minimize", "/maximize", "/noninteractive":
            // ウィンドウ状態・対話性(GUI専用)。CLIは常に非対話のため無操作
            break
        case "/x", "/xq":
            // 「比較後に閉じる」GUI用スイッチ。CLIでは進捗抑止(-q 相当)へマップ
            options.quiet = true
        case "/enableexitcode":
            options.enableExitCode = true
        case "/dl", "/dm", "/dr":
            // 左右ペインの表示ラベル(GUI専用)。値は消費して無視する
            guard iterator.next() != nil else { return .missingValue(arg) }
        case "/or":
            guard let value = iterator.next() else { return .missingValue("/or") }
            options.outputPath = value
            options.outputFromWinMerge = true
        default:
            return .unknownOption(arg)
        }
        return nil
    }

    // MARK: - 形式の解決(純粋関数)

    static func parseFormat(_ raw: String) -> ReportFormat? {
        switch raw.lowercased() {
        case "json": return .json
        case "csv": return .csv
        case "md", "markdown": return .markdown
        case "html": return .html
        default: return nil
        }
    }

    /// 構造化比較モードの文字列表現を解析する。未知の値は nil(使用時は .invalidStructuredMode)。
    static func parseStructuredMode(_ raw: String) -> StructuredComparisonMode? {
        switch raw.lowercased() {
        case "off", "disabled": return .disabled
        case "auto", "automatic": return .automatic
        case "json": return .json
        case "xml": return .xml
        case "csv": return .csv
        default: return nil
        }
    }

    /// 出力ファイルパスから形式を推測する(拡張子基準)。未知ならnil。
    static func formatForOutputPath(_ path: String) -> ReportFormat? {
        switch (path as NSString).pathExtension.lowercased() {
        case "json": return .json
        case "csv": return .csv
        case "md", "markdown": return .markdown
        case "html": return .html
        default: return nil
        }
    }

    /// レポートの出力先を決定する(純粋関数)。
    /// - 未指定 → .none(人間向けサマリのみをstdoutへ)
    /// - -oのみ → 拡張子から形式を推測してファイルへ
    /// - -fのみ → stdoutへ
    /// - -fと-o → ファイルへ
    static func reportTarget(options: CLIOptions) -> Result<CLIReportTarget, CLIParseError> {
        switch (options.reportFormat, options.outputPath) {
        case (nil, nil):
            return .success(.none)
        case (nil, let output?):
            if let format = formatForOutputPath(output) {
                return .success(.file(path: output, format: format))
            }
            // /or(WinMerge互換)は拡張子不明でも既定形式(HTML)へフォールバックする。
            // WinMergeの /or は既定でHTMLレポートを書くため、これに合わせる。
            if options.outputFromWinMerge {
                return .success(.file(path: output, format: .html))
            }
            return .failure(.outputExtensionUnknown(output))
        case (let format?, nil):
            return .success(.stdout(format))
        case (let format?, let output?):
            return .success(.file(path: output, format: format))
        }
    }
}

/// 引数・パス・オプション系エラーのユーザー向けメッセージ組み立て。
enum CLIError {
    static func message(for error: CLIParseError) -> String {
        switch error {
        case .unknownOption(let option):
            return "未知のオプションです: \(option)"
        case .missingValue(let option):
            return "オプションに値が必要です: \(option)"
        case .missingPaths:
            return "左右のフォルダパスを2つ指定してください。"
        case .extraArguments(let extra):
            return "想定外の引数です: \(extra.joined(separator: " "))"
        case .invalidFormat(let format):
            return "不明なレポート形式です: \(format) (json / csv / md / html)"
        case .invalidStructuredMode(let mode):
            return "不明な構造化比較モードです: \(mode) (off / auto / json / xml / csv)"
        case .outputExtensionUnknown(let path):
            return "出力ファイルの拡張子から形式を判別できません: \(path) (--formatを指定してください)"
        case .unknownPreset(let preset):
            return "不明なプリセットです: \(preset) (source-control / build-artifacts / web-nodejs / macos-metadata)"
        }
    }
}
