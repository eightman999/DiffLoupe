// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: 2フォルダを再帰走査し「サイズ→SHA-256」の段階判定で分類するコアエンジン(SPEC.md §2)

import Foundation
import CryptoKit

struct FolderComparator: Sendable {

    struct Options: Sendable {
        /// 既定除外パターン(.git / .DS_Store / node_modules)を適用するか(SPEC.md §2.2)
        var useDefaultExcludes = true

        static let defaultExcludedNames: Set<String> = [".git", ".DS_Store", "node_modules"]

        var excludedNames: Set<String> {
            useDefaultExcludes ? Self.defaultExcludedNames : []
        }
    }

    typealias ProgressHandler = @Sendable (ComparisonProgress) -> Void

    /// 64KiBチャンク(既存C++/Python版と同一。SPEC.md §2.3)
    static let hashChunkSize = 65536

    func compare(
        left: URL,
        right: URL,
        options: Options = Options(),
        progress: ProgressHandler? = nil
    ) async throws -> ComparisonResult {

        // フェーズ1: 走査
        var foundCount = 0
        let leftMap = try scanTree(root: left, options: options, found: &foundCount, progress: progress)
        let rightMap = try scanTree(root: right, options: options, found: &foundCount, progress: progress)
        progress?(.scanning(found: foundCount))

        // フェーズ2: 突き合わせと一次分類(SPEC.md §2.3 手順1〜3)
        let allKeys = Set(leftMap.keys).union(rightMap.keys)
        var statusByPath: [String: DiffStatus] = [:]
        var errorByPath: [String: String] = [:]
        var hashTargets: [(key: String, left: URL, right: URL)] = []

        for key in allKeys {
            switch (leftMap[key], rightMap[key]) {
            case (.some, nil):
                statusByPath[key] = .leftOnly
            case (nil, .some):
                statusByPath[key] = .rightOnly
            case let (l?, r?):
                if l.isDirectory != r.isDirectory {
                    statusByPath[key] = .different      // 種別不一致
                } else if l.isDirectory {
                    statusByPath[key] = .identical      // 仮置き。後段で子孫から集約
                } else if l.size != r.size {
                    statusByPath[key] = .different      // サイズ不一致 → 相違確定
                } else {
                    hashTargets.append((key, l.url, r.url))
                }
            case (nil, nil):
                break
            }
        }

        // フェーズ3: サイズ一致ファイルのハッシュ比較(並列・SPEC.md §2.5)
        if !hashTargets.isEmpty {
            progress?(.comparing(done: 0, total: hashTargets.count))
            let hashResults = try await compareByHash(targets: hashTargets, progress: progress)
            for (key, status, message) in hashResults {
                statusByPath[key] = status
                if let message { errorByPath[key] = message }
            }
        }

        // フェーズ4: ディレクトリへの集約と結果構築(SPEC.md §2.4)
        try Task.checkCancellation()
        propagateToAncestors(statusByPath: &statusByPath)

        return buildResult(
            leftRoot: left, rightRoot: right,
            leftMap: leftMap, rightMap: rightMap,
            statusByPath: statusByPath, errorByPath: errorByPath
        )
    }

    // MARK: - 走査(SPEC.md §2.2)

    /// ルート配下を再帰走査し「NFC正規化済み相対パス → メタデータ」を返す。
    /// ディレクトリのシンボリックリンクは辿らず記録もしない。隠しファイルは対象に含める。
    private func scanTree(
        root: URL,
        options: Options,
        found: inout Int,
        progress: ProgressHandler?
    ) throws -> [String: FileEntryMeta] {

        let fm = FileManager.default
        let keys: Set<URLResourceKey> = [.isDirectoryKey, .isSymbolicLinkKey, .fileSizeKey]
        var result: [String: FileEntryMeta] = [:]
        var stack: [(url: URL, relativePath: String)] = [(root, "")]

        while let (dirURL, relPath) = stack.popLast() {
            try Task.checkCancellation()

            let children = try fm.contentsOfDirectory(
                at: dirURL,
                includingPropertiesForKeys: Array(keys),
                options: []
            )

            for child in children {
                let name = child.lastPathComponent
                if options.excludedNames.contains(name) { continue }

                let childRel = relPath.isEmpty ? name : relPath + "/" + name
                let normalizedKey = childRel.precomposedStringWithCanonicalMapping

                let values = try? child.resourceValues(forKeys: keys)
                let isSymlink = values?.isSymbolicLink ?? false

                if isSymlink {
                    // リンク先がディレクトリなら無視(辿らない・記録しない)。
                    // ファイルなら対象とし、サイズはリンク先のもの。
                    // リンク切れは -1 とし、ハッシュ段階で読み込み失敗 → error になる
                    var targetIsDir: ObjCBool = false
                    let targetExists = fm.fileExists(atPath: child.path, isDirectory: &targetIsDir)
                    if targetExists && targetIsDir.boolValue { continue }
                    var targetSize: Int64 = -1
                    if targetExists {
                        // URLResourceValuesはリンク自体を見るため、解決済みパスから取得する
                        targetSize = (try? child.resolvingSymlinksInPath()
                            .resourceValues(forKeys: [.fileSizeKey]).fileSize)
                            .flatMap { Int64($0) } ?? -1
                    }
                    result[normalizedKey] = FileEntryMeta(
                        url: child, isDirectory: false, size: targetSize, isSymlink: true)
                } else if values?.isDirectory == true {
                    result[normalizedKey] = FileEntryMeta(
                        url: child, isDirectory: true, size: -1, isSymlink: false)
                    stack.append((child, childRel))
                } else {
                    let size = values?.fileSize.flatMap { Int64($0) } ?? -1
                    result[normalizedKey] = FileEntryMeta(
                        url: child, isDirectory: false, size: size, isSymlink: false)
                }

                found += 1
                if found % 256 == 0 {
                    progress?(.scanning(found: found))
                }
            }
        }
        return result
    }

    // MARK: - ハッシュ比較(SPEC.md §2.3 手順4)

    private func compareByHash(
        targets: [(key: String, left: URL, right: URL)],
        progress: ProgressHandler?
    ) async throws -> [(key: String, status: DiffStatus, errorMessage: String?)] {

        let maxConcurrent = min(8, ProcessInfo.processInfo.activeProcessorCount)
        let reportInterval = max(1, targets.count / 100)
        var results: [(String, DiffStatus, String?)] = []
        results.reserveCapacity(targets.count)

        try await withThrowingTaskGroup(of: (String, DiffStatus, String?).self) { group in
            var iterator = targets.makeIterator()

            @discardableResult
            func addNext() -> Bool {
                guard let target = iterator.next() else { return false }
                group.addTask {
                    try Task.checkCancellation()
                    do {
                        let leftHash = try Self.sha256(of: target.left)
                        let rightHash = try Self.sha256(of: target.right)
                        return (target.key, leftHash == rightHash ? .identical : .different, nil)
                    } catch is CancellationError {
                        throw CancellationError()
                    } catch {
                        // 読み込み不可は「一致」扱いにしない(SPEC.md §1.4 B3の修正)
                        return (target.key, .error, error.localizedDescription)
                    }
                }
                return true
            }

            for _ in 0..<maxConcurrent { addNext() }

            var done = 0
            while let item = try await group.next() {
                results.append(item)
                done += 1
                if done % reportInterval == 0 || done == targets.count {
                    progress?(.comparing(done: done, total: targets.count))
                }
                addNext()
            }
        }
        return results
    }

    /// SHA-256をストリーミング計算(64KiBチャンク、キャンセル対応)
    static func sha256(of url: URL) throws -> SHA256.Digest {
        let handle = try FileHandle(forReadingFrom: url)
        defer { try? handle.close() }

        var hasher = SHA256()
        while true {
            try Task.checkCancellation()
            let chunk = try autoreleasepool {
                try handle.read(upToCount: hashChunkSize)
            }
            guard let chunk, !chunk.isEmpty else { break }
            hasher.update(data: chunk)
        }
        return hasher.finalize()
    }

    // MARK: - 集約と結果構築(SPEC.md §2.4)

    /// 非identicalなエントリの祖先(両側に存在するディレクトリ)を different にする
    private func propagateToAncestors(statusByPath: inout [String: DiffStatus]) {
        let snapshot = statusByPath.filter { $0.value != .identical }
        for path in snapshot.keys {
            var current = path
            while let separatorIndex = current.lastIndex(of: "/") {
                current = String(current[..<separatorIndex])
                guard let ancestorStatus = statusByPath[current] else { break }
                if ancestorStatus == .identical {
                    statusByPath[current] = .different
                } else {
                    break   // 既に非identical: その祖先は当該エントリ自身の伝播で処理済み
                }
            }
        }
    }

    private func buildResult(
        leftRoot: URL, rightRoot: URL,
        leftMap: [String: FileEntryMeta], rightMap: [String: FileEntryMeta],
        statusByPath: [String: DiffStatus], errorByPath: [String: String]
    ) -> ComparisonResult {

        var entries: [ComparedEntry] = []
        entries.reserveCapacity(statusByPath.count)
        var summary = ComparisonResult.Summary()

        for (path, status) in statusByPath {
            let leftMeta = leftMap[path]
            let rightMeta = rightMap[path]
            let isDirectory = (leftMeta?.isDirectory ?? false) || (rightMeta?.isDirectory ?? false)
            let name = path.components(separatedBy: "/").last ?? path

            entries.append(ComparedEntry(
                relativePath: path,
                name: name,
                isDirectory: isDirectory,
                status: status,
                leftURL: leftMeta?.url,
                rightURL: rightMeta?.url,
                leftSize: leftMeta?.size ?? -1,
                rightSize: rightMeta?.size ?? -1,
                errorMessage: errorByPath[path]
            ))

            if !isDirectory {
                switch status {
                case .identical: summary.identicalFiles += 1
                case .different: summary.differentFiles += 1
                case .leftOnly: summary.leftOnlyFiles += 1
                case .rightOnly: summary.rightOnlyFiles += 1
                case .error: summary.errorFiles += 1
                }
            }
        }

        entries.sort { $0.relativePath < $1.relativePath }
        let byPath = Dictionary(uniqueKeysWithValues: entries.map { ($0.relativePath, $0) })
        let rootNode = Self.buildTree(entries: entries)

        return ComparisonResult(
            leftRoot: leftRoot, rightRoot: rightRoot,
            entries: entries, entriesByPath: byPath,
            rootNode: rootNode, summary: summary
        )
    }

    /// フラットなエントリ列からツリーを構築する。
    /// 同一階層はディレクトリ優先・名前昇順(SPEC.md §2.4)
    static func buildTree(entries: [ComparedEntry]) -> DiffNode {
        final class Builder {
            var name = ""
            var path = ""
            var entry: ComparedEntry?
            var children: [String: Builder] = [:]
        }

        let root = Builder()
        for entry in entries {
            var node = root
            var components = ArraySlice(entry.relativePath.components(separatedBy: "/"))
            var pathSoFar = ""
            while let component = components.popFirst() {
                pathSoFar = pathSoFar.isEmpty ? component : pathSoFar + "/" + component
                if let child = node.children[component] {
                    node = child
                } else {
                    let child = Builder()
                    child.name = component
                    child.path = pathSoFar
                    node.children[component] = child
                    node = child
                }
            }
            node.entry = entry
        }

        func convert(_ builder: Builder) -> DiffNode {
            let isDirectory = builder.entry?.isDirectory ?? true
            let hasChildren = !builder.children.isEmpty
            var childNodes: [DiffNode]? = nil
            if isDirectory || hasChildren {
                childNodes = builder.children.values.map(convert).sorted { a, b in
                    if a.isDirectory != b.isDirectory { return a.isDirectory }
                    return a.name.localizedStandardCompare(b.name) == .orderedAscending
                }
            }
            return DiffNode(
                id: builder.path,
                name: builder.name,
                isDirectory: isDirectory || hasChildren,
                status: builder.entry?.status ?? .different,
                children: childNodes
            )
        }

        let rootChildren = root.children.values.map(convert).sorted { a, b in
            if a.isDirectory != b.isDirectory { return a.isDirectory }
            return a.name.localizedStandardCompare(b.name) == .orderedAscending
        }
        let rootStatus: DiffStatus = rootChildren.allSatisfy { $0.status == .identical }
            ? .identical : .different
        return DiffNode(id: "", name: "", isDirectory: true, status: rootStatus, children: rootChildren)
    }
}
