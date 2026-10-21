// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: 2フォルダを再帰走査し「サイズ→SHA-256」の段階判定で分類するコアエンジン(SPEC.md §2)。
// 実装は Rust コア(diffloupe-ffi)の `diffloupeCompareFolders` への薄いファサード。
// ブロッキング呼び出しはバックグラウンドキューで実行し、キャンセルは
// FfiCancellationToken 経由で Rust 側へ伝搬する。

import Foundation
import Darwin

public struct FolderComparator: Sendable {

    public init() {}

    public struct EntryLimitExceeded: LocalizedError, Sendable {
        public let limit: Int
        public let observed: Int

        public var errorDescription: String? {
            "Freeは\(limit)ファイルまで比較できます。Proで制限を解除できます。"
        }
    }

    /// Rust 側の IO エラーをラップする(呼び出し側には error.localizedDescription で届く)
    struct CompareIOError: LocalizedError, Sendable {
        let message: String
        var errorDescription: String? { message }
    }

    public struct Options: Sendable {
        /// サブディレクトリへ再帰的に降下して比較するか。既定 true(SwiftUIアプリの既存挙動)。
        /// false のときは各ルートの直下エントリのみを対象とし、トップレベルのディレクトリは
        /// エントリとして記録するが、その中へは降下しない(WinMergeの非再帰比較相当)。
        public var recursive = true

        /// 既定除外パターン(.git / .DS_Store / node_modules)を適用するか(SPEC.md §2.2)
        public var useDefaultExcludes = true

        /// 比較対象エントリ数の上限。nilなら無制限(Pro)。
        public var maxEntries: Int?

        /// ユーザー定義の除外globパターン。区切りなしはファイル名、区切りありは相対パスに照合する。
        /// ディレクトリに一致すればサブツリーごと枝刈りされる(WinMerge互換の除外拡張)。
        public var userExcludePatterns: [String] = []

        /// ユーザー定義のインクルードglobパターン。空なら全ファイル対象(従来挙動と同一)。
        /// 非空のときは、ファイル名/相対パスの少なくとも1つに一致するファイルのみを対象に含める。
        /// ディレクトリはインクルードで刈らず(一致しなくても降下する)、除外はインクルードより優先する。
        public var userIncludePatterns: [String] = []

        /// 内容比較モード。既定は厳密比較(バイト/SHA-256)で、既存挙動と同一(SPEC.md §2.3)。
        public var contentComparison = ContentComparison()

        /// 左右片側のみのファイルについて、内容ハッシュ一致のリネーム/移動候補を検出するか。
        /// 既定オフ(片側のみのハッシュ計算コストを避ける)。有効化すると追加の並列ハッシュ走査が入る。
        public var detectRenames = false

        /// 正規化付き内容比較でData(contentsOf:)一括読込を許容する最大バイト数。
        /// 超過するファイルはメモリ保護のため正規化比較をせず、バイト厳密(ハッシュ/サイズ)で判定する。
        /// 既定 64MiB(TextFileLoaderの表示上限32MiBの2倍)。
        public var maxNormalizationCompareBytes: Int64 = 64 * 1024 * 1024

        public static let freeMaxEntries = 2000
        public static let defaultExcludedNames: Set<String> = [".git", ".DS_Store", "node_modules"]

        var excludedNames: Set<String> {
            useDefaultExcludes ? Self.defaultExcludedNames : []
        }

        public init(
            recursive: Bool = true,
            useDefaultExcludes: Bool = true,
            maxEntries: Int? = nil,
            userExcludePatterns: [String] = [],
            userIncludePatterns: [String] = [],
            contentComparison: ContentComparison = ContentComparison(),
            detectRenames: Bool = false,
            maxNormalizationCompareBytes: Int64 = 64 * 1024 * 1024
        ) {
            self.recursive = recursive
            self.useDefaultExcludes = useDefaultExcludes
            self.maxEntries = maxEntries
            self.userExcludePatterns = userExcludePatterns
            self.userIncludePatterns = userIncludePatterns
            self.contentComparison = contentComparison
            self.detectRenames = detectRenames
            self.maxNormalizationCompareBytes = maxNormalizationCompareBytes
        }
    }

    public typealias ProgressHandler = @Sendable (ComparisonProgress) -> Void

    /// 64KiBチャンク(既存C++/Python版と同一。SPEC.md §2.3)
    static let hashChunkSize = 65536

    public func compare(
        left: URL,
        right: URL,
        options: Options = Options(),
        progress: ProgressHandler? = nil
    ) async throws -> ComparisonResult {

        let token = FfiCancellationToken()
        // 呼び出し時点で既にキャンセルされていた場合に備える(後続の onCancel と同じ経路)
        if Task.isCancelled { token.cancel() }

        return try await withTaskCancellationHandler {
            try await withCheckedThrowingContinuation { (continuation: CheckedContinuation<ComparisonResult, Error>) in
                DispatchQueue.global(qos: .userInitiated).async {
                    do {
                        let callback: FfiProgressCallback? = progress.map { p in
                            ProgressCallbackBox { progress in p(ComparisonProgress(progress)) }
                        }
                        let ffiResult = try diffloupeCompareFolders(
                            leftPath: left.path,
                            rightPath: right.path,
                            options: options.ffi,
                            progress: callback,
                            cancel: token
                        )
                        continuation.resume(returning: ComparisonResult(
                            ffiResult, leftRoot: left, rightRoot: right))
                    } catch let error as FfiCompareError {
                        switch error {
                        case .Cancelled:
                            continuation.resume(throwing: CancellationError())
                        case .EntryLimitExceeded(limit: let limit, observed: let observed):
                            continuation.resume(throwing: EntryLimitExceeded(
                                limit: Int(limit), observed: Int(observed)))
                        case .Io(message: let message):
                            continuation.resume(throwing: CompareIOError(message: message))
                        }
                    } catch {
                        continuation.resume(throwing: error)
                    }
                }
            }
        } onCancel: {
            token.cancel()
        }
    }

    /// SHA-256をストリーミング計算(64KiBチャンク)。内部利用。
    /// Rust 側で計算した32バイトのダイジェストをそのまま返す。
    static func sha256(of url: URL) throws -> Data {
        try diffloupeSha256File(path: url.path)
    }

    /// フラットなエントリ列からツリーを構築する。
    /// 同一階層はディレクトリ優先・名前昇順(SPEC.md §2.4)
    static func buildTree(entries: [ComparedEntry]) -> DiffNode {
        DiffNode(diffloupeBuildDiffTree(entries: entries.map { $0.ffi }))
    }
}

// MARK: - FFI 変換

/// Swift の ProgressHandler を FFI コールバックへ包む箱
private final class ProgressCallbackBox: FfiProgressCallback, @unchecked Sendable {
    private let handler: @Sendable (FfiComparisonProgress) -> Void
    init(_ handler: @escaping @Sendable (FfiComparisonProgress) -> Void) {
        self.handler = handler
    }
    func onProgress(progress: FfiComparisonProgress) {
        handler(progress)
    }
}

extension FolderComparator.Options {
    var ffi: FfiCompareOptions {
        FfiCompareOptions(
            recursive: recursive,
            useDefaultExcludes: useDefaultExcludes,
            maxEntries: maxEntries.map { UInt64($0) },
            userExcludePatterns: userExcludePatterns,
            userIncludePatterns: userIncludePatterns,
            contentComparison: contentComparison.ffi,
            detectRenames: detectRenames,
            maxNormalizationCompareBytes: maxNormalizationCompareBytes
        )
    }
}
