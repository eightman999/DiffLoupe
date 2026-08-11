// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: 差分表示用のテキスト読み込み。主要エンコードを自動判定してデコードし、
// バイナリ・巨大ファイルを判別する(SPEC.md §2.6)
// 判定順序: BOM → NULバイトsniff → ISO-2022-JP(ESC検出) → UTF-8厳密デコード
//          → Shift_JIS/EUC-JPヒューリスティック → 全滅なら .binary
// 有効なUTF-8ファイルは常にBOM無しなら"UTF-8"判定・従来と同一文字列になる(後方互換最優先)。

import Foundation

enum TextLoadResult: Sendable {
    case text(String, encodingName: String)
    case binary      // NULバイトを含む、または既知エンコードでデコード不能
    case tooLarge    // 表示上限(32MiB)超過
}

enum TextFileLoader {

    /// 差分表示の対象とする最大ファイルサイズ
    static let maxTextBytes = 32 * 1024 * 1024

    /// バイナリ判定に使う先頭バイト数
    static let binarySniffLength = 8192

    static func load(url: URL) throws -> TextLoadResult {
        if let size = try url.resourceValues(forKeys: [.fileSizeKey]).fileSize,
           size > maxTextBytes {
            return .tooLarge
        }

        let data = try Data(contentsOf: url)
        if data.count > maxTextBytes { return .tooLarge }
        if data.isEmpty { return .text("", encodingName: "UTF-8") }

        if let (text, name) = decode(data) {
            return .text(text, encodingName: name)
        }
        return .binary
    }

    // MARK: - デコード本体

    /// バイト列から (デコード後文字列, 判定エンコード表示名) を返す。全判定失敗時は nil。
    static func decode(_ data: Data) -> (String, String)? {
        // 1. BOMによる判定(最優先。UTF-16/32 BOM付きファイルはNULを含み得るため
        //    NULバイトsniffより先に行う)
        if let bomResult = decodeByBOM(data) {
            return bomResult
        }

        // 2. NULバイトsniff(BOM無しファイルのバイナリ早期判定。UTF-8として偶然
        //    デコード可能でもNULを含む場合はバイナリ扱いとする既存挙動を維持)
        if data.prefix(binarySniffLength).contains(0) {
            return nil
        }

        // 3. ISO-2022-JP (ESCシーケンス検出 + 7bit確認)。
        //    ISO-2022-JPのESC/シフト制御バイトはASCII範囲内のためUTF-8としても
        //    デコード可能になってしまうことがあるので、UTF-8厳密デコードより先に
        //    ESCシーケンスの有無を見て判定する(誤判定防止)。
        if looksLikeISO2022JP(data), let text = String(data: data, encoding: .iso2022JP) {
            return (text, "ISO-2022-JP")
        }

        // 4. BOM無しUTF-8の厳密デコード(既存の後方互換性を最優先で確保)
        if let text = String(data: data, encoding: .utf8) {
            return (text, "UTF-8")
        }

        // 5. Shift_JIS / EUC-JP ヒューリスティック
        if let (text, encoding) = decodeJapaneseLegacyEncoding(data) {
            let name = encoding == .shiftJIS ? "Shift_JIS" : "EUC-JP"
            return (text, name)
        }

        return nil
    }

    /// BOMを検出してデコードする。BOMが無ければ nil。
    private static func decodeByBOM(_ data: Data) -> (String, String)? {
        let bytes = [UInt8](data.prefix(4))

        // UTF-32は4バイトBOMのため、UTF-16のBOM(2バイトプレフィックスが一致)より先に判定する
        if bytes.count >= 4, bytes[0] == 0x00, bytes[1] == 0x00, bytes[2] == 0xFE, bytes[3] == 0xFF {
            let body = data.dropFirst(4)
            if let text = String(data: body, encoding: .utf32BigEndian) {
                return (text, "UTF-32 BE")
            }
        }
        if bytes.count >= 4, bytes[0] == 0xFF, bytes[1] == 0xFE, bytes[2] == 0x00, bytes[3] == 0x00 {
            let body = data.dropFirst(4)
            if let text = String(data: body, encoding: .utf32LittleEndian) {
                return (text, "UTF-32 LE")
            }
        }
        if bytes.count >= 3, bytes[0] == 0xEF, bytes[1] == 0xBB, bytes[2] == 0xBF {
            let body = data.dropFirst(3)
            if let text = String(data: body, encoding: .utf8) {
                return (text, "UTF-8 (BOM)")
            }
        }
        if bytes.count >= 2, bytes[0] == 0xFE, bytes[1] == 0xFF {
            let body = data.dropFirst(2)
            if let text = String(data: body, encoding: .utf16BigEndian) {
                return (text, "UTF-16 BE")
            }
        }
        if bytes.count >= 2, bytes[0] == 0xFF, bytes[1] == 0xFE {
            let body = data.dropFirst(2)
            if let text = String(data: body, encoding: .utf16LittleEndian) {
                return (text, "UTF-16 LE")
            }
        }
        return nil
    }

    /// ISO-2022-JPらしいESCシーケンス(ESC $ B / ESC $ @ / ESC ( B / ESC ( J 等)を含み、
    /// かつ全バイトが7bit範囲内かを確認する簡易ヒューリスティック
    private static func looksLikeISO2022JP(_ data: Data) -> Bool {
        var foundEscSequence = false
        var index = data.startIndex
        while index < data.endIndex {
            let byte = data[index]
            if byte > 0x7F {
                return false // 8bit以上を含む場合はISO-2022-JPではない
            }
            if byte == 0x1B { // ESC
                foundEscSequence = true
            }
            index = data.index(after: index)
        }
        return foundEscSequence
    }

    /// Shift_JIS / EUC-JP をヒューリスティックに判定してデコードする。
    /// NSString の自動エンコード検出を用い、判定結果がどちらかであれば採用する。
    private static func decodeJapaneseLegacyEncoding(_ data: Data) -> (String, String.Encoding)? {
        let candidateEncodings: [String.Encoding] = [.shiftJIS, .japaneseEUC]

        var convertedString: NSString?
        var usedLossyConversion: ObjCBool = false
        let detected = NSString.stringEncoding(
            for: data,
            encodingOptions: [
                .suggestedEncodingsKey: candidateEncodings.map { NSNumber(value: $0.rawValue) },
                .useOnlySuggestedEncodingsKey: true
            ],
            convertedString: &convertedString,
            usedLossyConversion: &usedLossyConversion
        )

        if detected != 0, !usedLossyConversion.boolValue, let converted = convertedString as String? {
            let encoding = String.Encoding(rawValue: detected)
            if candidateEncodings.contains(encoding) {
                return (converted, encoding)
            }
        }

        // フォールバック: 個別に厳密デコードを試す(Shift_JIS優先)
        for encoding in candidateEncodings {
            if let text = String(data: data, encoding: encoding) {
                return (text, encoding)
            }
        }
        return nil
    }
}
