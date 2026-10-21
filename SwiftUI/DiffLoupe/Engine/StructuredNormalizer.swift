// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: JSON / XML / CSV・TSV の構造化正規化(比較用の決定論的カノニカル形式への変換)。
// フォルダ分類(FolderComparator)と詳細差分(LineDiff)の両方で同一の正規化結果を使う。
//
// 正規化に成功しなかった場合(didNormalize == false)は、既存のテキスト/バイト正規化へ
// フォールバックする。構造化パースが成功していないのに「構造化で一致」と判定しないための
// 安全側の契約(normalize が返す notice を UI が警告表示に使う)。
//
// 制約:
// - パース上限 maxParseBytes(8 MiB)を超えるデータは構造化パースしない(フォールバック)
// - JSON: オブジェクトのキーをソート、配列は順序を保持、決定論的に整形
// - XML : 属性をソート、2スペース字下げ、インデント専用の空白のみ除去
// - CSV : RFC4180 の引用符規則を解釈し、区切り文字(カンマ/タブ/セミコロン)を自動検出
// 実装は Rust コア(diffloupe-ffi)への薄いファサード。

import Foundation

/// 構造化フォーマット。URL拡張子(.json/.xml/.csv/.tsv)または明示モードから決まる。
public enum StructuredFormat: String, Sendable, Hashable, CaseIterable {
    case json
    case xml
    case csv

    public var displayName: String {
        switch self {
        case .json: return "JSON"
        case .xml: return "XML"
        case .csv: return "CSV"
        }
    }

    /// 拡張子からフォーマットを判定する。.tsv は CSV(タブ区切り)として扱う。未知なら nil。
    public init?(urlExtension: String) {
        switch urlExtension.lowercased() {
        case "json": self = .json
        case "xml": self = .xml
        case "csv": self = .csv
        case "tsv": self = .csv
        default: return nil
        }
    }
}

/// 構造化比較モード。既定は disabled(既存挙動と同一)。
public enum StructuredComparisonMode: String, Sendable, Hashable, CaseIterable {
    /// 構造化比較を行わない(バイト/テキスト正規化のみ)
    case disabled
    /// 拡張子(.json/.xml/.csv/.tsv)から自動判定する。未知なら従来どおりテキスト比較へ
    case automatic
    case json
    case xml
    case csv

    public var displayName: String {
        switch self {
        case .disabled: return "オフ"
        case .automatic: return "自動(拡張子で判定)"
        case .json: return "JSON"
        case .xml: return "XML"
        case .csv: return "CSV"
        }
    }
}

/// 構造化正規化の結果。didNormalize == true のとき normalizedText が有効。
/// didNormalize == false のときは notice にフォールバック理由を載せ、呼び出し側は
/// 既存のテキスト/バイト正規化へフォールバックする(構造化一致を主張しない)。
public struct StructuredNormalizationResult: Sendable, Equatable {
    public let didNormalize: Bool
    /// 正規化に使った(または試みた)フォーマット。自動判定で未知拡張子の場合は nil
    public let format: StructuredFormat?
    /// 正規化後の決定論的テキスト(didNormalize == true のときのみ非 nil)
    public let normalizedText: String?
    /// フォールバック理由などの通知(didNormalize == false のとき設定されることがある)
    public let notice: String?

    public init(
        didNormalize: Bool,
        format: StructuredFormat?,
        normalizedText: String?,
        notice: String?
    ) {
        self.didNormalize = didNormalize
        self.format = format
        self.normalizedText = normalizedText
        self.notice = notice
    }
}

public enum StructuredNormalizeError: Error, Equatable {
    case jsonParse(String)
    case xmlParse(String)
    case csvParse(String)
}

/// 構造化正規化エンジン。Foundation のみに依存する(パースに任意のスレッドで使える)。
public enum StructuredNormalizer {

    /// 構造化パースの上限バイト数。超過はパースせずフォールバックする。
    public static let maxParseBytes = 8 * 1024 * 1024

    // MARK: - 公開API

    /// データを指定モードで正規化する。
    /// - Parameters:
    ///   - data: 対象バイト列(UTF-8 前提)
    ///   - url: 拡張子判定用(automatic モードで使用)
    ///   - mode: 構造化比較モード
    /// - Returns: 正規化結果。失敗時は didNormalize == false でフォールバックを指示する
    public static func normalize(
        _ data: Data,
        url: URL?,
        mode: StructuredComparisonMode
    ) -> StructuredNormalizationResult {
        let r = diffloupeStructuredNormalize(
            data: data, url: url?.path, mode: mode.ffi)
        return StructuredNormalizationResult(
            didNormalize: r.didNormalize,
            format: r.format.map { StructuredFormat($0) },
            normalizedText: r.normalizedText,
            notice: r.notice
        )
    }

    /// CSVの区切り文字を先頭行の頻度から検出する(カンマ > タブ > セミコロンの優先順)。
    static func detectCSVDelimiter(_ text: String) -> Character {
        Character(diffloupeDetectCsvDelimiter(text: text))
    }

    /// RFC4180の引用符規則を解釈してCSVをパースする。
    /// - Throws: StructuredNormalizeError.csvParse(引用符が閉じていない等)
    static func parseCSV(_ text: String, delimiter: Character) throws -> [[String]] {
        do {
            return try diffloupeParseCsv(text: text, delimiter: String(delimiter))
        } catch let error as FfiStructuredError {
            switch error {
            case .Json(message: let m): throw StructuredNormalizeError.jsonParse(m)
            case .Xml(message: let m): throw StructuredNormalizeError.xmlParse(m)
            case .Csv(message: let m): throw StructuredNormalizeError.csvParse(m)
            }
        }
    }
}

// MARK: - FFI 変換

extension StructuredComparisonMode {
    var ffi: FfiStructuredComparisonMode {
        switch self {
        case .disabled: return .disabled
        case .automatic: return .automatic
        case .json: return .json
        case .xml: return .xml
        case .csv: return .csv
        }
    }
}

extension StructuredFormat {
    init(_ ffi: FfiStructuredFormat) {
        switch ffi {
        case .json: self = .json
        case .xml: self = .xml
        case .csv: self = .csv
        }
    }
}
