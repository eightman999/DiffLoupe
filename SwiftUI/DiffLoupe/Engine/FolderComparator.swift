// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: 2フォルダを再帰走査し「サイズ→SHA-256」の段階判定で分類するコアエンジン(SPEC.md §2)

import Foundation
import CryptoKit
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
        var contentTargets: [(key: String, left: URL, right: URL, leftSize: Int64, rightSize: Int64)] = []
        let contentStrict = options.contentComparison.isStrict

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
                } else if contentStrict {
                    if l.size != r.size {
                        statusByPath[key] = .different      // サイズ不一致 → 相違確定
                    } else {
                        hashTargets.append((key, l.url, r.url))
                    }
                } else {
                    // 正規化モード: サイズでは相違を確定できないため、必ず内容比較する
                    // (例: CRLF vs LF、行末空白のみ異なるファイルはサイズが異なるが正規化後に一致しうる)
                    contentTargets.append((key, l.url, r.url, l.size, r.size))
                }
            case (nil, nil):
                break
            }
        }

        // フェーズ3: 内容比較(並列・SPEC.md §2.5)
        if !hashTargets.isEmpty {
            progress?(.comparing(done: 0, total: hashTargets.count))
            let hashResults = try await compareByHash(targets: hashTargets, progress: progress)
            for (key, status, message) in hashResults {
                statusByPath[key] = status
                if let message { errorByPath[key] = message }
            }
        }

        if !contentTargets.isEmpty {
            progress?(.comparing(done: 0, total: contentTargets.count))
            let contentResults = try await compareByContent(
                targets: contentTargets, options: options, progress: progress)
            for (key, status, message) in contentResults {
                statusByPath[key] = status
                if let message { errorByPath[key] = message }
            }
        }

        // フェーズ3.5: リネーム/移動候補の検出(片側のみファイルの内容ハッシュ突き合わせ)
        var renameByPath: [String: RenameCandidate] = [:]
        if options.detectRenames {
            renameByPath = try await detectRenames(
                leftMap: leftMap, rightMap: rightMap,
                statusByPath: statusByPath, progress: progress)
        }

        // フェーズ4: ディレクトリへの集約と結果構築(SPEC.md §2.4)
        try Task.checkCancellation()
        propagateToAncestors(statusByPath: &statusByPath)

        return buildResult(
            leftRoot: left, rightRoot: right,
            leftMap: leftMap, rightMap: rightMap,
            statusByPath: statusByPath, errorByPath: errorByPath,
            renameByPath: renameByPath
        )
    }

    // MARK: - 走査(SPEC.md §2.2)

    /// ルート配下を再帰走査し「NFC正規化済み相対パス → メタデータ」を返す。
    /// ディレクトリのシンボリックリンクは辿らず記録もしない。隠しファイルは対象に含める。
    /// 除外globに一致するエントリはスキップし、ディレクトリならサブツリーごと枝刈りする。
    /// インクルードglobが非空のときは、一致するファイルのみを対象に含める
    /// (ディレクトリはインクルードで刈らず、ネストした一致ファイルへ降下する)。
    private func scanTree(
        root: URL,
        options: Options,
        found: inout Int,
        progress: ProgressHandler?
    ) throws -> [String: FileEntryMeta] {

        let fm = FileManager.default
        let keys: Set<URLResourceKey> = [
            .isDirectoryKey, .isSymbolicLinkKey, .fileSizeKey,
            .contentModificationDateKey
        ]
        let filter = FilterSet(
            includePatterns: options.userIncludePatterns,
            excludePatterns: options.userExcludePatterns
        )
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

                // フィルタ照合・結果キーはNFC正規化済みパスで統一する。APFS/HFS+はファイル名を
                // NFDで返すため、そのまま照合するとNFCパターンと不一致になる(FilterRule/GlobPatternの
                // 契約は「relativePathはNFC正規化済み」)。正規化形式に依存しない一貫した照合のため
                // 名前と相対パスの両方を明示的にNFCへ揃える。
                let normalizedName = name.precomposedStringWithCanonicalMapping
                let childRel = relPath.isEmpty ? name : relPath + "/" + name
                let normalizedRel = childRel.precomposedStringWithCanonicalMapping
                if filter.isExcluded(name: normalizedName, relativePath: normalizedRel) {
                    continue
                }
                let normalizedKey = normalizedRel

                let values = try? child.resourceValues(forKeys: keys)
                let isSymlink = values?.isSymbolicLink ?? false

                // POSIXメタデータはstatで取得(NSURLでは取得不可のため)
                let posix = Self.posixMetadata(of: child)
                var modificationDate = values?.contentModificationDate

                if isSymlink {
                    // リンク先がディレクトリなら無視(辿らない・記録しない)。
                    // ファイルなら対象とし、サイズはリンク先のもの。
                    // リンク切れは -1 とし、ハッシュ段階で読み込み失敗 → error になる
                    var targetIsDir: ObjCBool = false
                    let targetExists = fm.fileExists(atPath: child.path, isDirectory: &targetIsDir)
                    if targetExists && targetIsDir.boolValue { continue }
                    guard filter.allowsFile(name: normalizedName, relativePath: normalizedRel) else { continue }
                    var targetSize: Int64 = -1
                    if targetExists {
                        // URLResourceValuesはリンク自体を見るため、解決済みパスから取得する
                        let resolved = child.resolvingSymlinksInPath()
                        let resolvedValues = try? resolved.resourceValues(forKeys: keys)
                        targetSize = (resolvedValues?.fileSize).flatMap { Int64($0) } ?? -1
                        modificationDate = resolvedValues?.contentModificationDate
                            ?? modificationDate
                    }
                    result[normalizedKey] = FileEntryMeta(
                        url: child, isDirectory: false, size: targetSize, isSymlink: true,
                        modificationDate: modificationDate, permissions: posix?.permissions,
                        ownerID: posix?.ownerID, groupID: posix?.groupID)
                } else if values?.isDirectory == true {
                    // インクルードはディレクトリを刈らない。降下は再帰オプションのみで制御する
                    result[normalizedKey] = FileEntryMeta(
                        url: child, isDirectory: true, size: -1, isSymlink: false,
                        modificationDate: modificationDate, permissions: posix?.permissions,
                        ownerID: posix?.ownerID, groupID: posix?.groupID)
                    if options.recursive {
                        stack.append((child, normalizedRel))
                    }
                } else {
                    guard filter.allowsFile(name: normalizedName, relativePath: normalizedRel) else { continue }
                    let size = values?.fileSize.flatMap { Int64($0) } ?? -1
                    result[normalizedKey] = FileEntryMeta(
                        url: child, isDirectory: false, size: size, isSymlink: false,
                        modificationDate: modificationDate, permissions: posix?.permissions,
                        ownerID: posix?.ownerID, groupID: posix?.groupID)
                }

                found += 1
                if let maxEntries = options.maxEntries, found > maxEntries {
                    throw EntryLimitExceeded(limit: maxEntries, observed: found)
                }
                if found % 256 == 0 {
                    progress?(.scanning(found: found))
                }
            }
        }
        return result
    }

    /// POSIX基本メタデータ(パーミッションビット・UID・GID)をstatで取得する。
    /// シンボリックリンクはリンク先を辿る(サイズ取得と同じ規則)。取得失敗時(リンク切れ等)は nil。
    private static func posixMetadata(of url: URL) -> POSIXBasicMetadata? {
        var statBuffer = stat()
        guard stat(url.path, &statBuffer) == 0 else { return nil }
        return POSIXBasicMetadata(
            permissions: Int(statBuffer.st_mode & 0o7777),
            ownerID: Int(statBuffer.st_uid),
            groupID: Int(statBuffer.st_gid)
        )
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

    // MARK: - 正規化付き内容比較(SPEC §2.x 内容比較モード)

    /// 正規化オプション付きの内容比較。サイズ一致時のみハッシュの高速経路を使い、
    /// ハッシュ不一致・サイズ不一致の場合は実内容を読み込み、ContentComparisonで正規化して比較する。
    /// バイナリやUTF-8としてデコード不能なファイルはバイト厳密比較にフォールバックする
    /// (正規化で内容相違を見逃さないための保守側の挙動)。
    private func compareByContent(
        targets: [(key: String, left: URL, right: URL, leftSize: Int64, rightSize: Int64)],
        options: Options,
        progress: ProgressHandler?
    ) async throws -> [(key: String, status: DiffStatus, errorMessage: String?)] {

        let contentComparison = options.contentComparison
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
                        // 高速経路: サイズ一致かつハッシュ一致 → バイト同一なので正規化後も一致
                        if target.leftSize == target.rightSize,
                           target.leftSize >= 0, target.rightSize >= 0 {
                            let leftHash = try Self.sha256(of: target.left)
                            let rightHash = try Self.sha256(of: target.right)
                            if leftHash == rightHash {
                                return (target.key, .identical, nil)
                            }
                            // ハッシュ不一致。厳密比較なら相違確定。
                            // 正規化モードでも巨大ファイルは一括読込しない(バイト厳密フォールバック)。
                            if contentComparison.isStrict
                                || target.leftSize > options.maxNormalizationCompareBytes {
                                return (target.key, .different, nil)
                            }
                        } else if contentComparison.isStrict
                                    || max(target.leftSize, target.rightSize) > options.maxNormalizationCompareBytes {
                            // サイズ不一致。厳密比較なら相違確定。巨大ファイルもバイト厳密フォールバック。
                            return (target.key, .different, nil)
                        }

                        // 正規化比較(サイズ不一致・ハッシュ不一致の両方をカバー)
                        let leftData = try Data(contentsOf: target.left)
                        let rightData = try Data(contentsOf: target.right)
                        // 構造化モード(JSON/XML/CSV)は同じ正規化エンジンを両側に適用し、
                        // 拡張子(automatic)または明示モードで判定する。左右両方が正規化に
                        // 成功した場合のみカノニカル形式で比較する。片側でもパース失敗したら
                        // structuredMode を無効化した従来のテキスト/バイト比較へ左右両方を
                        // フォールバックする(成功側のcanonicalと失敗側のrawを混在比較しない)。
                        if let canonicalPair = contentComparison.structuredCanonicalPair(
                            leftData, url: target.left,
                            rightData, url: target.right
                        ) {
                            var canonicalComparison = contentComparison
                            canonicalComparison.structuredMode = .disabled
                            let leftNormalized = canonicalComparison.normalizedText(canonicalPair.leftText)
                                .data(using: .utf8)
                            let rightNormalized = canonicalComparison.normalizedText(canonicalPair.rightText)
                                .data(using: .utf8)
                            return (
                                target.key,
                                leftNormalized == rightNormalized ? .identical : .different,
                                nil
                            )
                        }
                        var fallbackComparison = contentComparison
                        fallbackComparison.structuredMode = .disabled
                        let leftNormalized = fallbackComparison.normalizedData(leftData)
                        let rightNormalized = fallbackComparison.normalizedData(rightData)
                        if let leftNormalized, let rightNormalized {
                            return (
                                target.key,
                                leftNormalized == rightNormalized ? .identical : .different,
                                nil
                            )
                        }
                        return (target.key, leftData == rightData ? .identical : .different, nil)
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

    // MARK: - リネーム/移動候補の検出(SPEC §2.x)

    /// 片側のみのファイルを内容ハッシュで束ね、対向側の片側のみファイルと一致するものを
    /// リネーム/移動候補として返す。DiffStatusは変更しない(ComparedEntry.renameCandidateに載せる)。
    /// ディレクトリは対象外。同内容が複数存在する場合は1対1対応(isUniqueMatch)を立てず、
    /// 代表パス+件数を示すことで「確定でない対応」をUIが安全に扱えるようにする。
    private func detectRenames(
        leftMap: [String: FileEntryMeta],
        rightMap: [String: FileEntryMeta],
        statusByPath: [String: DiffStatus],
        progress: ProgressHandler?
    ) async throws -> [String: RenameCandidate] {

        let leftFiles = leftMap.keys.filter {
            statusByPath[$0] == .leftOnly && !(leftMap[$0]?.isDirectory ?? true)
        }
        let rightFiles = rightMap.keys.filter {
            statusByPath[$0] == .rightOnly && !(rightMap[$0]?.isDirectory ?? true)
        }
        guard !leftFiles.isEmpty, !rightFiles.isEmpty else { return [:] }

        let maxConcurrent = min(8, ProcessInfo.processInfo.activeProcessorCount)
        var leftHashes: [String: [UInt8]] = [:]
        var rightHashes: [String: [UInt8]] = [:]

        struct Target {
            let isLeft: Bool
            let key: String
            let url: URL
        }
        let targets: [Target] =
            leftFiles.map { Target(isLeft: true, key: $0, url: leftMap[$0]!.url) } +
            rightFiles.map { Target(isLeft: false, key: $0, url: rightMap[$0]!.url) }

        progress?(.renames(done: 0, total: targets.count))
        let reportInterval = max(1, targets.count / 100)

        try await withThrowingTaskGroup(of: (Bool, String, [UInt8]?).self) { group in
            var iterator = targets.makeIterator()

            @discardableResult
            func addNext() -> Bool {
                guard let target = iterator.next() else { return false }
                group.addTask {
                    try Task.checkCancellation()
                    do {
                        let hash = try Array(Self.sha256(of: target.url))
                        return (target.isLeft, target.key, hash)
                    } catch is CancellationError {
                        // キャンセルは読み込みエラーと区別して即座に再送出する
                        throw CancellationError()
                    } catch {
                        // 読み込み不可のファイルは候補に含めない(内容不明のため)
                        return (target.isLeft, target.key, nil)
                    }
                }
                return true
            }

            for _ in 0..<maxConcurrent { addNext() }
            var done = 0
            while let item = try await group.next() {
                if let hash = item.2 {
                    if item.0 { leftHashes[item.1] = hash } else { rightHashes[item.1] = hash }
                }
                done += 1
                if done % reportInterval == 0 || done == targets.count {
                    progress?(.renames(done: done, total: targets.count))
                }
                addNext()
            }
        }

        var leftByHash: [[UInt8]: [String]] = [:]
        var rightByHash: [[UInt8]: [String]] = [:]
        for (path, hash) in leftHashes { leftByHash[hash, default: []].append(path) }
        for (path, hash) in rightHashes { rightByHash[hash, default: []].append(path) }

        var result: [String: RenameCandidate] = [:]
        for (hash, leftPaths) in leftByHash {
            guard let rightPaths = rightByHash[hash], !rightPaths.isEmpty else { continue }
            let isUnique = leftPaths.count == 1 && rightPaths.count == 1
            let sortedLeft = leftPaths.sorted()
            let sortedRight = rightPaths.sorted()
            for leftPath in sortedLeft {
                result[leftPath] = RenameCandidate(
                    counterpartPath: sortedRight[0],
                    isUniqueMatch: isUnique,
                    totalMatches: sortedRight.count
                )
            }
            for rightPath in sortedRight {
                result[rightPath] = RenameCandidate(
                    counterpartPath: sortedLeft[0],
                    isUniqueMatch: isUnique,
                    totalMatches: sortedLeft.count
                )
            }
        }
        return result
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
        statusByPath: [String: DiffStatus], errorByPath: [String: String],
        renameByPath: [String: RenameCandidate] = [:]
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
                errorMessage: errorByPath[path],
                leftModificationDate: leftMeta?.modificationDate,
                rightModificationDate: rightMeta?.modificationDate,
                leftPOSIX: leftMeta.map {
                    POSIXBasicMetadata(permissions: $0.permissions, ownerID: $0.ownerID, groupID: $0.groupID)
                },
                rightPOSIX: rightMeta.map {
                    POSIXBasicMetadata(permissions: $0.permissions, ownerID: $0.ownerID, groupID: $0.groupID)
                },
                renameCandidate: renameByPath[path]
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
