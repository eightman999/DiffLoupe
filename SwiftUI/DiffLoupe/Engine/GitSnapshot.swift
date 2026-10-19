// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: gitリポジトリの検出とref(ブランチ/タグ/HEAD等)の一時ディレクトリへの実体化(スナップショット)を担う。
// FolderComparatorはFileManager直依存のまま変更せず、git側の内容を一時ディレクトリに展開してURLとして渡す最小侵襲方式。

import Foundation

/// 検出されたgit作業ツリー1件分の情報
public struct GitRepository: Sendable, Equatable {

    /// `git`実行時に発生し得るエラー。ユーザー向けメッセージを持つ。
    public struct GitError: LocalizedError, Sendable {
        public let message: String
        public var errorDescription: String? { message }
    }

    /// 作業ツリーのトップレベル(リポジトリルート)絶対パス
    public let toplevel: URL

    private static let gitExecutablePath = "/usr/bin/git"

    /// 指定フォルダがgit作業ツリー内かを検出する。
    /// gitバイナリが存在しない、またはリポジトリでない場合はnilを返す。
    public static func detect(url: URL) -> GitRepository? {
        guard FileManager.default.isExecutableFile(atPath: gitExecutablePath) else { return nil }

        guard let output = try? runGit(
            arguments: ["-C", url.path, "rev-parse", "--show-toplevel"],
            currentDirectory: url
        ) else { return nil }

        let trimmed = output.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return nil }
        return GitRepository(toplevel: URL(fileURLWithPath: trimmed, isDirectory: true))
    }

    /// ブランチ・タグ一覧を取得し、先頭に"HEAD"を追加して返す。
    public func listRefs() -> [String] {
        guard let output = try? Self.runGit(
            arguments: ["-C", toplevel.path, "for-each-ref", "--format=%(refname:short)"],
            currentDirectory: toplevel
        ) else { return ["HEAD"] }

        let refs = output
            .split(separator: "\n")
            .map { String($0).trimmingCharacters(in: .whitespaces) }
            .filter { !$0.isEmpty }

        return ["HEAD"] + refs
    }

    /// 指定refの内容を一時ディレクトリに展開(archive方式)し、そのURLを返す。
    /// `checkout`で作業ツリーを汚す方式は使わない。
    /// - Throws: refが存在しない等gitコマンドが失敗した場合、`GitError`をユーザー向けメッセージ付きでthrowする。
    public func materializeSnapshot(ref: String) throws -> URL {
        let tempDir = FileManager.default.temporaryDirectory
            .appendingPathComponent("DiffLoupe-git-\(UUID().uuidString)", isDirectory: true)

        do {
            try FileManager.default.createDirectory(at: tempDir, withIntermediateDirectories: true)
        } catch {
            throw GitError(message: "一時ディレクトリの作成に失敗しました: \(error.localizedDescription)")
        }

        do {
            try runGitArchivePipedToTar(ref: ref, destination: tempDir)
        } catch {
            // 失敗時は作りかけの一時ディレクトリを掃除してから投げ直す
            try? FileManager.default.removeItem(at: tempDir)
            throw error
        }

        return tempDir
    }

    /// materializeSnapshotで作成した一時ディレクトリを削除する。
    public static func cleanup(_ url: URL) {
        try? FileManager.default.removeItem(at: url)
    }

    // MARK: - Process実行

    /// `git archive <ref>` の標準出力を `tar -x -C <destination>` の標準入力へPipeで接続して展開する。
    /// 引数は必ず配列で渡し、シェル文字列組み立ては行わない(インジェクション防止)。
    private func runGitArchivePipedToTar(ref: String, destination: URL) throws {
        let archiveProcess = Process()
        archiveProcess.executableURL = URL(fileURLWithPath: Self.gitExecutablePath)
        archiveProcess.arguments = ["-C", toplevel.path, "archive", ref]
        archiveProcess.currentDirectoryURL = toplevel

        let tarProcess = Process()
        tarProcess.executableURL = URL(fileURLWithPath: "/usr/bin/tar")
        tarProcess.arguments = ["-x", "-C", destination.path]

        let pipe = Pipe()
        archiveProcess.standardOutput = pipe
        tarProcess.standardInput = pipe

        let archiveErrorPipe = Pipe()
        archiveProcess.standardError = archiveErrorPipe
        let tarErrorPipe = Pipe()
        tarProcess.standardError = tarErrorPipe

        do {
            try archiveProcess.run()
            try tarProcess.run()
        } catch {
            throw GitError(message: "gitまたはtarの起動に失敗しました: \(error.localizedDescription)")
        }

        archiveProcess.waitUntilExit()
        // archive側のstdoutを閉じないとtar側がEOFを検知できない
        try? pipe.fileHandleForWriting.close()
        tarProcess.waitUntilExit()

        guard archiveProcess.terminationStatus == 0 else {
            let errData = archiveErrorPipe.fileHandleForReading.readDataToEndOfFile()
            let message = String(data: errData, encoding: .utf8)?
                .trimmingCharacters(in: .whitespacesAndNewlines)
            throw GitError(message: "ref '\(ref)' の取得に失敗しました。\(message.flatMap { $0.isEmpty ? nil : $0 } ?? "存在しないref、またはgit archiveに失敗した可能性があります。")")
        }

        guard tarProcess.terminationStatus == 0 else {
            let errData = tarErrorPipe.fileHandleForReading.readDataToEndOfFile()
            let message = String(data: errData, encoding: .utf8)?
                .trimmingCharacters(in: .whitespacesAndNewlines)
            throw GitError(message: "スナップショットの展開に失敗しました。\(message.flatMap { $0.isEmpty ? nil : $0 } ?? "")")
        }
    }

    /// gitコマンドを実行し標準出力を文字列で返す。失敗時はnilを返す(検出系の軽量呼び出し向け)。
    @discardableResult
    private static func runGit(arguments: [String], currentDirectory: URL) throws -> String {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: gitExecutablePath)
        process.arguments = arguments
        process.currentDirectoryURL = currentDirectory

        let outPipe = Pipe()
        let errPipe = Pipe()
        process.standardOutput = outPipe
        process.standardError = errPipe

        try process.run()
        let outData = outPipe.fileHandleForReading.readDataToEndOfFile()
        process.waitUntilExit()

        guard process.terminationStatus == 0 else {
            throw GitError(message: "gitコマンドの実行に失敗しました")
        }

        return String(data: outData, encoding: .utf8) ?? ""
    }
}
