// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: 差分表示用のテキスト読み込み。主要エンコードを自動判定してデコードし、
// バイナリ・巨大ファイルを判別する(SPEC.md §2.6)
// 判定順序: BOM → NULバイトsniff → ISO-2022-JP(ESC検出) → UTF-8厳密デコード
//          → Shift_JIS/EUC-JPヒューリスティック → 全滅なら .binary
// 有効なUTF-8ファイルは常にBOM無しなら"UTF-8"判定・従来と同一文字列になる(後方互換最優先)。
// 実装は Rust コア(diffloupe-ffi)への薄いファサード。

import Foundation

public enum TextLoadResult: Sendable {
    case text(String, encodingName: String)
    case binary      // NULバイトを含む、または既知エンコードでデコード不能
    case tooLarge    // 表示上限(32MiB)超過
}

/// Rust 側の読み込み失敗をラップするエラー
struct TextLoadIOError: LocalizedError, Sendable {
    let message: String
    var errorDescription: String? { message }
}

public enum TextFileLoader {

    /// 差分表示の対象とする最大ファイルサイズ
    public static let maxTextBytes = 32 * 1024 * 1024

    /// バイナリ判定に使う先頭バイト数
    public static let binarySniffLength = 8192

    public static func load(url: URL) throws -> TextLoadResult {
        let result: FfiTextLoadResult
        do {
            result = try diffloupeLoadText(path: url.path)
        } catch let error as FfiIoError {
            switch error {
            case .Io(message: let message): throw TextLoadIOError(message: message)
            }
        }
        switch result {
        case .text(let text, let encodingName):
            return .text(text, encodingName: encodingName)
        case .binary:
            return .binary
        case .tooLarge:
            return .tooLarge
        }
    }

    /// バイト列から (デコード後文字列, 判定エンコード表示名) を返す。全判定失敗時は nil。
    public static func decode(_ data: Data) -> (String, String)? {
        guard let decoded = diffloupeDecodeText(data: data) else { return nil }
        return (decoded.text, decoded.encodingName)
    }
}
