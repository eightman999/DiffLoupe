// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: 左右ルートの再帰スナップショットを一定間隔で取り直し、ファイルの追加・削除・
// サイズ変更・mtime変更を検知してコールバックを一度だけ通知する。外部依存なしのFoundation実装
// (WinMergeの「外部変更後の再比較」相当をopt-inで提供)。UI/AppKit非依存のためCoreターゲットからも
// 参照できる。ディレクトリのシンボリックリンクは再帰追跡しない(種別のみ記録)。
// コールバックはwatcher内部のキューで呼ばれるため、UIへ反映する場合は利用側でMainActorへ
// 戻してから行うこと。stop()またはdeinitでタイマーを止め、停止後の通知は行わない。

import Foundation
import Dispatch

/// ディレクトリツリーの比較可能なスナップショット。値型で、同値比較により変更検知に使う。
public struct DirectorySnapshot: Equatable, Sendable {

    /// ツリー内の1エントリ。相対パス・種別・サイズ・mtime・symlink状態から同一性を判定する。
    /// ディレクトリは追加/削除のみで検知するため、fileSizeは0・modificationDateはnilに正規化する。
    public struct Entry: Hashable, Sendable {
        public let relativePath: String
        public let isDirectory: Bool
        public let fileSize: Int64
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
        let keys: Set<URLResourceKey> = [
            .isDirectoryKey,
            .fileSizeKey,
            .contentModificationDateKey,
            .isSymbolicLinkKey,
        ]
        guard let enumerator = fileManager.enumerator(
            at: root,
            includingPropertiesForKeys: Array(keys),
            options: [],
            errorHandler: nil
        ) else {
            throw DirectoryWatcherError.unreadableRoot(rootPath)
        }
        var entries: [Entry] = []
        for case let url as URL in enumerator {
            let name = url.lastPathComponent
            let values = try? url.resourceValues(forKeys: keys)
            let isSymlink = values?.isSymbolicLink ?? false
            let isDir = values?.isDirectory ?? false
            if excludedNames.contains(name) {
                // 除外ディレクトリ(.git等)はサブツリーごと枝刈りし、除外ファイル(.DS_Store等)は記録しない
                if isDir || isSymlink {
                    enumerator.skipDescendants()
                }
                continue
            }
            entries.append(Entry(
                relativePath: Self.relativePath(of: url, from: rootPath),
                isDirectory: isDir,
                fileSize: isDir ? 0 : Int64(values?.fileSize ?? 0),
                modificationDate: isDir ? nil : values?.contentModificationDate,
                isSymbolicLink: isSymlink
            ))
            if isSymlink {
                enumerator.skipDescendants()
            }
        }
        return DirectorySnapshot(entries: entries.sorted { $0.relativePath < $1.relativePath })
    }

    /// urlをルートからの相対パスへ変換する。ルート直下のエントリは相対パスがファイル名になる。
    private static func relativePath(of url: URL, from rootPath: String) -> String {
        let path = url.standardizedFileURL.path
        guard path.hasPrefix(rootPath) else { return url.lastPathComponent }
        let trimmed = path.dropFirst(rootPath.count).trimmingCharacters(in: CharacterSet(charactersIn: "/"))
        return String(trimmed)
    }
}

/// スナップショット作成時の失敗を表す。
public enum DirectoryWatcherError: Error, Sendable {
    case rootIsNotDirectory(String)
    case unreadableRoot(String)
}

/// 複数ルートを一定間隔で再スキャンし、スナップショットが変わったときにコールバックを
/// 一度だけ通知する監視器。外部依存なし。タイマーは内部のserialキューで駆動するため、
/// スキャン同士が並行に走ることはない。
public final class DirectoryWatcher: @unchecked Sendable {

    public typealias ChangeHandler = @Sendable () -> Void

    private let roots: [URL]
    private let interval: TimeInterval
    private let excludedNames: Set<String>
    private let onChange: ChangeHandler
    private let queue = DispatchQueue(label: "com.diffloupe.DirectoryWatcher.\(UUID().uuidString)")
    private let lock = NSLock()
    private var timer: DispatchSourceTimer?
    private var lastSnapshots: [String: DirectorySnapshot] = [:]
    private var isRunning = false
    /// 初回baseline(基準スナップショット)の記録が完了するまでfalse。完了前のpollは何もしない。
    private var baselineReady = false
    /// start/stopごとに進めるrun識別子。古いrunの非同期baselineが新しいrunへ書き込まないためのトークン。
    private var generation = 0

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

    /// 監視を開始する。初回の基準スナップショットはwatcher内部のserialキューで非同期に記録する
    /// (呼び出しスレッド=MainActor等をブロックしない)。baseline完了前のpollは何もしない。
    /// baseline完了後、intervalごとに再スキャンして変化があればonChangeを一度だけ呼ぶ
    /// (同一ポーリング内の複数変更は1回にまとめられる)。stop後に再startすることが可能で、
    /// 古いrunのbaselineタスクが新しいrunのスナップショットを上書きすることはない。
    public func start() {
        lock.lock()
        defer { lock.unlock() }
        guard !isRunning else { return }
        isRunning = true
        baselineReady = false
        generation += 1
        let runGeneration = generation
        lastSnapshots = [:]

        let timer = DispatchSource.makeTimerSource(queue: queue)
        timer.schedule(deadline: .now() + interval, repeating: interval, leeway: .milliseconds(50))
        timer.setEventHandler { [weak self] in
            self?.poll()
        }
        timer.resume()
        self.timer = timer

        // 初回baselineはserial queueで非同期にスキャンする(修正: MainActor上での同期scanを廃止)。
        // queue上のためtimerのpollとは直列化され、baseline完了前のpollはbaselineReady=falseで無視される。
        queue.async { [weak self] in
            guard let self else { return }
            var snapshots: [String: DirectorySnapshot] = [:]
            snapshots.reserveCapacity(self.roots.count)
            for root in self.roots {
                if let snapshot = try? DirectorySnapshot.scan(
                    root: root, excludedNames: self.excludedNames
                ) {
                    snapshots[root.path] = snapshot
                }
            }
            self.lock.lock()
            defer { self.lock.unlock() }
            // この間にstop()/再start()でgenerationが進んでいたら、古いrunのbaselineとして破棄する
            guard self.isRunning, self.generation == runGeneration else { return }
            self.lastSnapshots = snapshots
            self.baselineReady = true
        }
    }

    /// 監視を停止する。停止後は再スキャンも通知も行わない。進行中の非同期baselineは
    /// generationトークンの不整合により破棄されるため、baseline途中でも安全に終わる。
    /// 停止後に再startすることは可能。
    public func stop() {
        lock.lock()
        generation += 1
        let timer = self.timer
        self.timer = nil
        isRunning = false
        baselineReady = false
        lastSnapshots = [:]
        lock.unlock()
        timer?.cancel()
    }

    private func poll() {
        let rootsToCheck: [URL]
        lock.lock()
        guard isRunning, baselineReady else {
            lock.unlock()
            return
        }
        rootsToCheck = roots
        lock.unlock()

        var changed = false
        for root in rootsToCheck {
            guard let snapshot = try? DirectorySnapshot.scan(
                root: root, excludedNames: excludedNames
            ) else { continue }
            lock.lock()
            let previous = lastSnapshots[root.path]
            lastSnapshots[root.path] = snapshot
            lock.unlock()
            if previous != snapshot {
                changed = true
            }
        }

        guard changed else { return }
        lock.lock()
        let shouldNotify = isRunning
        lock.unlock()
        if shouldNotify {
            onChange()
        }
    }
}
