// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: Git参照比較の読み取り専用モデル層。git subprocessでツリー参照を作業ツリーの
// スナップショットとしてexportし、従来のフォルダ比較(FolderComparator)へ渡す
// (read-only by contract: 書き込み・checkout・ファイル同期は一切しない)。
// 実装は Rust コア(diffloupe-ffi)への薄いファサード(git subprocess を呼ぶのは Rust 側)。

import Foundation

/// git リポジトリの作業ツリー。`toplevel`は `git rev-parse --show-toplevel` の結果。
/// すべて読み取り専用(作業ツリーへの書き込み・checkout・コミット等は一切行わない)。
public struct GitRepository: Sendable, Equatable {

    /// git サブプロセスの失敗をラップするエラー
    public struct GitError: LocalizedError, Sendable {
        public let message: String
        public var errorDescription: String? { message }
    }

    /// `git rev-parse --show-toplevel` の出力パス
    public let toplevel: URL

    /// URL が git 作業ツリー内か判定し、リポジトリルートを返す。非リポジトリなら nil。
    /// `git rev-parse --show-toplevel` の成功=作業ツリー内、失敗=非リポジトリ。
    public static func detect(url: URL) -> GitRepository? {
        guard let toplevel = diffloupeGitDetect(path: url.path) else { return nil }
        return GitRepository(toplevel: URL(fileURLWithPath: toplevel))
    }

    /// この作業ツリーが属する git 参照(ブランチ/タグ/commit)の一覧を返す。
    /// `git for-each-ref --format=%(refname:short)` の出力で、HEAD は除く。
    /// 取得不能な場合(非リポジトリ・git不在・gitエラー)は空配列を返し、失敗で壊さない設計。
    public func listRefs() -> [String] {
        diffloupeGitListRefs(toplevel: toplevel.path)
    }

    /// git参照をスナップショットとして物質化する(`git archive <ref> | tar -x`)。
    /// 物質化先は一時ディレクトリ(このアプリのみの書き込み領域)で、.git ディレクトリは含まない。
    /// 成功した場合はスナップショットのルートURLを返す。
    /// - Parameter ref: ブランチ/タグ/commit(git rev-parse で解釈可能な文字列)
    /// - Throws: GitError(git不在・参照不明・exportエラー)
    public func materializeSnapshot(ref: String) throws -> URL {
        do {
            let path = try diffloupeGitMaterializeSnapshot(
                toplevel: toplevel.path, gitRef: ref)
            return URL(fileURLWithPath: path)
        } catch let error as FfiGitError {
            switch error {
            case .Git(message: let message): throw GitError(message: message)
            }
        }
    }

    /// 物質化したスナップショット(一時ディレクトリ)を除去する。
    /// 失敗しても投げない(一時ファイルの掃除は最善努力)。
    public static func cleanup(_ url: URL) {
        diffloupeGitCleanup(path: url.path)
    }
}
