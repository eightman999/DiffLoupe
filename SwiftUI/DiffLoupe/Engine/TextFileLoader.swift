// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: 差分表示用のテキスト読み込み。UTF-8固定でバイナリ・巨大ファイルを判別する(SPEC.md §2.6)

import Foundation

enum TextLoadResult: Sendable {
    case text(String)
    case binary      // NULバイトを含む、またはUTF-8でデコード不能
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
        if data.isEmpty { return .text("") }
        if data.prefix(binarySniffLength).contains(0) { return .binary }
        guard let text = String(data: data, encoding: .utf8) else { return .binary }
        return .text(text)
    }
}
