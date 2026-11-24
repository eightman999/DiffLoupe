// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: zip アーカイブ比較の読み取り専用モデル層。比較の直前に zip を一時ディレクトリへ
// 展開(実体化)して従来のフォルダ比較(FolderComparator)へ渡し、結果を捨てるときに掃除する
// (git ref のスナップショットと同じ流れ。元の zip には一切書き込まない)。
// 実装は Rust コア(diffloupe-ffi)への薄いファサード(展開・上限判定は Rust 側)。

import Foundation

/// zip アーカイブをフォルダ比較用のスナップショットとして扱う薄い型。
public enum ArchiveSnapshot {

    /// 展開の失敗(壊れた zip・パスワード付き・上限超過など)。メッセージは日本語でそのまま表示できる。
    public struct ArchiveError: LocalizedError, Sendable {
        public let message: String
        public var errorDescription: String? { message }
    }

    /// 拡張子等から zip として扱えるファイルかどうか(ディレクトリは false)。
    public static func isArchive(url: URL) -> Bool {
        diffloupeArchiveIsSupported(path: url.path)
    }

    /// zip を一時ディレクトリへ展開し、そのルートURLを返す。
    /// 展開先はこのアプリのみの書き込み領域で、失敗時は Rust 側で後始末済み。
    /// - Throws: ArchiveError
    public static func materialize(url: URL) throws -> URL {
        do {
            let path = try diffloupeArchiveMaterialize(path: url.path)
            return URL(fileURLWithPath: path, isDirectory: true)
        } catch let error as FfiArchiveError {
            switch error {
            case .Archive(message: let message): throw ArchiveError(message: message)
            }
        }
    }

    /// 展開したスナップショット(一時ディレクトリ)を除去する。失敗しても投げない(最善努力)。
    public static func cleanup(_ url: URL) {
        diffloupeArchiveCleanup(path: url.path)
    }
}
