// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: diffloupe CLIの標準出力/標準エラー書き込みと進捗表示。
// stdoutはレポート専用にするため、人間向けサマリ・進捗はstderrへ出す。

import Foundation
import DiffLoupeCore

enum CLIO {
    static func standardOutput(_ text: String) {
        FileHandle.standardOutput.write(Data((text + "\n").utf8))
    }

    static func standardError(_ text: String) {
        FileHandle.standardError.write(Data((text + "\n").utf8))
    }
}

enum CLIProgress {
    static func print(_ progress: ComparisonProgress) {
        let line: String
        switch progress {
        case .scanning(let found):
            line = "走査中: \(found) エントリ"
        case .comparing(let done, let total):
            line = "内容比較: \(done)/\(total)"
        case .renames(let done, let total):
            line = "リネーム候補検出: \(done)/\(total)"
        }
        CLIO.standardError(line)
    }
}
