// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: 比較対象フォルダの変更を監視し、自動再比較のトリガーにする。
// 実装は Rust コア(diffloupe-ffi)のポーリング監視への薄いファサード。
// Rust 側のワーカースレッドが定期的にスナップショット差分を検出し、Swift の
// onChange コールバックを呼ぶ。baseline・通知回数の規則は従来の Swift 実装と同一。

import Foundation

/// 比較対象のフォルダツリー(名前+サイズ+mtime)のスナップショット。
/// watcherの差分判定に使う内部値。相対パス順でソートされるため、
/// 走査順が変わっても同一ツリーなら常に同値になる。
public struct DirectorySnapshot: Equatable, Sendable {

    /// 1エントリ(ファイルまたはディレクトリ)のスナップショット
    public struct Entry: Hashable, Sendable {
        public let relativePath: String
        public let isDirectory: Bool
        public let fileSize: Int64
        /// 最終変更時刻。取得不能時は nil とし、変更検出を「不明=変更」に倒す保守側
        public let modificationDate: Date?
        public let isSymbolicLink: Bool

        public init(
            relativePath: String,
            isDirectory: Bool,
            fileSize: Int64,
            modificationDate: Date?,
            isSymbolicLink: Bool
        ) {
            self.relativePath = relativePath
            self.isDirectory = isDirectory
            self.fileSize = fileSize
            self.modificationDate = modificationDate
            self.isSymbolicLink = isSymbolicLink
        }
    }

    public let entries: [Entry]

    public init(entries: [Entry]) {
        self.entries = entries
    }

    /// ルートを再帰走査してスナップショットを作る。相対パス順でソートするため、
    /// 走査順が変わっても同一ツリーなら常に同値になる。ディレクトリのシンボリックリンクは
    /// 再帰追跡せず、そのエントリのみを記録する。
    /// - Parameter excludedNames: 名前が一致するエントリを除外する。ディレクトリはサブツリーごと
    ///   スキップ(skipDescendants)し、ファイルは記録しない。FolderComparatorの既定除外(.git /
    ///   .DS_Store / node_modules)を watcher のスナップショットにも適用するために使う。空なら従来通り。
    public static func scan(
        root: URL,
        fileManager: FileManager = .default,
        excludedNames: Set<String> = []
    ) throws -> DirectorySnapshot {
        let rootPath = root.standardizedFileURL.path
        var isDirectory: ObjCBool = false
        guard fileManager.fileExists(atPath: rootPath, isDirectory: &isDirectory), isDirectory.boolValue else {
            throw DirectoryWatcherError.rootIsNotDirectory(rootPath)
        }
        let ffi: FfiDirectorySnapshot
        do {
            ffi = try diffloupeSnapshotScan(
                path: rootPath, excludedNames: Array(excludedNames).sorted())
        } catch let error as FfiWatchError {
            switch error {
            case .RootIsNotDirectory(path: let p):
                throw DirectoryWatcherError.rootIsNotDirectory(p)
            case .UnreadableRoot(path: let p):
                throw DirectoryWatcherError.unreadableRoot(p)
            }
        }
        return DirectorySnapshot(entries: ffi.entries.map { e in
            Entry(
                relativePath: e.relativePath,
                isDirectory: e.isDirectory,
                fileSize: e.fileSize,
                modificationDate: e.modificationDate.map { Date(timeIntervalSince1970: $0) },
                isSymbolicLink: e.isSymbolicLink
            )
        })
    }
}

public enum DirectoryWatcherError: Error, Sendable {
    case rootIsNotDirectory(String)
    case unreadableRoot(String)
}

/// 複数ルートを一定間隔で再スキャンし、スナップショットが変わったときにコールバックを
/// 一度だけ通知する監視器。ポーリング実装は Rust コア(FfiDirectoryWatcher)に委譲する。
public final class DirectoryWatcher: @unchecked Sendable {

    public typealias ChangeHandler = @Sendable () -> Void

    private let roots: [URL]
    private let interval: TimeInterval
    private let excludedNames: Set<String>
    private let onChange: ChangeHandler
    private var watcher: FfiDirectoryWatcher?

    /// - Parameter roots: 監視対象のルート(左右フォルダなど)。空なら何も監視しない。
    /// - Parameter interval: 再スキャン間隔(秒)。既定1秒。テスト等では短縮できる(下限0.01秒)。
    /// - Parameter excludedNames: スナップショットから除外する名前(既定除外など)。空なら従来通り。
    /// - Parameter onChange: 変更検知時に一度だけ呼ばれるコールバック。watcher内部のキューで
    ///   呼ばれるため、UIへ反映する場合は利用側でMainActor等へ戻してから行うこと。
    public init(
        roots: [URL],
        interval: TimeInterval = 1.0,
        excludedNames: Set<String> = [],
        onChange: @escaping ChangeHandler
    ) {
        self.roots = roots
        self.interval = max(interval, 0.01)
        self.excludedNames = excludedNames
        self.onChange = onChange
    }

    deinit {
        stop()
    }

    /// 監視を開始する。初回の基準スナップショットはRust側で非同期に記録される
    /// (呼び出しスレッド=MainActor等をブロックしない)。baseline完了前のpollは何もしない。
    /// baseline完了後、intervalごとに再スキャンして変化があればonChangeを一度だけ呼ぶ。
    /// stop後に再startすることが可能。
    public func start() {
        guard watcher == nil else { return }
        let w = FfiDirectoryWatcher(
            roots: roots.map { $0.standardizedFileURL.path },
            intervalSecs: interval,
            excludedNames: Array(excludedNames).sorted(),
            onChange: ChangeCallbackBox(onChange)
        )
        watcher = w
        w.start()
    }

    /// 監視を停止する(Rust ワーカースレッドを終了させる)。複数回呼んでも安全。
    public func stop() {
        watcher?.stop()
        watcher = nil
    }
}

// MARK: - FFI 変換

/// Swift の onChange コールバックを FFI コールバックへ包む箱
private final class ChangeCallbackBox: FfiWatchCallback, @unchecked Sendable {
    private let handler: @Sendable () -> Void
    init(_ handler: @escaping @Sendable () -> Void) {
        self.handler = handler
    }
    func onChange() {
        handler()
    }
}
