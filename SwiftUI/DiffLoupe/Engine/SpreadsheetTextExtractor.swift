// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: 表計算ファイル(xlsx / xlsm / xls / ods)をテキストへ抽出する読み取り専用の層。
// シートごとに「## シート名」の見出しとタブ区切りの行を出力し、行差分で比較できる形にする。
// 実装は Rust コア(diffloupe-ffi)への薄いファサード。元ファイルには書き込まない。

import Foundation

public enum SpreadsheetTextExtractor {

    /// 抽出の失敗(壊れたファイル・パスワード付き・非対応形式など)。メッセージは日本語表示用。
    public struct ExtractError: LocalizedError, Sendable {
        public let message: String
        public var errorDescription: String? { message }
    }

    /// 拡張子から表計算ファイルと判定できるか(xlsx / xlsm / xls / ods)。
    public static func isSpreadsheet(url: URL) -> Bool {
        diffloupeDocumentIsSpreadsheet(path: url.path)
    }

    /// 表計算ファイルのテキストを返す。
    /// - Throws: ExtractError
    public static func extractText(from url: URL) throws -> String {
        do {
            return try diffloupeDocumentExtractSpreadsheet(path: url.path)
        } catch let error as FfiDocumentError {
            switch error {
            case .Document(message: let message): throw ExtractError(message: message)
            }
        }
    }
}
